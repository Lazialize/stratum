// 設定ファイル読み込みサービス
//
// core::config の純粋性を保つため、ファイルI/Oはこのサービスに集約する。

use crate::core::config::Config;
use anyhow::{Context, Result};
use regex::Regex;
use serde_saphyr;
use std::path::Path;

/// 設定ファイル読み込みサービス
#[derive(Debug, Clone, Default)]
pub struct ConfigLoader;

impl ConfigLoader {
    /// YAMLファイルから設定を読み込む
    ///
    /// 設定値内の `${ENV_VAR}` パターンを環境変数の値で展開します。
    /// 環境変数が未定義の場合は空文字列に置換されます。
    pub fn from_file(path: &Path) -> Result<Config> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;
        let expanded = Self::expand_env_vars(&content);
        serde_saphyr::from_str(&expanded).with_context(|| "Failed to parse config file")
    }

    /// デフォルトパスから設定を読み込む
    pub fn load_default() -> Result<Config> {
        let path = Path::new(Config::DEFAULT_CONFIG_PATH);
        Self::from_file(path)
    }

    /// 文字列内の `${ENV_VAR}` パターンを環境変数の値で展開
    ///
    /// 環境変数が未定義の場合は空文字列に置換し、警告を出力します。
    fn expand_env_vars(content: &str) -> String {
        let re = Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex");
        re.replace_all(content, |caps: &regex::Captures| {
            let var_name = &caps[1];
            match std::env::var(var_name) {
                Ok(value) => value,
                Err(_) => {
                    eprintln!(
                        "Warning: Environment variable '{}' is not defined, using empty string",
                        var_name
                    );
                    String::new()
                }
            }
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_expand_env_vars_replaces_known_var() {
        std::env::set_var("TEST_STRATUM_VAR", "hello");
        let result = ConfigLoader::expand_env_vars("password: ${TEST_STRATUM_VAR}");
        assert_eq!(result, "password: hello");
        std::env::remove_var("TEST_STRATUM_VAR");
    }

    #[test]
    #[serial]
    fn test_expand_env_vars_unknown_var_becomes_empty() {
        std::env::remove_var("NONEXISTENT_STRATUM_VAR");
        let result = ConfigLoader::expand_env_vars("password: ${NONEXISTENT_STRATUM_VAR}");
        assert_eq!(result, "password: ");
    }

    #[test]
    fn test_expand_env_vars_no_vars() {
        let input = "password: plain_text";
        let result = ConfigLoader::expand_env_vars(input);
        assert_eq!(result, input);
    }

    #[test]
    #[serial]
    fn test_expand_env_vars_multiple() {
        std::env::set_var("TEST_HOST", "myhost");
        std::env::set_var("TEST_PORT", "5432");
        let result = ConfigLoader::expand_env_vars("host: ${TEST_HOST}\nport: ${TEST_PORT}");
        assert_eq!(result, "host: myhost\nport: 5432");
        std::env::remove_var("TEST_HOST");
        std::env::remove_var("TEST_PORT");
    }
}
