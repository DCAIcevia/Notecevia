//! 集成测试共用的临时工作区。用系统临时目录，跑完就删，不碰真实用户目录。

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use notecevia_store::Workspace;

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempWorkspace {
    pub root: PathBuf,
    pub workspace: Workspace,
    tag: String,
}

impl TempWorkspace {
    pub fn new(name: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let tag = format!("notecevia-store-{name}-{}-{unique}", std::process::id());
        let root = std::env::temp_dir().join(&tag);
        let _ = fs::remove_dir_all(&root);
        let workspace = Workspace::open(&root).expect("建立临时工作区");
        Self {
            root,
            workspace,
            tag,
        }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// 工作区外面的一个唯一路径，测路径逃逸用。
    pub fn outside_path(&self, name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{}-outside-{name}", self.tag))
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// 递归找某个后缀的文件。
pub fn files_with_extension(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
                found.push(path);
            }
        }
    }
    found
}
/// 把目录整个拷到另一个目录，源目录不动。
pub fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
