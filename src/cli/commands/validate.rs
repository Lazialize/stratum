// validateコマンドハンドラー
//
// スキーマ検証機能を実装します。
// - スキーマ定義ファイルの読み込み
// - バリデーションルールの実行
// - エラーと警告のフォーマットされた表示
// - 検証結果のサマリー表示

use crate::core::config::Config;
use crate::services::schema_parser::SchemaParserService;
use crate::services::schema_validator::SchemaValidatorService;
use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

/// validateコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct ValidateCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// スキーマディレクトリのパス（指定されない場合は設定ファイルから取得）
    pub schema_dir: Option<PathBuf>,
}

/// validateコマンドハンドラー
#[derive(Debug, Clone)]
pub struct ValidateCommandHandler {}

impl ValidateCommandHandler {
    /// 新しいValidateCommandHandlerを作成
    pub fn new() -> Self {
        Self {}
    }

    /// validateコマンドを実行
    ///
    /// # Arguments
    ///
    /// * `command` - validateコマンドのパラメータ
    ///
    /// # Returns
    ///
    /// 成功時は検証結果のサマリー、失敗時はエラーメッセージ
    pub fn execute(&self, command: &ValidateCommand) -> Result<String> {
        // 設定ファイルを読み込む
        let config_path = command.project_path.join(Config::DEFAULT_CONFIG_PATH);
        if !config_path.exists() {
            return Err(anyhow!(
                "Config file not found: {:?}. Please initialize the project first with the `init` command.",
                config_path
            ));
        }

        let config = Config::from_file(&config_path)
            .with_context(|| "Failed to read config file")?;

        // スキーマディレクトリのパスを解決
        let schema_dir = if let Some(ref custom_dir) = command.schema_dir {
            custom_dir.clone()
        } else {
            command.project_path.join(&config.schema_dir)
        };

        if !schema_dir.exists() {
            return Err(anyhow!(
                "Schema directory not found: {:?}",
                schema_dir
            ));
        }

        // スキーマ定義を読み込む
        let parser = SchemaParserService::new();
        let schema = parser
            .parse_schema_directory(&schema_dir)
            .with_context(|| "Failed to parse schema")?;

        // スキーマを検証
        let validator = SchemaValidatorService::new();
        let validation_result = validator.validate(&schema);

        // 検証結果を表示用にフォーマット
        let summary = self.format_validation_result(&validation_result, &schema);

        Ok(summary)
    }

    /// 検証結果をフォーマット
    fn format_validation_result(
        &self,
        result: &crate::core::error::ValidationResult,
        schema: &crate::core::schema::Schema,
    ) -> String {
        let mut output = String::new();

        output.push_str("=== Schema Validation Results ===\n\n");

        // エラーの表示
        if !result.errors.is_empty() {
            output.push_str(&format!("❌ {} error(s) found:\n\n", result.errors.len()));

            for (i, error) in result.errors.iter().enumerate() {
                output.push_str(&format!("{}. {}\n", i + 1, error));

                // エラーの場所を表示
                if let Some(location) = self.get_error_location(error) {
                    if let Some(table) = &location.table {
                        output.push_str(&format!("   Location: table '{}'\n", table));
                        if let Some(column) = &location.column {
                            output.push_str(&format!("             column '{}'\n", column));
                        }
                    }
                }

                // 修正案を表示
                if let Some(suggestion) = self.get_error_suggestion(error) {
                    output.push_str(&format!("   Suggestion: {}\n", suggestion));
                }

                output.push('\n');
            }
        }

        // 統計情報の表示
        output.push_str("\n=== Validation Statistics ===\n");
        let stats = self.calculate_statistics(schema);
        output.push_str(&format!("Tables: {}\n", stats.0));
        output.push_str(&format!("Columns: {}\n", stats.1));
        output.push_str(&format!("Indexes: {}\n", stats.2));
        output.push_str(&format!("Constraints: {}\n", stats.3));

        // 結果サマリー
        output.push_str("\n=== Result ===\n");
        if result.is_valid() {
            output.push_str("✓ Validation complete. No errors found.\n");
        } else {
            output.push_str(&format!(
                "✗ Validation complete. {} error(s) found.\n",
                result.errors.len()
            ));
        }

        output
    }

    /// エラーの場所を取得
    fn get_error_location<'a>(&self, error: &'a crate::core::error::ValidationError) -> Option<&'a crate::core::error::ErrorLocation> {
        match error {
            crate::core::error::ValidationError::Syntax { location, .. }
            | crate::core::error::ValidationError::Reference { location, .. }
            | crate::core::error::ValidationError::Constraint { location, .. } => location.as_ref(),
        }
    }

    /// エラーの修正案を取得
    fn get_error_suggestion<'a>(&self, error: &'a crate::core::error::ValidationError) -> Option<&'a str> {
        match error {
            crate::core::error::ValidationError::Syntax { suggestion, .. }
            | crate::core::error::ValidationError::Reference { suggestion, .. }
            | crate::core::error::ValidationError::Constraint { suggestion, .. } => {
                suggestion.as_deref()
            }
        }
    }

    /// スキーマの統計情報を計算
    fn calculate_statistics(&self, schema: &crate::core::schema::Schema) -> (usize, usize, usize, usize) {
        let table_count = schema.table_count();
        let mut column_count = 0;
        let mut index_count = 0;
        let mut constraint_count = 0;

        for table in schema.tables.values() {
            column_count += table.columns.len();
            index_count += table.indexes.len();
            constraint_count += table.constraints.len();
        }

        (table_count, column_count, index_count, constraint_count)
    }

    /// 検証結果のサマリーをフォーマット（テスト用）
    pub fn format_validation_summary(
        &self,
        is_valid: bool,
        error_count: usize,
        warning_count: usize,
        table_count: usize,
        column_count: usize,
        index_count: usize,
        constraint_count: usize,
    ) -> String {
        let mut output = String::new();

        output.push_str("=== Schema Validation Results ===\n\n");

        if error_count > 0 {
            output.push_str(&format!("❌ {} error(s) found\n", error_count));
        }

        if warning_count > 0 {
            output.push_str(&format!("⚠️  {} warning(s) found\n", warning_count));
        }

        output.push_str("\n=== Validation Statistics ===\n");
        output.push_str(&format!("Tables: {}\n", table_count));
        output.push_str(&format!("Columns: {}\n", column_count));
        output.push_str(&format!("Indexes: {}\n", index_count));
        output.push_str(&format!("Constraints: {}\n", constraint_count));

        output.push_str("\n=== Result ===\n");
        if is_valid && error_count == 0 {
            output.push_str("✓ Validation complete. No errors found.\n");
        } else {
            output.push_str(&format!(
                "✗ Validation complete. {} error(s) found.\n",
                error_count
            ));
        }

        output
    }
}

impl Default for ValidateCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_handler() {
        let handler = ValidateCommandHandler::new();
        assert!(format!("{:?}", handler).contains("ValidateCommandHandler"));
    }

    #[test]
    fn test_format_validation_summary() {
        let handler = ValidateCommandHandler::new();

        // エラーなしの場合
        let summary = handler.format_validation_summary(true, 0, 0, 2, 5, 3, 1);
        assert!(summary.contains("Validation complete"));
        assert!(summary.contains("Tables: 2"));
        assert!(summary.contains("No errors found"));

        // エラーありの場合
        let summary = handler.format_validation_summary(false, 3, 1, 2, 5, 3, 1);
        assert!(summary.contains("3 error(s) found"));
        assert!(summary.contains("1 warning(s) found"));
    }

    #[test]
    fn test_calculate_statistics() {
        use crate::core::schema::{Column, ColumnType, Constraint, Schema, Table};

        let handler = ValidateCommandHandler::new();
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let (table_count, column_count, index_count, constraint_count) =
            handler.calculate_statistics(&schema);

        assert_eq!(table_count, 1);
        assert_eq!(column_count, 2);
        assert_eq!(index_count, 0);
        assert_eq!(constraint_count, 1);
    }
}
