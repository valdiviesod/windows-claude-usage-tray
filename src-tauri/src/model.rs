//! Shared data types. Everything the frontend sees is defined here.

use serde::{Deserialize, Serialize};

pub const NEAR_LIMIT_PCT: f64 = 80.0;

/// Account slots are fixed: exactly two, never mixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Slot(u8);

impl Slot {
    pub const ALL: [Slot; 2] = [Slot(1), Slot(2)];

    pub fn new(n: u8) -> Option<Slot> {
        (1..=2).contains(&n).then_some(Slot(n))
    }

    pub fn number(self) -> u8 {
        self.0
    }

    pub fn index(self) -> usize {
        (self.0 - 1) as usize
    }

    pub fn label(self) -> &'static str {
        if self.0 == 1 {
            "Personal"
        } else {
            "Trabajo"
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LimitWindow {
    /// 0..=100
    pub used_pct: f64,
    /// Unix epoch seconds
    pub resets_at: i64,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct UsageSnapshot {
    /// Unix epoch seconds when the data was observed.
    pub captured_at: i64,
    /// "statusline" (Claude Code official statusline data) or "probe".
    pub source: String,
    pub five_hour: Option<LimitWindow>,
    pub seven_day: Option<LimitWindow>,
}

impl UsageSnapshot {
    /// Drop windows whose reset time has already passed: their numbers belong
    /// to a finished window and must not be presented as current.
    pub fn live(&self, now: i64) -> UsageSnapshot {
        UsageSnapshot {
            captured_at: self.captured_at,
            source: self.source.clone(),
            five_hour: self.five_hour.filter(|w| w.resets_at > now),
            seven_day: self.seven_day.filter(|w| w.resets_at > now),
        }
    }

    pub fn has_windows(&self) -> bool {
        self.five_hour.is_some() || self.seven_day.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Connection {
    Disconnected,
    /// Startup: previously connected, verifying with the official client.
    Checking,
    /// Waiting for the user to finish the official sign-in flow.
    Connecting,
    Connected,
    /// Was connected before, but the official client reports no valid session.
    Expired,
    /// Could not determine (Claude Code missing, timeout, ...).
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageStatus {
    Available,
    NearLimit,
    Exhausted,
    NoData,
    Disconnected,
}

pub fn compute_status(conn: Connection, usage: Option<&UsageSnapshot>, now: i64) -> UsageStatus {
    if conn != Connection::Connected {
        return UsageStatus::Disconnected;
    }
    let live = usage.and_then(|u| u.five_hour).filter(|w| w.resets_at > now);
    match live {
        None => UsageStatus::NoData,
        Some(w) if w.used_pct >= 100.0 => UsageStatus::Exhausted,
        Some(w) if w.used_pct >= NEAR_LIMIT_PCT => UsageStatus::NearLimit,
        Some(_) => UsageStatus::Available,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AccountView {
    pub slot: u8,
    pub label: &'static str,
    pub connection: Connection,
    pub email: Option<String>,
    pub plan: Option<String>,
    pub usage: Option<UsageSnapshot>,
    pub status: UsageStatus,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AppView {
    pub now: i64,
    pub accounts: Vec<AccountView>,
    pub settings: crate::settings::Settings,
    pub syncing: bool,
    pub last_sync_at: Option<i64>,
    pub claude_found: bool,
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(pct: f64, resets_at: i64) -> UsageSnapshot {
        UsageSnapshot {
            captured_at: 0,
            source: "statusline".into(),
            five_hour: Some(LimitWindow { used_pct: pct, resets_at }),
            seven_day: None,
        }
    }

    #[test]
    fn slot_rejects_out_of_range() {
        assert!(Slot::new(0).is_none());
        assert!(Slot::new(3).is_none());
        assert_eq!(Slot::new(2).unwrap().index(), 1);
    }

    #[test]
    fn status_thresholds() {
        let c = Connection::Connected;
        assert_eq!(compute_status(c, Some(&snap(10.0, 200)), 100), UsageStatus::Available);
        assert_eq!(compute_status(c, Some(&snap(80.0, 200)), 100), UsageStatus::NearLimit);
        assert_eq!(compute_status(c, Some(&snap(100.0, 200)), 100), UsageStatus::Exhausted);
    }

    #[test]
    fn expired_window_is_no_data_not_invented() {
        let c = Connection::Connected;
        assert_eq!(compute_status(c, Some(&snap(95.0, 50)), 100), UsageStatus::NoData);
        assert!(snap(95.0, 50).live(100).five_hour.is_none());
        assert_eq!(compute_status(c, None, 100), UsageStatus::NoData);
    }

    #[test]
    fn disconnected_never_reports_usage_status() {
        for c in [Connection::Disconnected, Connection::Checking, Connection::Connecting, Connection::Expired, Connection::Error] {
            assert_eq!(compute_status(c, Some(&snap(10.0, 200)), 100), UsageStatus::Disconnected);
        }
    }
}
