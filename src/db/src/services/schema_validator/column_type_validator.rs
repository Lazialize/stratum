// カラム型の検証

use crate::core::error::{ErrorLocation, ValidationError, ValidationResult};
use crate::core::schema::{ColumnType, Schema};

/// カラム型の検証
///
/// - DECIMAL型の精度とスケールの検証
/// - CHAR型の長さの検証
/// - ENUM参照の存在確認
pub fn validate_column_types(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (table_name, table) in &schema.tables {
        for column in &table.columns {
            validate_column_type_internal(
                &column.column_type,
                table_name,
                &column.name,
                &mut result,
            );

            // ENUM参照の存在確認
            if let ColumnType::Enum { name } = &column.column_type {
                if !schema.enums.contains_key(name) {
                    result.add_error(ValidationError::Reference {
                        message: format!(
                            "Column '{}.{}' references undefined ENUM '{}'",
                            table_name, column.name, name
                        ),
                        location: Some(ErrorLocation::with_table_and_column(
                            table_name,
                            &column.name,
                        )),
                        suggestion: Some(format!(
                            "Define ENUM '{}' in the schema enums section",
                            name
                        )),
                    });
                }
            }
        }
    }

    result
}

/// カラムの型固有バリデーション（内部用）
///
/// # Arguments
///
/// * `column_type` - 検証対象のカラム型
/// * `table_name` - テーブル名
/// * `column_name` - カラム名
/// * `result` - バリデーション結果（エラーを追加）
fn validate_column_type_internal(
    column_type: &ColumnType,
    table_name: &str,
    column_name: &str,
    result: &mut ValidationResult,
) {
    match column_type {
        ColumnType::DECIMAL { precision, scale } => {
            // scale <= precision の検証
            if scale > precision {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "DECIMAL type in column '{}.{}' has scale ({}) greater than precision ({})",
                        table_name, column_name, scale, precision
                    ),
                    location: Some(ErrorLocation::with_table_and_column(
                        table_name,
                        column_name,
                    )),
                    suggestion: Some(format!(
                        "Set scale <= precision (e.g., DECIMAL({}, {}))",
                        precision,
                        precision.min(scale)
                    )),
                });
            }

            // precision の範囲チェック（MySQL: 65, PostgreSQL: 1000）
            // 最も厳しい制約（MySQL）を基準とする
            if *precision > 65 {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "DECIMAL type in column '{}.{}' has precision ({}) exceeding maximum (65)",
                        table_name, column_name, precision
                    ),
                    location: Some(ErrorLocation::with_table_and_column(
                        table_name,
                        column_name,
                    )),
                    suggestion: Some(
                        "Set precision to 65 or less for MySQL compatibility".to_string(),
                    ),
                });
            }

            // precision が 0 でないことを検証
            if *precision == 0 {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "DECIMAL type in column '{}.{}' has invalid precision (0)",
                        table_name, column_name
                    ),
                    location: Some(ErrorLocation::with_table_and_column(
                        table_name,
                        column_name,
                    )),
                    suggestion: Some("Set precision to at least 1".to_string()),
                });
            }
        }
        ColumnType::CHAR { length } => {
            // length の範囲チェック（1-255）
            if *length == 0 {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "CHAR type in column '{}.{}' has invalid length (0)",
                        table_name, column_name
                    ),
                    location: Some(ErrorLocation::with_table_and_column(
                        table_name,
                        column_name,
                    )),
                    suggestion: Some("Set length to at least 1".to_string()),
                });
            }

            if *length > 255 {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "CHAR type in column '{}.{}' has length ({}) exceeding maximum (255)",
                        table_name, column_name, length
                    ),
                    location: Some(ErrorLocation::with_table_and_column(
                        table_name,
                        column_name,
                    )),
                    suggestion: Some(
                        "Set length to 255 or less, or use VARCHAR/TEXT for longer strings"
                            .to_string(),
                    ),
                });
            }
        }
        // 他の型は追加のバリデーション不要
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::core::schema::{Column, ColumnType, Constraint, Table};

    use super::*;

    #[test]
    fn test_validate_column_types_decimal_invalid_scale() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 10,
                scale: 15,
            },
            false,
        ));
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("scale")));
    }

    #[test]
    fn test_validate_column_types_enum_reference_missing() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::Enum {
                name: "nonexistent_enum".to_string(),
            },
            false,
        ));
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("undefined ENUM")));
    }

    #[test]
    fn test_validate_decimal_type_invalid_scale() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 10,
                scale: 15, // scale > precision はエラー
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
        assert!(result.errors[0]
            .to_string()
            .contains("scale (15) greater than precision (10)"));
    }

    #[test]
    fn test_validate_decimal_type_zero_precision() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 0, // precision = 0 はエラー
                scale: 0,
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
        assert!(result.errors[0]
            .to_string()
            .contains("invalid precision (0)"));
    }

    #[test]
    fn test_validate_decimal_type_excessive_precision() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 100, // precision > 65 は警告
                scale: 2,
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
        assert!(result.errors[0]
            .to_string()
            .contains("precision (100) exceeding maximum (65)"));
    }

    #[test]
    fn test_validate_char_type_zero_length() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "code".to_string(),
            ColumnType::CHAR { length: 0 }, // length = 0 はエラー
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
        assert!(result.errors[0].to_string().contains("invalid length (0)"));
    }

    #[test]
    fn test_validate_char_type_excessive_length() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "code".to_string(),
            ColumnType::CHAR { length: 300 }, // length > 255 はエラー
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
        assert!(result.errors[0]
            .to_string()
            .contains("length (300) exceeding maximum (255)"));
    }
}
