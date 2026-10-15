//! Local cache so the last known values survive restarts (flagged with their
//! capture time; never presented as live).

use crate::alerts::AlertMemory;
use crate::model::UsageSnapshot;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountCache {
    pub email: Option<String>,
    pub plan: Option<String>,
    pub was_connected: bool,
    pub usage: Option<UsageSnapshot>,
    pub alert_five_hour: Option<AlertMemory>,
    pub alert_seven_day: Option<AlertMemory>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Cache {
    pub accounts: Vec<AccountCache>,
}

impl Default for Cache {
    fn default() -> Self {
        Cache { accounts: vec![AccountCache::default(), AccountCache::default()] }
    }
}

impl Cache {
    /// A missing or corrupt file yields an empty cache (never an error).
    pub fn load(path: &Path) -> Cache {
        let mut cache: Cache = std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        cache.accounts.resize_with(2, AccountCache::default);
        cache
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        paths::write_atomic(path, &serde_json::to_vec(self).map_err(std::io::Error::other)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survives_restart_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cache.json");
        let mut c = Cache::default();
        c.accounts[1].email = Some("work@x.com".into());
        c.accounts[1].was_connected = true;
        c.save(&p).unwrap();
        let back = Cache::load(&p);
        assert_eq!(back.accounts[1].email.as_deref(), Some("work@x.com"));
        assert!(back.accounts[0].email.is_none());
    }

    #[test]
    fn corrupt_or_short_cache_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cache.json");
        std::fs::write(&p, b"garbage").unwrap();
        assert_eq!(Cache::load(&p).accounts.len(), 2);
        std::fs::write(&p, br#"{"accounts":[{"was_connected":true}]}"#).unwrap();
        let c = Cache::load(&p);
        assert_eq!(c.accounts.len(), 2);
        assert!(c.accounts[0].was_connected);
    }
}
