//! Background sync engine. Lives in Rust, independent of the window: it keeps
//! running (and updating the tray tooltip) while the UI is hidden.
//!
//! Per tick and per account, independently:
//!   1. (rarely) ask the official client who is signed in  -> connection state
//!   2. read the account's local statusline snapshot        -> usage (free)
//!   3. (opt-in / user-triggered) probe through the client  -> usage (costs quota)
//! A failure on one account never blocks or alters the other.

use crate::alerts::{self, backoff_secs, fmt_duration};
use crate::cache::{AccountCache, Cache};
use crate::claude_cli::{self, AuthStatus};
use crate::model::{
    compute_status, now_secs, AccountView, AppView, Connection, Slot, UsageSnapshot, UsageStatus,
};
use crate::provider::{newest, AccountCtx, ProbeProvider, StatuslineProvider, UsageProvider};
use crate::settings::Settings;
use crate::{paths, tray};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Notify;

const AUTH_RECHECK_OK_SECS: i64 = 30 * 60;
const AUTH_RECHECK_INACTIVE_SECS: i64 = 10 * 60;
const AUTH_BACKOFF_BASE: u64 = 30;
const AUTH_BACKOFF_CAP: u64 = 30 * 60;
const PROBE_BACKOFF_CAP: u64 = 6 * 3600;
const LOGIN_POLL_SECS: u64 = 3;
const LOGIN_POLL_ATTEMPTS: u32 = 100; // ~5 minutes

#[derive(Clone, Copy, Debug, Default)]
pub struct TickOpts {
    /// Ignore the auth re-check schedule.
    pub force_auth: bool,
    /// Run the (quota-consuming) probe now.
    pub probe: bool,
}

#[derive(Debug)]
pub struct Runtime {
    pub connection: Connection,
    pub error: Option<String>,
    pub cache: AccountCache,
    pub auth_fails: u32,
    pub next_auth_check: i64,
    pub probe_fails: u32,
    pub next_probe: i64,
    pub login_gen: u64,
}

impl Runtime {
    pub fn from_cache(cache: AccountCache) -> Runtime {
        // Never claim "connected" before the official client confirmed it.
        let connection = if cache.was_connected { Connection::Checking } else { Connection::Disconnected };
        Runtime {
            connection,
            error: None,
            cache,
            auth_fails: 0,
            next_auth_check: 0,
            probe_fails: 0,
            next_probe: 0,
            login_gen: 0,
        }
    }

    fn forget_identity(&mut self) {
        self.cache.usage = None;
        self.cache.alert_five_hour = None;
        self.cache.alert_seven_day = None;
    }
}

/// Result of applying an auth check; tells the caller about side effects.
#[derive(Debug, PartialEq)]
pub struct AuthEffect {
    /// A different Claude account is now signed in: old snapshot must go.
    pub identity_changed: bool,
}

pub fn apply_auth(r: &mut Runtime, res: Result<AuthStatus, String>, now: i64) -> AuthEffect {
    let mut effect = AuthEffect { identity_changed: false };
    match res {
        Ok(st) if st.logged_in => {
            if st.auth_method.as_deref().is_some_and(|m| m != "claude.ai") {
                r.connection = Connection::Error;
                r.error = Some("Esta cuenta no usa una suscripción de Claude (Pro/Max).".into());
                r.next_auth_check = now + AUTH_RECHECK_INACTIVE_SECS;
                return effect;
            }
            if r.cache.email.is_some() && st.email.is_some() && r.cache.email != st.email {
                r.forget_identity();
                effect.identity_changed = true;
            }
            r.connection = Connection::Connected;
            r.error = None;
            r.cache.email = st.email;
            r.cache.plan = st.plan;
            r.cache.was_connected = true;
            r.auth_fails = 0;
            r.next_auth_check = now + AUTH_RECHECK_OK_SECS;
        }
        Ok(_) => {
            r.forget_identity();
            if r.cache.was_connected {
                r.connection = Connection::Expired;
                r.error = Some("La sesión expiró. Vuelve a conectar la cuenta.".into());
            } else {
                r.connection = Connection::Disconnected;
                r.error = None;
            }
            r.auth_fails = 0;
            r.next_auth_check = now + AUTH_RECHECK_INACTIVE_SECS;
        }
        Err(e) => {
            r.auth_fails += 1;
            r.next_auth_check = now + backoff_secs(AUTH_BACKOFF_BASE, r.auth_fails, AUTH_BACKOFF_CAP) as i64;
            r.error = Some(e);
            if matches!(r.connection, Connection::Checking | Connection::Disconnected) {
                r.connection = Connection::Error;
            }
        }
    }
    effect
}

struct Inner {
    acc: [Runtime; 2],
    settings: Settings,
    syncing: bool,
    last_sync_at: Option<i64>,
    claude: Option<PathBuf>,
    last_saved: String,
}

pub struct Engine {
    inner: Mutex<Inner>,
    base: PathBuf,
    exe: PathBuf,
    settings_changed: Notify,
}

impl Engine {
    pub fn new(base: PathBuf, exe: PathBuf, settings: Settings) -> Arc<Engine> {
        let cache = Cache::load(&paths::cache_path(&base));
        let mut accounts = cache.accounts.into_iter();
        let acc = [
            Runtime::from_cache(accounts.next().unwrap_or_default()),
            Runtime::from_cache(accounts.next().unwrap_or_default()),
        ];
        Arc::new(Engine {
            inner: Mutex::new(Inner {
                acc,
                settings: settings.sanitized(),
                syncing: false,
                last_sync_at: None,
                claude: claude_cli::find_claude(),
                last_saved: String::new(),
            }),
            base,
            exe,
            settings_changed: Notify::new(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn ctx(&self, slot: Slot) -> AccountCtx {
        AccountCtx { slot, base: self.base.clone(), claude: self.lock().claude.clone() }
    }

    pub fn settings(&self) -> Settings {
        self.lock().settings.clone()
    }

    pub fn set_settings(&self, s: Settings) -> Settings {
        let s = s.sanitized();
        self.lock().settings = s.clone();
        self.settings_changed.notify_one();
        s
    }

    pub fn view(&self) -> AppView {
        let g = self.lock();
        let now = now_secs();
        let accounts = Slot::ALL
            .iter()
            .map(|slot| {
                let r = &g.acc[slot.index()];
                let usage = (r.connection == Connection::Connected)
                    .then(|| r.cache.usage.as_ref().map(|u| u.live(now)))
                    .flatten();
                AccountView {
                    slot: slot.number(),
                    label: slot.label(),
                    connection: r.connection,
                    email: r.cache.email.clone().filter(|_| r.connection != Connection::Disconnected),
                    plan: r.cache.plan.clone().filter(|_| r.connection != Connection::Disconnected),
                    status: compute_status(r.connection, r.cache.usage.as_ref(), now),
                    usage,
                    error: r.error.clone(),
                }
            })
            .collect();
        AppView {
            now,
            accounts,
            settings: g.settings.clone(),
            syncing: g.syncing,
            last_sync_at: g.last_sync_at,
            claude_found: g.claude.is_some(),
        }
    }

    // ---- background loop ------------------------------------------------

    pub async fn run(self: Arc<Self>, app: AppHandle) {
        self.refresh_statusline_config();
        self.tick(&app, TickOpts { force_auth: true, probe: false }).await;
        let mut last_full = std::time::Instant::now();
        let mut mtimes = self.snapshot_mtimes();
        loop {
            // Real time: snapshot files are checked every second (a metadata
            // call, ~free); full sync on the configured interval.
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                _ = self.settings_changed.notified() => { last_full = std::time::Instant::now() - Duration::from_secs(86_400); }
            }
            let now_m = self.snapshot_mtimes();
            if now_m != mtimes {
                mtimes = now_m;
                self.reload_snapshots(&app);
            }
            if last_full.elapsed() >= Duration::from_secs(self.settings().interval_secs) {
                last_full = std::time::Instant::now();
                self.tick(&app, TickOpts::default()).await;
            }
        }
    }

    fn snapshot_mtimes(&self) -> [Option<std::time::SystemTime>; 2] {
        Slot::ALL.map(|s| std::fs::metadata(paths::snapshot_path(&self.base, s)).and_then(|m| m.modified()).ok())
    }

    fn reload_snapshots(&self, app: &AppHandle) {
        for slot in Slot::ALL {
            if self.lock().acc[slot.index()].connection == Connection::Connected {
                if let Ok(snap) = crate::snapshot::read_snapshot(&paths::snapshot_path(&self.base, slot)) {
                    self.merge_usage(slot, snap);
                }
            }
        }
        self.lock().last_sync_at = Some(now_secs());
        self.after_update(app);
    }

    /// Keep each connected account's Claude Code pointed at this exe's helper
    /// (the install path can change between versions).
    fn refresh_statusline_config(&self) {
        for slot in Slot::ALL {
            let was = self.lock().acc[slot.index()].cache.was_connected;
            if was {
                let _ = claude_cli::ensure_statusline(&paths::config_dir(&self.base, slot), &self.exe, slot.number());
            }
        }
    }

    pub async fn tick(self: &Arc<Self>, app: &AppHandle, opts: TickOpts) {
        {
            let mut g = self.lock();
            if g.syncing {
                return; // never run duplicate syncs
            }
            g.syncing = true;
            if g.claude.is_none() {
                g.claude = claude_cli::find_claude();
            }
        }
        self.emit(app);
        tokio::join!(self.sync_account(Slot::ALL[0], opts), self.sync_account(Slot::ALL[1], opts));
        {
            let mut g = self.lock();
            g.syncing = false;
            g.last_sync_at = Some(now_secs());
        }
        self.after_update(app);
    }

    async fn sync_account(&self, slot: Slot, opts: TickOpts) {
        let now = now_secs();
        let ctx = self.ctx(slot);
        let (conn, auth_due, probe_due, probe_enabled) = {
            let g = self.lock();
            let r = &g.acc[slot.index()];
            (r.connection, r.next_auth_check <= now, r.next_probe <= now, g.settings.probe_enabled)
        };

        if conn != Connection::Connecting && (opts.force_auth || auth_due) {
            let res = match ctx.claude.as_deref() {
                Some(claude) => claude_cli::auth_status(claude, &ctx.config_dir()).await,
                None => Err("Claude Code no está instalado (https://claude.ai/download).".into()),
            };
            let effect = apply_auth(&mut self.lock().acc[slot.index()], res, now);
            if effect.identity_changed {
                let _ = std::fs::remove_file(paths::snapshot_path(&self.base, slot));
            }
        }

        if self.lock().acc[slot.index()].connection != Connection::Connected {
            return;
        }

        match StatuslineProvider.fetch(&ctx).await {
            Ok(snap) => self.merge_usage(slot, snap),
            Err(e) => self.lock().acc[slot.index()].error = Some(e),
        }

        // Auto-probe only when no fresher data arrived from Claude Code itself.
        let stale = {
            let g = self.lock();
            let max_age = (g.settings.probe_interval_mins * 60) as i64;
            g.acc[slot.index()].cache.usage.as_ref().map_or(true, |u| now - u.captured_at >= max_age)
        };
        if opts.probe || (probe_enabled && probe_due && stale) {
            let res = ProbeProvider.fetch(&ctx).await;
            self.apply_probe(slot, res, now);
        }
    }

    fn merge_usage(&self, slot: Slot, snap: Option<UsageSnapshot>) {
        let mut g = self.lock();
        let r = &mut g.acc[slot.index()];
        r.cache.usage = newest(r.cache.usage.take(), snap);
    }

    fn apply_probe(&self, slot: Slot, res: Result<Option<UsageSnapshot>, String>, now: i64) {
        let mut g = self.lock();
        let interval = g.settings.probe_interval_mins * 60;
        let r = &mut g.acc[slot.index()];
        match res {
            Ok(snap) => {
                r.probe_fails = 0;
                r.next_probe = now + interval as i64;
                r.error = None;
                r.cache.usage = newest(r.cache.usage.take(), snap);
            }
            Err(e) => {
                r.probe_fails += 1;
                r.next_probe = now + backoff_secs(interval, r.probe_fails, PROBE_BACKOFF_CAP) as i64;
                r.error = Some(e);
            }
        }
    }

    // ---- connect / disconnect ------------------------------------------

    pub fn connect(self: &Arc<Self>, app: &AppHandle, slot: Slot) -> Result<(), String> {
        let claude = self
            .lock()
            .claude
            .clone()
            .or_else(claude_cli::find_claude)
            .ok_or("Claude Code no está instalado. Instálalo desde https://claude.ai/download")?;
        let cfg = paths::config_dir(&self.base, slot);
        claude_cli::ensure_statusline(&cfg, &self.exe, slot.number()).map_err(|e| e.to_string())?;
        claude_cli::launch_login(&claude, &cfg, slot.number(), &paths::account_dir(&self.base, slot))?;

        let gen = {
            let mut g = self.lock();
            g.claude = Some(claude.clone());
            let r = &mut g.acc[slot.index()];
            r.login_gen += 1;
            r.connection = Connection::Connecting;
            r.error = None;
            r.login_gen
        };
        self.emit(app);

        let this = Arc::clone(self);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            this.watch_login(&app, slot, gen, claude).await;
        });
        Ok(())
    }

    async fn watch_login(self: &Arc<Self>, app: &AppHandle, slot: Slot, gen: u64, claude: PathBuf) {
        let cfg = paths::config_dir(&self.base, slot);
        for _ in 0..LOGIN_POLL_ATTEMPTS {
            tokio::time::sleep(Duration::from_secs(LOGIN_POLL_SECS)).await;
            if self.lock().acc[slot.index()].login_gen != gen {
                return; // superseded by another connect/disconnect
            }
            if let Ok(st) = claude_cli::auth_status(&claude, &cfg).await {
                if st.logged_in {
                    let effect = apply_auth(&mut self.lock().acc[slot.index()], Ok(st), now_secs());
                    if effect.identity_changed {
                        let _ = std::fs::remove_file(paths::snapshot_path(&self.base, slot));
                    }
                    self.merge_usage(slot, crate::snapshot::read_snapshot(&paths::snapshot_path(&self.base, slot)).ok().flatten());
                    self.after_update(app);
                    return;
                }
            }
        }
        {
            let mut g = self.lock();
            let r = &mut g.acc[slot.index()];
            if r.login_gen == gen && r.connection == Connection::Connecting {
                r.connection = if r.cache.was_connected { Connection::Expired } else { Connection::Disconnected };
                r.error = Some("No se completó el inicio de sesión. Inténtalo de nuevo.".into());
            }
        }
        self.after_update(app);
    }

    pub async fn disconnect(self: &Arc<Self>, app: &AppHandle, slot: Slot) -> Result<(), String> {
        let (claude, gen) = {
            let mut g = self.lock();
            let claude = g.claude.clone();
            let r = &mut g.acc[slot.index()];
            r.login_gen += 1; // cancels a pending login watcher
            (claude, r.login_gen)
        };
        if let Some(claude) = claude {
            claude_cli::logout(&claude, &paths::config_dir(&self.base, slot)).await?;
        }
        {
            let mut g = self.lock();
            let r = &mut g.acc[slot.index()];
            if r.login_gen == gen {
                *r = Runtime::from_cache(AccountCache::default());
                r.login_gen = gen;
            }
        }
        let _ = std::fs::remove_file(paths::snapshot_path(&self.base, slot));
        self.after_update(app);
        Ok(())
    }

    // ---- outputs --------------------------------------------------------

    fn emit(&self, app: &AppHandle) {
        let _ = app.emit("state-changed", self.view());
    }

    fn after_update(&self, app: &AppHandle) {
        let (alerts, enabled) = self.collect_alerts();
        if enabled {
            for (title, body) in alerts {
                let _ = app.notification().builder().title(title).body(body).show();
            }
        }
        self.persist_cache();
        let view = self.view();
        tray::set_tooltip(app, &tooltip(&view));
        let _ = app.emit("state-changed", view);
    }

    fn collect_alerts(&self) -> (Vec<(String, String)>, bool) {
        let mut g = self.lock();
        let enabled = g.settings.notifications;
        let now = now_secs();
        let mut out = Vec::new();
        for slot in Slot::ALL {
            let r = &mut g.acc[slot.index()];
            if r.connection != Connection::Connected {
                continue;
            }
            let Some(usage) = r.cache.usage.as_ref().map(|u| u.live(now)) else { continue };
            for (name, window, mem) in [
                ("la ventana de 5 horas", usage.five_hour, &mut r.cache.alert_five_hour),
                ("el límite semanal", usage.seven_day, &mut r.cache.alert_seven_day),
            ] {
                let Some(w) = window else { continue };
                if let Some(level) = alerts::next_alert(mem, &w) {
                    let title = if level >= 100 {
                        format!("{}: límite alcanzado", slot.label())
                    } else {
                        format!("{}: {level}% de uso", slot.label())
                    };
                    let body = format!(
                        "Has usado el {:.0}% de {name}. Se reinicia en {}.",
                        w.used_pct,
                        fmt_duration(w.resets_at - now)
                    );
                    out.push((title, body));
                }
            }
        }
        (out, enabled)
    }

    fn persist_cache(&self) {
        let mut g = self.lock();
        let cache = Cache { accounts: g.acc.iter().map(|r| r.cache.clone()).collect() };
        if let Ok(json) = serde_json::to_string(&cache) {
            if json != g.last_saved {
                if cache.save(&paths::cache_path(&self.base)).is_ok() {
                    g.last_saved = json;
                }
            }
        }
    }
}

/// Windows truncates tray tooltips at 127 UTF-16 units: keep it short.
pub fn tooltip(view: &AppView) -> String {
    let parts: Vec<String> = view
        .accounts
        .iter()
        .map(|a| {
            let live = a.usage.as_ref().and_then(|u| u.five_hour);
            let detail = match (a.status, live) {
                (UsageStatus::Disconnected, _) => "sin conectar".to_string(),
                (UsageStatus::NoData, _) | (_, None) => "sin datos".to_string(),
                (_, Some(w)) => format!("{:.0}% · {}", w.used_pct, fmt_duration(w.resets_at - view.now)),
            };
            format!("{} {detail}", a.label)
        })
        .collect();
    format!("Claude Usage\n{}", parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(email: &str) -> Result<AuthStatus, String> {
        Ok(AuthStatus {
            logged_in: true,
            email: Some(email.into()),
            plan: Some("max".into()),
            auth_method: Some("claude.ai".into()),
        })
    }

    fn logged_out() -> Result<AuthStatus, String> {
        Ok(AuthStatus { logged_in: false, email: None, plan: None, auth_method: Some("none".into()) })
    }

    fn fresh() -> Runtime {
        Runtime::from_cache(AccountCache::default())
    }

    #[test]
    fn restart_never_claims_connected_before_verification() {
        let remembered = AccountCache { was_connected: true, email: Some("a@x.com".into()), ..Default::default() };
        assert_eq!(Runtime::from_cache(remembered).connection, Connection::Checking);
        assert_eq!(fresh().connection, Connection::Disconnected);
    }

    #[test]
    fn login_then_logout_flow() {
        let mut r = fresh();
        apply_auth(&mut r, ok("a@x.com"), 100);
        assert_eq!(r.connection, Connection::Connected);
        assert_eq!(r.cache.email.as_deref(), Some("a@x.com"));
        apply_auth(&mut r, logged_out(), 200);
        assert_eq!(r.connection, Connection::Expired);
        assert!(r.cache.usage.is_none());
    }

    #[test]
    fn never_connected_and_logged_out_is_plain_disconnected() {
        let mut r = fresh();
        apply_auth(&mut r, logged_out(), 100);
        assert_eq!(r.connection, Connection::Disconnected);
        assert!(r.error.is_none());
    }

    #[test]
    fn identity_change_drops_old_usage_and_alerts() {
        let mut r = fresh();
        apply_auth(&mut r, ok("old@x.com"), 1);
        r.cache.usage = Some(UsageSnapshot { captured_at: 1, ..Default::default() });
        r.cache.alert_five_hour = Some(alerts::AlertMemory { resets_at: 5, level: 95 });
        let effect = apply_auth(&mut r, ok("new@x.com"), 2);
        assert!(effect.identity_changed);
        assert!(r.cache.usage.is_none() && r.cache.alert_five_hour.is_none());
    }

    #[test]
    fn network_error_keeps_connected_and_backs_off_exponentially() {
        let mut r = fresh();
        apply_auth(&mut r, ok("a@x.com"), 0);
        apply_auth(&mut r, Err("timeout".into()), 1000);
        assert_eq!(r.connection, Connection::Connected);
        let first = r.next_auth_check - 1000;
        apply_auth(&mut r, Err("timeout".into()), 1000);
        let second = r.next_auth_check - 1000;
        assert!(second > first);
        apply_auth(&mut r, ok("a@x.com"), 2000);
        assert_eq!(r.auth_fails, 0);
        assert!(r.error.is_none());
    }

    #[test]
    fn first_check_error_is_error_state_not_fake_connection() {
        let mut r = Runtime::from_cache(AccountCache { was_connected: true, ..Default::default() });
        apply_auth(&mut r, Err("claude no respondió a tiempo".into()), 0);
        assert_eq!(r.connection, Connection::Error);
    }

    #[test]
    fn non_subscription_login_is_rejected() {
        let mut r = fresh();
        let st = AuthStatus { logged_in: true, email: Some("a@x.com".into()), plan: None, auth_method: Some("api_key".into()) };
        apply_auth(&mut r, Ok(st), 0);
        assert_eq!(r.connection, Connection::Error);
    }

    #[test]
    fn tooltip_is_short_and_honest() {
        let view = AppView {
            now: 1000,
            accounts: vec![
                AccountView { slot: 1, label: "Personal", connection: Connection::Connected, email: None, plan: None,
                    usage: Some(UsageSnapshot { five_hour: Some(crate::model::LimitWindow { used_pct: 42.4, resets_at: 1000 + 7800 }), ..Default::default() }),
                    status: UsageStatus::Available, error: None },
                AccountView { slot: 2, label: "Trabajo", connection: Connection::Disconnected, email: None, plan: None,
                    usage: None, status: UsageStatus::Disconnected, error: None },
            ],
            settings: Settings::default(),
            syncing: false,
            last_sync_at: None,
            claude_found: true,
        };
        let t = tooltip(&view);
        assert!(t.contains("Personal 42% · 2h 10m") && t.contains("Trabajo sin conectar"));
        assert!(t.encode_utf16().count() <= 127);
    }
}
