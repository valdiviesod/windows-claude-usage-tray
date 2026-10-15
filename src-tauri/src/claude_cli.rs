//! Thin wrapper around the *unmodified, official* `claude` binary.
//!
//! The app never sees, reads or stores Claude credentials: sign-in, sign-out
//! and token storage are done by Claude Code itself inside an isolated
//! `CLAUDE_CONFIG_DIR` per account. We only ask it "who is logged in?".

use crate::model::{now_secs, LimitWindow, UsageSnapshot};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Variables that would silently override the per-account OAuth session.
const SCRUBBED_ENV: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
];

/// Probe values: documented as `utilization` in the SDK rate-limit event; the
/// unit is verified in docs/RESEARCH.md.
const PROBE_FRACTION_TO_PCT: f64 = 100.0;

pub fn find_claude() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let exe = dir.join("claude.exe");
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from)?;
    [".local/bin/claude.exe", "AppData/Roaming/npm/claude.cmd"]
        .iter()
        .map(|rel| home.join(rel))
        .find(|p| p.is_file())
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthStatus {
    pub logged_in: bool,
    pub email: Option<String>,
    pub plan: Option<String>,
    pub auth_method: Option<String>,
}

pub fn parse_auth_status(stdout: &str) -> Result<AuthStatus, String> {
    let start = stdout.find('{').ok_or("respuesta vacía de claude auth status")?;
    let v: Value = serde_json::from_str(&stdout[start..]).map_err(|e| format!("respuesta no válida: {e}"))?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    Ok(AuthStatus {
        logged_in: v.get("loggedIn").and_then(Value::as_bool).unwrap_or(false),
        email: s("email"),
        plan: s("subscriptionType"),
        auth_method: s("authMethod"),
    })
}

fn base_command(claude: &Path, config_dir: &Path) -> Command {
    let mut cmd = Command::new(claude);
    cmd.env("CLAUDE_CONFIG_DIR", config_dir)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null());
    for var in SCRUBBED_ENV {
        cmd.env_remove(var);
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

async fn run(mut cmd: Command, secs: u64) -> Result<std::process::Output, String> {
    match tokio::time::timeout(Duration::from_secs(secs), cmd.output()).await {
        Ok(Ok(out)) => Ok(out),
        Ok(Err(e)) => Err(format!("no se pudo ejecutar claude: {e}")),
        Err(_) => Err("claude no respondió a tiempo".into()),
    }
}

pub async fn auth_status(claude: &Path, config_dir: &Path) -> Result<AuthStatus, String> {
    std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
    let mut cmd = base_command(claude, config_dir);
    cmd.args(["auth", "status"]);
    let out = run(cmd, 25).await?;
    parse_auth_status(&String::from_utf8_lossy(&out.stdout))
}

pub async fn logout(claude: &Path, config_dir: &Path) -> Result<(), String> {
    let mut cmd = base_command(claude, config_dir);
    cmd.args(["auth", "logout"]);
    let out = run(cmd, 25).await?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Opens a visible console running the official `claude auth login` flow
/// (browser OAuth done by Claude Code) bound to this account's config dir.
pub fn launch_login(claude: &Path, config_dir: &Path, slot: u8, script_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(script_dir).map_err(|e| e.to_string())?;
    let script = script_dir.join("login.cmd");
    let body = format!(
        "@echo off\r\ntitle Claude Usage - Conectar cuenta {slot}\r\nset \"CLAUDE_CONFIG_DIR={cfg}\"\r\nset \"ANTHROPIC_API_KEY=\"\r\nset \"ANTHROPIC_AUTH_TOKEN=\"\r\necho Inicia sesion con la cuenta que quieres usar como cuenta {slot}.\r\necho.\r\n\"{claude}\" auth login --claudeai\r\necho.\r\necho Listo. Puedes cerrar esta ventana.\r\ntimeout /t 4 >nul\r\n",
        cfg = config_dir.display(),
        claude = claude.display(),
    );
    std::fs::write(&script, body).map_err(|e| e.to_string())?;
    std::process::Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(&script)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("no se pudo abrir la ventana de inicio de sesión: {e}"))
}

/// Make sure this account's Claude Code config dir reports its rate limits to
/// our helper (`claude-usage-tray.exe --statusline <slot>`). Existing keys in
/// settings.json are preserved. Returns true when the file was changed.
pub fn ensure_statusline(config_dir: &Path, exe: &Path, slot: u8) -> std::io::Result<bool> {
    std::fs::create_dir_all(config_dir)?;
    let path = config_dir.join("settings.json");
    let mut root: Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    // Forward slashes: Claude Code runs commands through a POSIX-style shell on Windows.
    let command = format!("\"{}\" --statusline {slot}", exe.to_string_lossy().replace('\\', "/"));
    // refreshInterval: keep reporting every few seconds even while the session is idle.
    let desired = json!({ "type": "command", "command": command, "refreshInterval": 5 });
    if root.get("statusLine") == Some(&desired) {
        return Ok(false);
    }
    root["statusLine"] = desired;
    std::fs::write(&path, serde_json::to_vec_pretty(&root).map_err(std::io::Error::other)?)?;
    Ok(true)
}

/// One extra-small real request through the official binary, to read the
/// `rate_limit_event` it emits. Consumes a little subscription quota, so it is
/// opt-in or user-triggered only.
pub async fn probe(claude: &Path, config_dir: &Path, work_dir: &Path) -> Result<UsageSnapshot, String> {
    std::fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    let mut cmd = base_command(claude, config_dir);
    cmd.current_dir(work_dir).args([
        "-p",
        ".",
        "--model",
        "haiku",
        "--output-format",
        "stream-json",
        "--verbose",
        "--max-turns",
        "1",
        "--no-session-persistence",
    ]);
    let out = run(cmd, 90).await?;
    parse_probe(&String::from_utf8_lossy(&out.stdout), now_secs())
        .ok_or_else(|| "claude no devolvió datos de límite (¿plan sin límites de suscripción?)".to_string())
}

fn probe_window(v: Option<&Value>) -> Option<LimitWindow> {
    let v = v?;
    let used = v.get("utilization")?.as_f64()? * PROBE_FRACTION_TO_PCT;
    let resets = v.get("resetsAt")?.as_i64()?;
    (used.is_finite() && resets > 0).then(|| LimitWindow { used_pct: used.clamp(0.0, 100.0), resets_at: resets })
}

pub fn parse_probe(stdout: &str, now: i64) -> Option<UsageSnapshot> {
    let mut best: Option<UsageSnapshot> = None;
    for line in stdout.lines().filter(|l| l.contains("rate_limit_event")) {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let Some(windows) = v.pointer("/rate_limit_info/unifiedWindows") else { continue };
        let snap = UsageSnapshot {
            captured_at: now,
            source: "probe".into(),
            five_hour: probe_window(windows.get("five_hour")),
            seven_day: probe_window(windows.get("seven_day")),
        };
        if snap.has_windows() {
            best = Some(snap);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_status_logged_in() {
        let s = parse_auth_status(r#"{"loggedIn":true,"email":"a@b.c","subscriptionType":"pro","authMethod":"claude.ai"}"#).unwrap();
        assert!(s.logged_in);
        assert_eq!(s.email.as_deref(), Some("a@b.c"));
        assert_eq!(s.plan.as_deref(), Some("pro"));
    }

    #[test]
    fn auth_status_logged_out_and_garbage() {
        assert!(!parse_auth_status(r#"{"loggedIn":false,"authMethod":"none"}"#).unwrap().logged_in);
        assert!(parse_auth_status("").is_err());
        assert!(parse_auth_status("{nope").is_err());
    }

    #[test]
    fn ensure_statusline_preserves_other_keys_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), r#"{"theme":"dark"}"#).unwrap();
        let exe = Path::new("C:\\Apps\\tray.exe");
        assert!(ensure_statusline(dir.path(), exe, 2).unwrap());
        assert!(!ensure_statusline(dir.path(), exe, 2).unwrap());
        let v: Value = serde_json::from_slice(&std::fs::read(dir.path().join("settings.json")).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["statusLine"]["command"], "\"C:/Apps/tray.exe\" --statusline 2");
    }

    #[test]
    fn probe_event_is_parsed() {
        let line = r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.31,"resetsAt":100},"seven_day":{"utilization":0.65,"resetsAt":200}}}}"#;
        let s = parse_probe(&format!("{{\"type\":\"system\"}}\n{line}\n"), 5).unwrap();
        assert!((s.five_hour.unwrap().used_pct - 31.0).abs() < 1e-9);
        assert_eq!(s.source, "probe");
        assert!(parse_probe("{\"type\":\"system\"}\n", 5).is_none());
    }
}
