// カラム型の検証

use crate::core::error::{ErrorLocation, ValidationError, ValidationResult, ValidationWarning};
use crate::core::schema::{ColumnType, Schema};

/// 既知のColumnType kind値（大文字）
const KNOWN_COLUMN_TYPES: &[&str] = &[
    "INTEGER",
    "VARCHAR",
    "TEXT",
    "BOOLEAN",
    "TIMESTAMP",
    "JSON",
    "DECIMAL",
    "FLOAT",
    "DOUBLE",
    "CHAR",
    "DATE",
    "TIME",
    "BLOB",
    "UUID",
    "JSONB",
    "ENUM",
];

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

            // DialectSpecificのkind値の検証
            if let ColumnType::DialectSpecific { kind, params } = &column.column_type {
                let kind_upper = kind.to_uppercase();

                // MySQL ENUM with values is a valid dialect-specific type
                // Skip warning if kind is "ENUM" and params contains "values"
                let is_mysql_enum_with_values =
                    kind_upper == "ENUM" && params.get("values").is_some_and(|v| v.is_array());

                if KNOWN_COLUMN_TYPES.contains(&kind_upper.as_str()) {
                    if !is_mysql_enum_with_values {
                        result.add_warning(ValidationWarning::possible_typo(
                            format!(
                                "Column '{}.{}' uses DialectSpecific with kind '{}' which matches a known type. Did you mean to use ColumnType::{}?",
                                table_name, column.name, kind, kind_upper
                            ),
                            Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
                        ));
                    }
                } else {
                    // 未知の型名の場合、データベース実行時まで検証されない旨の警告を出す
                    result.add_warning(ValidationWarning::dialect_specific(
                        format!(
                            "Column '{}.{}' uses dialect-specific type '{}'. This type is not validated by Strata and will be passed through to the database as-is.",
                            table_name, column.name, kind
                        ),
                        Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
                    ));
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
        ColumnType::VARCHAR { length } => {
            // length の範囲チェック（1-65535）
            if *length == 0 {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "VARCHAR type in column '{}.{}' has invalid length (0)",
                        table_name, column_name
                    ),
                    location: Some(ErrorLocation::with_table_and_column(
                        table_name,
                        column_name,
                    )),
                    suggestion: Some("Set length to at least 1".to_string()),
                });
            }

            if *length > 65535 {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "VARCHAR type in column '{}.{}' has length ({}) exceeding maximum (65535)",
                        table_name, column_name, length
                    ),
                    location: Some(ErrorLocation::with_table_and_column(
                        table_name,
                        column_name,
                    )),
                    suggestion: Some(
                        "Set length to 65535 or less, or use TEXT for longer strings".to_string(),
                    ),
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

    #[test]
    fn test_validate_varchar_type_zero_length() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 0 }, // length = 0 はエラー
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
    fn test_validate_varchar_type_excessive_length() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 70000 }, // length > 65535 はエラー
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
            .contains("length (70000) exceeding maximum (65535)"));
    }

    #[test]
    fn test_validate_varchar_type_valid() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 }, // 有効な長さ
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_dialect_specific_with_known_type_warns() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        // "INTGER"のようなタイプミスはDialectSpecificとしてパースされるが、
        // "INTEGER"と類似しているため警告を出すべき
        // ここでは"INTEGER"（既知の型）をDialectSpecificとして指定した場合を検証
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::DialectSpecific {
                kind: "integer".to_string(), // 小文字でも既知の型として検出
                params: serde_json::json!({}),
            },
            false,
        ));
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(result.is_valid()); // エラーではない
        assert!(result.warning_count() > 0);
        assert!(result.warnings[0].message.contains("matches a known type"));
    }

    #[test]
    fn test_validate_dialect_specific_with_unknown_type_warns() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        // "SERIAL"のような方言固有の型は警告を出す
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::DialectSpecific {
                kind: "SERIAL".to_string(),
                params: serde_json::json!({}),
            },
            false,
        ));
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(result.is_valid()); // エラーではない
        assert!(result.warning_count() > 0);
        assert!(result.warnings[0].message.contains("dialect-specific type"));
        assert!(result.warnings[0].message.contains("not validated"));
    }

    #[test]
    fn test_validate_mysql_enum_with_values_no_warning() {
        // MySQL ENUM with values is a valid dialect-specific type
        // and should NOT trigger the "matches a known type" warning
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("posts".to_string());
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::DialectSpecific {
                kind: "ENUM".to_string(),
                params: serde_json::json!({
                    "values": ["draft", "published", "archived"]
                }),
            },
            false,
        ));
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(result.is_valid());
        assert_eq!(
            result.warning_count(),
            0,
            "MySQL ENUM with values should not trigger any warning"
        );
    }

    #[test]
    fn test_validate_dialect_specific_enum_without_values_warns() {
        // DialectSpecific ENUM without values should still warn
        // (this could be a typo for ColumnType::Enum)
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::DialectSpecific {
                kind: "ENUM".to_string(),
                params: serde_json::json!({}),
            },
            false,
        ));
        schema.add_table(table);

        let result = validate_column_types(&schema);

        assert!(result.is_valid());
        assert!(result.warning_count() > 0);
        assert!(result.warnings[0].message.contains("matches a known type"));
    }
}
