//! Threshold notifications (80 / 95 / 100 %), at most once per level per
//! rate-limit window, and the small pure helpers around timing.

use crate::model::LimitWindow;
use serde::{Deserialize, Serialize};

pub const LEVELS: [u8; 3] = [80, 95, 100];

/// Two observations whose reset times differ by less than this belong to the
/// same window (different sources round differently).
const SAME_WINDOW_TOLERANCE_SECS: i64 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlertMemory {
    pub resets_at: i64,
    pub level: u8,
}

pub fn level_for(pct: f64) -> u8 {
    LEVELS.iter().rev().copied().find(|l| pct >= f64::from(*l)).unwrap_or(0)
}

/// Returns the newly crossed level, if any, and updates `mem`.
pub fn next_alert(mem: &mut Option<AlertMemory>, w: &LimitWindow) -> Option<u8> {
    let same_window = mem.is_some_and(|m| (m.resets_at - w.resets_at).abs() < SAME_WINDOW_TOLERANCE_SECS);
    let previous = if same_window { mem.map_or(0, |m| m.level) } else { 0 };
    let level = level_for(w.used_pct);
    *mem = Some(AlertMemory { resets_at: w.resets_at, level: level.max(previous) });
    (level > previous).then_some(level)
}

/// Exponential backoff: `base * 2^fails`, capped.
pub fn backoff_secs(base: u64, fails: u32, cap: u64) -> u64 {
    base.saturating_mul(1u64 << fails.min(20)).min(cap)
}

/// "2h 10m", "45m", "<1m".
pub fn fmt_duration(secs: i64) -> String {
    let secs = secs.max(0);
    let (d, h, m) = (secs / 86_400, (secs % 86_400) / 3600, (secs % 3600) / 60);
    match (d, h, m) {
        (0, 0, 0) => "<1m".into(),
        (0, 0, m) => format!("{m}m"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(pct: f64, resets_at: i64) -> LimitWindow {
        LimitWindow { used_pct: pct, resets_at }
    }

    #[test]
    fn levels() {
        assert_eq!(level_for(79.9), 0);
        assert_eq!(level_for(80.0), 80);
        assert_eq!(level_for(96.0), 95);
        assert_eq!(level_for(100.0), 100);
    }

    #[test]
    fn each_level_fires_once_per_window() {
        let mut mem = None;
        assert_eq!(next_alert(&mut mem, &w(50.0, 10_000)), None);
        assert_eq!(next_alert(&mut mem, &w(81.0, 10_000)), Some(80));
        assert_eq!(next_alert(&mut mem, &w(85.0, 10_000)), None);
        assert_eq!(next_alert(&mut mem, &w(96.0, 10_020)), Some(95));
        assert_eq!(next_alert(&mut mem, &w(100.0, 10_020)), Some(100));
        assert_eq!(next_alert(&mut mem, &w(100.0, 10_020)), None);
    }

    #[test]
    fn new_window_rearms_alerts() {
        let mut mem = None;
        assert_eq!(next_alert(&mut mem, &w(90.0, 10_000)), Some(80));
        assert_eq!(next_alert(&mut mem, &w(10.0, 10_000 + 5 * 3600)), None);
        assert_eq!(next_alert(&mut mem, &w(82.0, 10_000 + 5 * 3600)), Some(80));
    }

    #[test]
    fn backoff_is_exponential_and_capped() {
        assert_eq!(backoff_secs(30, 0, 1800), 30);
        assert_eq!(backoff_secs(30, 3, 1800), 240);
        assert_eq!(backoff_secs(30, 30, 1800), 1800);
    }

    #[test]
    fn duration_format() {
        assert_eq!(fmt_duration(20), "<1m");
        assert_eq!(fmt_duration(45 * 60), "45m");
        assert_eq!(fmt_duration(2 * 3600 + 10 * 60), "2h 10m");
        assert_eq!(fmt_duration(3 * 86_400 + 5 * 3600), "3d 5h");
        assert_eq!(fmt_duration(-5), "<1m");
    }
}
