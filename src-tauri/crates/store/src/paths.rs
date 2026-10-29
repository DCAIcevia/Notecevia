//! 把文件里记的地址变成本机路径。
//!
//! 地址来自磁盘，不能直接当路径用：绝对路径、Windows 盘符和 `..` 一律拒绝，
//! 否则一个改过的画板文件就能让程序读写工作区之外的地方。

use std::path::{Component, Path, PathBuf};

/// 是不是工作区内的相对地址。空串也不算。
pub(crate) fn is_inside(reference: &str) -> bool {
    if reference.is_empty() || reference.contains(':') {
        return false;
    }
    let path = Path::new(reference);
    if path.is_absolute() {
        return false;
    }
    !path.components().any(|part| {
        matches!(
            part,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

/// 把相对地址接到工作区根下。不在工作区内的地址给 None。
pub(crate) fn inside(root: &Path, reference: &str) -> Option<PathBuf> {
    if !is_inside(reference) {
        return None;
    }
    Some(root.join(Path::new(reference)))
}

#[cfg(test)]
mod tests {
    use super::is_inside;

    #[test]
    fn workspace_relative_addresses_are_accepted() {
        assert!(is_inside("blocks/1-2.block"));
        assert!(is_inside("media/abc"));
    }

    #[test]
    fn escapes_are_refused() {
        assert!(!is_inside("../../secret"));
        assert!(!is_inside("blocks/../../secret"));
        assert!(!is_inside("C:/Windows/system32"));
        assert!(!is_inside("/etc/passwd"));
        assert!(!is_inside("//server/share"));
        assert!(!is_inside(""));
    }
}
