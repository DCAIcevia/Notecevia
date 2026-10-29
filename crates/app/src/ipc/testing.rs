//! 测试用的临时工作区：每个用例一个目录，跑完删掉。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use notecevia_store::Workspace;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 一个用完即删的工作区。
pub(crate) struct TempWorkspace {
    path: PathBuf,
    pub(crate) workspace: Workspace,
}

impl TempWorkspace {
    pub(crate) fn new(label: &str) -> Self {
        let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "notecevia-ipc-{label}-{}-{serial}",
            std::process::id()
        ));
        let workspace = Workspace::open(&path).expect("建临时工作区");
        Self { path, workspace }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
