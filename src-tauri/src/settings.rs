use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Background refresh period in seconds (local snapshot read).
    pub interval_secs: u64,
    pub notifications: bool,
    pub autostart: bool,
    /// Opt-in: also ask the official `claude` binary for rate-limit info.
    /// Each probe is one tiny real request that consumes subscription quota.
    pub probe_enabled: bool,
    pub probe_interval_mins: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            interval_secs: 60,
            notifications: true,
            autostart: true,
            probe_enabled: true,
            probe_interval_mins: 2,
        }
    }
}

impl Settings {
    pub const MIN_INTERVAL_SECS: u64 = 15;
    pub const MIN_PROBE_MINS: u64 = 1;

    pub fn sanitized(mut self) -> Settings {
        self.interval_secs = self.interval_secs.clamp(Self::MIN_INTERVAL_SECS, 3600);
        self.probe_interval_mins = self.probe_interval_mins.clamp(Self::MIN_PROBE_MINS, 1440);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_respects_provider_floor() {
        let s = Settings { interval_secs: 1, probe_interval_mins: 1, ..Default::default() }.sanitized();
        assert_eq!(s.interval_secs, Settings::MIN_INTERVAL_SECS);
        assert_eq!(s.probe_interval_mins, Settings::MIN_PROBE_MINS);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"notifications":false}"#).unwrap();
        assert!(!s.notifications);
        assert_eq!(s.interval_secs, 60);
    }
}
