// テーブル構造の検証

use crate::core::error::{ErrorLocation, ValidationError, ValidationResult};
use crate::core::schema::Schema;

/// テーブル構造の検証（カラムの存在確認）
pub fn validate_table_structure(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (table_name, table) in &schema.tables {
        if table.columns.is_empty() {
            result.add_error(ValidationError::Constraint {
                message: format!("Table '{}' has no columns defined", table_name),
                location: Some(ErrorLocation::with_table(table_name.clone())),
                suggestion: Some("Define at least one column".to_string()),
            });
        }
    }

    result
}
