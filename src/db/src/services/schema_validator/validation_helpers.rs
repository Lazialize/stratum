// バリデーションヘルパー関数
//
// バリデーター間で共通のチェックパターンをユーティリティ関数として提供します。

use crate::core::error::{ErrorLocation, ValidationError, ValidationResult};
use crate::core::schema::Table;

/// カラムの存在を確認し、存在しない場合はReferenceエラーをresultに追加する。
///
/// # Arguments
///
/// * `table` - 検証対象のテーブル
/// * `table_name` - テーブル名（エラーメッセージ用）
/// * `column_name` - 確認対象のカラム名
/// * `result` - エラーを追加するValidationResult
/// * `context` - エラーメッセージに含めるコンテキスト（例: "Index 'idx_foo' references"）
///
/// # Returns
///
/// カラムが存在する場合はtrue、存在しない場合はfalse
pub fn check_column_exists(
    table: &Table,
    table_name: &str,
    column_name: &str,
    result: &mut ValidationResult,
    context: &str,
) -> bool {
    if table.get_column(column_name).is_none() {
        result.add_error(ValidationError::Reference {
            message: format!(
                "{} column '{}' which does not exist in table '{}'",
                context, column_name, table_name
            ),
            location: Some(ErrorLocation::with_table_and_column(
                table_name,
                column_name,
            )),
            suggestion: Some(format!("Define column '{}'", column_name)),
        });
        false
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{Column, ColumnType, Table};

    #[test]
    fn test_check_column_exists_found() {
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        let mut result = ValidationResult::new();
        let exists = check_column_exists(&table, "users", "id", &mut result, "Test references");

        assert!(exists);
        assert!(result.is_valid());
    }

    #[test]
    fn test_check_column_exists_not_found() {
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        let mut result = ValidationResult::new();
        let exists = check_column_exists(
            &table,
            "users",
            "nonexistent",
            &mut result,
            "Index 'idx_test' references",
        );

        assert!(!exists);
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
        let error = &result.errors[0];
        assert!(error.is_reference());
        assert!(error.to_string().contains("nonexistent"));
        assert!(error.to_string().contains("users"));
        assert!(error.to_string().contains("Index 'idx_test' references"));
    }
}
