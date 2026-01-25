// 設定ファイル書き出しサービス
//
// core::config の純粋性を保つため、YAMLへの直列化はこのサービスに集約する。

use crate::core::config::Config;
use anyhow::{Context, Result};

/// 設定ファイル書き出しサービス
#[derive(Debug, Clone, Default)]
pub struct ConfigSerializer;

impl ConfigSerializer {
    /// ConfigをYAML文字列に変換
    pub fn to_yaml(config: &Config) -> Result<String> {
        serde_saphyr::to_string(config).with_context(|| "Failed to serialize config file")
    }
}
