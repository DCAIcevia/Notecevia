//! 应用配置：跟着程序走，不跟工作区走。
//!
//! 位置由操作系统的配置目录决定，不写进工作区：换工作区不该换掉行为策略，
//! 工作区是可备份、可删除的数据单位，配置混进去会被当成数据一起搬。
//!
//! 配置不是内容：文件缺失、读不懂，都退回默认值，不挡程序打开。
//! 因此这里不保留未知键——配置随程序版本走，新键由缺省补，旧键由代码删。

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// 自动保存的策略。缺省值与 `docs/design/architecture.md` 记的节拍一致。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutosavePolicy {
    /// 有改动时多久落一次盘，毫秒。
    pub interval_ms: u64,
    /// 缩略图最短重画间隔，毫秒。
    pub thumbnail_interval_ms: u64,
}

impl Default for AutosavePolicy {
    fn default() -> Self {
        Self {
            interval_ms: 2000,
            thumbnail_interval_ms: 5000,
        }
    }
}

/// 应用配置。新增一类策略时加一个字段，缺省值写在各段的 `Default` 里。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub autosave: AutosavePolicy,
}

impl AppConfig {
    /// 读配置。文件不存在或读不懂都退回默认值。
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// 写配置。先写临时文件、落盘后改名，与工作区存盘同一口径。
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let temporary = path.with_extension("json.tmp");
        {
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("notecevia-config-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("建临时目录");
        let path = directory.join("config.json");
        std::fs::remove_file(&path).ok();
        path
    }

    #[test]
    fn missing_file_gives_defaults() {
        let path = temp_file("missing");
        assert_eq!(AppConfig::load(&path), AppConfig::default());
        assert_eq!(AppConfig::load(&path).autosave.interval_ms, 2000);
    }

    #[test]
    fn broken_file_gives_defaults() {
        let path = temp_file("broken");
        std::fs::write(&path, "{ 这不是 JSON").expect("写坏文件");
        assert_eq!(AppConfig::load(&path), AppConfig::default());
    }

    #[test]
    fn a_partial_file_keeps_the_other_defaults() {
        let path = temp_file("partial");
        std::fs::write(&path, r#"{"autosave":{"intervalMs":500}}"#).expect("写半份配置");
        let config = AppConfig::load(&path);
        assert_eq!(config.autosave.interval_ms, 500);
        assert_eq!(config.autosave.thumbnail_interval_ms, 5000);
    }

    #[test]
    fn saving_round_trips_and_leaves_no_temporary_file() {
        let path = temp_file("roundtrip");
        let mut config = AppConfig::default();
        config.autosave.interval_ms = 750;
        config.save(&path).expect("写配置");

        assert_eq!(AppConfig::load(&path), config);
        assert!(
            !path.with_extension("json.tmp").exists(),
            "保存后不该留下临时文件"
        );
    }
}
