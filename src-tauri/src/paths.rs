//! On-disk layout. Each account owns a private directory; nothing is shared.
//!
//! ```text
//! %APPDATA%\com.claudeusage.tray\
//!   settings.json            (tauri-plugin-store)
//!   cache.json               (last known usage/identity per account)
//!   accounts\1\claude\       CLAUDE_CONFIG_DIR of account 1 (Claude Code's own credential store)
//!   accounts\1\usage.json    statusline snapshot of account 1
//!   accounts\2\...
//! ```

use crate::model::Slot;
use std::path::{Path, PathBuf};

pub const APP_DIR: &str = "com.claudeusage.tray";

/// Resolved without Tauri so the `--statusline` helper process can use it too.
pub fn base_dir() -> PathBuf {
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    appdata.join(APP_DIR)
}

pub fn account_dir(base: &Path, slot: Slot) -> PathBuf {
    base.join("accounts").join(slot.number().to_string())
}

pub fn config_dir(base: &Path, slot: Slot) -> PathBuf {
    account_dir(base, slot).join("claude")
}

pub fn snapshot_path(base: &Path, slot: Slot) -> PathBuf {
    account_dir(base, slot).join("usage.json")
}

pub fn cache_path(base: &Path) -> PathBuf {
    base.join("cache.json")
}

/// Atomic write: temp file in the same dir, then rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_never_share_paths() {
        let base = Path::new("C:/base");
        let (a, b) = (Slot::new(1).unwrap(), Slot::new(2).unwrap());
        assert_ne!(config_dir(base, a), config_dir(base, b));
        assert_ne!(snapshot_path(base, a), snapshot_path(base, b));
        assert!(!config_dir(base, a).starts_with(account_dir(base, b)));
        assert!(!snapshot_path(base, b).starts_with(account_dir(base, a)));
    }
}
