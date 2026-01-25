// 設定ファイル読み込みサービス
//
// core::config の純粋性を保つため、ファイルI/Oはこのサービスに集約する。

use crate::core::config::Config;
use anyhow::{Context, Result};
use serde_saphyr;
use std::path::Path;

/// 設定ファイル読み込みサービス
#[derive(Debug, Clone, Default)]
pub struct ConfigLoader;

impl ConfigLoader {
    /// YAMLファイルから設定を読み込む
    pub fn from_file(path: &Path) -> Result<Config> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;
        serde_saphyr::from_str(&content).with_context(|| "Failed to parse config file")
    }

    /// デフォルトパスから設定を読み込む
    pub fn load_default() -> Result<Config> {
        let path = Path::new(Config::DEFAULT_CONFIG_PATH);
        Self::from_file(path)
    }
}
