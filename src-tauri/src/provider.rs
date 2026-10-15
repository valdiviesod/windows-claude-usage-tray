//! Interchangeable usage sources. Each provider reads exactly one account's
//! data (via `AccountCtx`) and never touches the other account's paths.

use crate::claude_cli;
use crate::model::{Slot, UsageSnapshot};
use crate::{paths, snapshot};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Debug)]
pub struct AccountCtx {
    pub slot: Slot,
    pub base: PathBuf,
    pub claude: Option<PathBuf>,
}

impl AccountCtx {
    pub fn config_dir(&self) -> PathBuf {
        paths::config_dir(&self.base, self.slot)
    }
    pub fn account_dir(&self) -> PathBuf {
        paths::account_dir(&self.base, self.slot)
    }
}

pub trait UsageProvider: Send + Sync {
    fn id(&self) -> &'static str;
    /// `Ok(None)`: the source has no data (yet). Never fabricates values.
    fn fetch<'a>(&'a self, ctx: &'a AccountCtx) -> BoxFut<'a, Result<Option<UsageSnapshot>, String>>;
}

/// Free, local, official: data Claude Code itself reports to its statusline.
pub struct StatuslineProvider;

impl UsageProvider for StatuslineProvider {
    fn id(&self) -> &'static str {
        "statusline"
    }
    fn fetch<'a>(&'a self, ctx: &'a AccountCtx) -> BoxFut<'a, Result<Option<UsageSnapshot>, String>> {
        Box::pin(async move { snapshot::read_snapshot(&paths::snapshot_path(&ctx.base, ctx.slot)) })
    }
}

/// Opt-in: one minimal request through the official binary. Costs quota.
pub struct ProbeProvider;

impl UsageProvider for ProbeProvider {
    fn id(&self) -> &'static str {
        "probe"
    }
    fn fetch<'a>(&'a self, ctx: &'a AccountCtx) -> BoxFut<'a, Result<Option<UsageSnapshot>, String>> {
        Box::pin(async move {
            let claude = ctx.claude.as_deref().ok_or("Claude Code no está instalado")?;
            claude_cli::probe(claude, &ctx.config_dir(), &ctx.account_dir()).await.map(Some)
        })
    }
}

/// The freshest observation wins; ties prefer `a`.
pub fn newest(a: Option<UsageSnapshot>, b: Option<UsageSnapshot>) -> Option<UsageSnapshot> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if y.captured_at > x.captured_at { y } else { x }),
        (x, None) => x,
        (None, y) => y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(at: i64) -> UsageSnapshot {
        UsageSnapshot { captured_at: at, ..Default::default() }
    }

    #[test]
    fn newest_picks_latest_capture() {
        assert_eq!(newest(Some(s(1)), Some(s(2))).unwrap().captured_at, 2);
        assert_eq!(newest(Some(s(3)), Some(s(2))).unwrap().captured_at, 3);
        assert_eq!(newest(None, Some(s(2))).unwrap().captured_at, 2);
        assert!(newest(None, None).is_none());
    }

    #[test]
    fn statusline_provider_reads_only_its_own_slot() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = |n| AccountCtx { slot: Slot::new(n).unwrap(), base: dir.path().into(), claude: None };
        let snap = UsageSnapshot { captured_at: 7, source: "statusline".into(), ..Default::default() };
        snapshot::write_snapshot(&paths::snapshot_path(dir.path(), Slot::new(1).unwrap()), &snap).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        assert_eq!(rt.block_on(StatuslineProvider.fetch(&ctx(1))).unwrap(), Some(snap));
        assert_eq!(rt.block_on(StatuslineProvider.fetch(&ctx(2))).unwrap(), None);
    }
}
