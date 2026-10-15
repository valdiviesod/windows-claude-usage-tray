//! Statusline snapshots: the officially documented way to read Pro/Max rate
//! limits. Claude Code pipes a JSON document (with `rate_limits.five_hour` /
//! `seven_day`, `used_percentage` 0-100 and `resets_at` epoch seconds) to the
//! configured `statusLine` command. This module parses that JSON and persists
//! it per account. See docs/RESEARCH.md.

use crate::model::{now_secs, LimitWindow, Slot, UsageSnapshot};
use crate::paths;
use serde_json::Value;
use std::io::Read;
use std::path::Path;

fn parse_window(v: Option<&Value>) -> Option<LimitWindow> {
    let v = v?;
    let used = v.get("used_percentage")?.as_f64()?;
    let resets = v.get("resets_at")?.as_i64()?;
    if !used.is_finite() || resets <= 0 {
        return None;
    }
    Some(LimitWindow { used_pct: used.clamp(0.0, 100.0), resets_at: resets })
}

/// Returns `None` when the document carries no usable rate-limit window
/// (e.g. before the first API response, or for non-subscriber logins).
pub fn parse_statusline(input: &str, now: i64) -> Option<UsageSnapshot> {
    let root: Value = serde_json::from_str(input).ok()?;
    let limits = root.get("rate_limits")?;
    let snap = UsageSnapshot {
        captured_at: now,
        source: "statusline".into(),
        five_hour: parse_window(limits.get("five_hour")),
        seven_day: parse_window(limits.get("seven_day")),
    };
    snap.has_windows().then_some(snap)
}

pub fn write_snapshot(path: &Path, snap: &UsageSnapshot) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(snap).map_err(std::io::Error::other)?;
    paths::write_atomic(path, &bytes)
}

/// `Ok(None)` when no snapshot exists yet; `Err` when the file is corrupt.
pub fn read_snapshot(path: &Path) -> Result<Option<UsageSnapshot>, String> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| format!("snapshot corrupto: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("no se pudo leer el snapshot: {e}")),
    }
}

fn norm(p: &str) -> String {
    p.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// True when the process env does not contradict `slot`'s own config dir.
/// Claude Code exports its CLAUDE_CONFIG_DIR to the statusline command; if it
/// points at another account's directory we refuse to write, so one account
/// can never overwrite the other's snapshot.
fn env_matches_slot(base: &Path, slot: Slot, env_dir: Option<&str>) -> bool {
    match env_dir {
        None | Some("") => true,
        Some(dir) => norm(dir) == norm(&paths::config_dir(base, slot).to_string_lossy()),
    }
}

/// Entry point of `claude-usage-tray.exe --statusline <slot>`.
/// Prints nothing (the status line stays empty); always exits 0 so it can
/// never break the Claude Code session that invokes it.
pub fn run_statusline(slot_arg: &str) -> i32 {
    let Some(slot) = slot_arg.parse::<u8>().ok().and_then(Slot::new) else {
        return 0;
    };
    let base = paths::base_dir();
    if !env_matches_slot(&base, slot, std::env::var("CLAUDE_CONFIG_DIR").ok().as_deref()) {
        return 0;
    }
    let mut input = String::new();
    if std::io::stdin().take(1 << 20).read_to_string(&mut input).is_err() {
        return 0;
    }
    if let Some(snap) = parse_statusline(&input, now_secs()) {
        let _ = write_snapshot(&paths::snapshot_path(&base, slot), &snap);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"model":{"display_name":"x"},"rate_limits":{
        "five_hour":{"used_percentage":23.5,"resets_at":1738425600},
        "seven_day":{"used_percentage":41.2,"resets_at":1738857600}}}"#;

    #[test]
    fn parses_documented_schema() {
        let s = parse_statusline(SAMPLE, 10).unwrap();
        assert_eq!(s.five_hour.unwrap().used_pct, 23.5);
        assert_eq!(s.seven_day.unwrap().resets_at, 1738857600);
        assert_eq!(s.captured_at, 10);
    }

    #[test]
    fn missing_or_garbage_rate_limits_yield_nothing() {
        assert!(parse_statusline(r#"{"model":{}}"#, 1).is_none());
        assert!(parse_statusline("not json", 1).is_none());
        assert!(parse_statusline(r#"{"rate_limits":{"five_hour":{"used_percentage":"x","resets_at":1}}}"#, 1).is_none());
    }

    #[test]
    fn one_window_alone_is_kept_and_other_stays_absent() {
        let s = parse_statusline(r#"{"rate_limits":{"seven_day":{"used_percentage":5,"resets_at":99}}}"#, 1).unwrap();
        assert!(s.five_hour.is_none() && s.seven_day.is_some());
    }

    #[test]
    fn percentage_is_clamped() {
        let s = parse_statusline(r#"{"rate_limits":{"five_hour":{"used_percentage":140,"resets_at":99}}}"#, 1).unwrap();
        assert_eq!(s.five_hour.unwrap().used_pct, 100.0);
    }

    #[test]
    fn snapshot_roundtrip_and_isolation_between_slots() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (Slot::new(1).unwrap(), Slot::new(2).unwrap());
        let s = parse_statusline(SAMPLE, 10).unwrap();
        write_snapshot(&paths::snapshot_path(dir.path(), a), &s).unwrap();
        assert_eq!(read_snapshot(&paths::snapshot_path(dir.path(), a)).unwrap(), Some(s));
        assert_eq!(read_snapshot(&paths::snapshot_path(dir.path(), b)).unwrap(), None);
    }

    #[test]
    fn corrupt_snapshot_is_an_error_not_silent_zero() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("usage.json");
        std::fs::write(&p, b"{broken").unwrap();
        assert!(read_snapshot(&p).is_err());
    }

    #[test]
    fn statusline_refuses_foreign_config_dir() {
        let base = Path::new("C:/data/app");
        let (a, b) = (Slot::new(1).unwrap(), Slot::new(2).unwrap());
        let dir_b = paths::config_dir(base, b).to_string_lossy().to_string();
        assert!(env_matches_slot(base, b, Some(&dir_b)));
        assert!(!env_matches_slot(base, a, Some(&dir_b)));
        assert!(env_matches_slot(base, a, None));
    }
}
