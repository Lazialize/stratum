// コマンドハンドラー層
// 各CLIコマンドの実装

pub mod apply;
pub mod destructive_change_formatter;
pub(crate) mod dry_run_formatter;
pub mod export;
pub mod generate;
pub mod init;
pub mod migration_loader;
pub mod rollback;
pub(crate) mod sql_parser;
pub mod status;
pub mod validate;

pub(crate) use sql_parser::split_sql_statements;

use crate::cli::OutputFormat;
use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

/// コマンド出力を構造化するためのトレイト
///
/// 各コマンドの出力構造体はこのトレイトを実装し、
/// テキスト表示とJSONシリアライズの両方をサポートする。
pub trait CommandOutput: Serialize {
    /// 人間向けテキスト表示を生成する
    fn to_text(&self) -> String;
}

/// OutputFormat に応じて出力文字列を生成する
///
/// - `Text`: `CommandOutput::to_text()` を使用
/// - `Json`: `serde_json` でシリアライズ
pub fn render_output<T: CommandOutput>(output: &T, format: &OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Text => Ok(output.to_text()),
        OutputFormat::Json => {
            serde_json::to_string_pretty(output).map_err(|e| anyhow::anyhow!("JSON serialization error: {}", e))
        }
    }
}

/// エラーレスポンスの構造化出力
#[derive(Debug, Clone, Serialize)]
pub struct ErrorOutput {
    /// エラーメッセージ
    pub error: String,
}

impl ErrorOutput {
    /// エラーメッセージから ErrorOutput を作成
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }

    /// JSON 文字列にシリアライズ
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| format!("{{\"error\": \"{}\"}}", self.error))
    }
}

/// 破壊的 SQL 操作を検出するための共通正規表現
///
/// 検出対象:
/// - DROP TABLE/COLUMN/TYPE/INDEX/CONSTRAINT/SCHEMA/DATABASE
/// - ALTER ... DROP/RENAME
/// - RENAME TABLE/COLUMN
/// - TRUNCATE TABLE
/// - DELETE FROM (WHERE句の有無に関わらず潜在的に破壊的)
pub(crate) static DESTRUCTIVE_SQL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(DROP\s+(TABLE|COLUMN|TYPE|INDEX|CONSTRAINT|SCHEMA|DATABASE)|ALTER\s+.*\s+(DROP|RENAME)|RENAME\s+(TABLE|COLUMN)|TRUNCATE\s+TABLE|DELETE\s+FROM)\b")
        .expect("Invalid destructive SQL regex pattern")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_output_json_serialization() {
        let error = ErrorOutput::new("Config file not found");
        let json = error.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["error"], "Config file not found");
    }

    #[test]
    fn test_render_output_text_mode() {
        #[derive(Debug, Serialize)]
        struct TestOutput {
            value: i32,
        }
        impl CommandOutput for TestOutput {
            fn to_text(&self) -> String {
                format!("value is {}", self.value)
            }
        }

        let output = TestOutput { value: 42 };
        let result = render_output(&output, &OutputFormat::Text).unwrap();
        assert_eq!(result, "value is 42");
    }

    #[test]
    fn test_render_output_json_mode() {
        #[derive(Debug, Serialize)]
        struct TestOutput {
            value: i32,
            #[serde(skip)]
            text: String,
        }
        impl CommandOutput for TestOutput {
            fn to_text(&self) -> String {
                self.text.clone()
            }
        }

        let output = TestOutput {
            value: 42,
            text: "should not appear".to_string(),
        };
        let result = render_output(&output, &OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        assert_eq!(parsed["value"], 42);
        assert!(parsed.get("text").is_none());
    }
}
