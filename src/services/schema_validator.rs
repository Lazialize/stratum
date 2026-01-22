// スキーマバリデーターサービス
//
// スキーマ定義の整合性、参照整合性、制約の検証を行うサービス。
// テーブル定義、インデックス、外部キー制約などを検証します。

use crate::core::config::Dialect;
use crate::core::error::{ErrorLocation, ValidationError, ValidationResult, ValidationWarning};
use crate::core::schema::{ColumnType, Constraint, Schema};

/// スキーマバリデーターサービス
///
/// スキーマ定義の検証を行います。
#[derive(Debug, Clone)]
pub struct SchemaValidatorService {
    // 将来的な拡張のためのフィールドを予約
}

impl SchemaValidatorService {
    /// 新しいSchemaValidatorServiceを作成
    pub fn new() -> Self {
        Self {}
    }

    /// スキーマ定義の全体的な検証を実行
    ///
    /// # Arguments
    ///
    /// * `schema` - 検証対象のスキーマ
    ///
    /// # Returns
    ///
    /// 検証結果（エラーのリストを含む）
    pub fn validate(&self, schema: &Schema) -> ValidationResult {
        self.validate_internal(schema, None)
    }

    /// スキーマ定義の全体的な検証を実行（方言指定あり）
    ///
    /// # Arguments
    ///
    /// * `schema` - 検証対象のスキーマ
    /// * `dialect` - データベース方言
    pub fn validate_with_dialect(&self, schema: &Schema, dialect: Dialect) -> ValidationResult {
        self.validate_internal(schema, Some(dialect))
    }

    fn validate_internal(&self, schema: &Schema, dialect: Option<Dialect>) -> ValidationResult {
        let mut result = ValidationResult::new();

        // ENUMはPostgreSQL専用
        if let Some(dialect) = dialect {
            if !matches!(dialect, Dialect::PostgreSQL) && !schema.enums.is_empty() {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "ENUM definitions are only supported in PostgreSQL (current: {})",
                        dialect
                    ),
                    location: None,
                    suggestion: Some("Remove ENUM definitions or switch to PostgreSQL".to_string()),
                });
            }
        }

        // ENUM定義の検証
        for enum_def in schema.enums.values() {
            if enum_def.values.is_empty() {
                result.add_error(ValidationError::Constraint {
                    message: format!("ENUM '{}' has no values defined", enum_def.name),
                    location: None,
                    suggestion: Some("Define at least one ENUM value".to_string()),
                });
                continue;
            }

            let mut seen = std::collections::HashSet::new();
            for value in &enum_def.values {
                if !seen.insert(value) {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "ENUM '{}' has duplicate value '{}'",
                            enum_def.name, value
                        ),
                        location: None,
                        suggestion: Some("Remove duplicate values".to_string()),
                    });
                    break;
                }
            }
        }

        // 空のスキーマは有効
        if schema.table_count() == 0 && schema.enums.is_empty() {
            return result;
        }

        // 各テーブルの検証
        for (table_name, table) in &schema.tables {
            // テーブルが少なくとも1つのカラムを持つことを検証
            if table.columns.is_empty() {
                result.add_error(ValidationError::Constraint {
                    message: format!("Table '{}' has no columns defined", table_name),
                    location: Some(ErrorLocation::with_table(table_name.clone())),
                    suggestion: Some("Define at least one column".to_string()),
                });
            }

            // 各カラムの型固有バリデーション
            for column in &table.columns {
                self.validate_column_type(
                    &column.column_type,
                    table_name,
                    &column.name,
                    &mut result,
                );

                if let ColumnType::Enum { name } = &column.column_type {
                    if !schema.enums.contains_key(name) {
                        result.add_error(ValidationError::Reference {
                            message: format!(
                                "Column '{}.{}' references undefined ENUM '{}'",
                                table_name, column.name, name
                            ),
                            location: Some(ErrorLocation {
                                table: Some(table_name.clone()),
                                column: Some(column.name.clone()),
                                line: None,
                            }),
                            suggestion: Some(format!(
                                "Define ENUM '{}' in the schema enums section",
                                name
                            )),
                        });
                    }
                }
            }

            // プライマリキーの存在確認
            let has_primary_key = table
                .constraints
                .iter()
                .any(|c| matches!(c, Constraint::PRIMARY_KEY { .. }));

            if !has_primary_key && !table.columns.is_empty() {
                result.add_error(ValidationError::Constraint {
                    message: format!("Table '{}' has no primary key defined", table_name),
                    location: Some(ErrorLocation::with_table(table_name.clone())),
                    suggestion: Some("Add a PRIMARY KEY constraint".to_string()),
                });
            }

            // インデックスのカラム存在確認
            for index in &table.indexes {
                for column_name in &index.columns {
                    if table.get_column(column_name).is_none() {
                        result.add_error(ValidationError::Reference {
                            message: format!(
                                "Index '{}' references column '{}' which does not exist in table '{}'",
                                index.name, column_name, table_name
                            ),
                            location: Some(ErrorLocation {
                                table: Some(table_name.clone()),
                                column: Some(column_name.clone()),
                                line: None,
                            }),
                            suggestion: Some(format!(
                                "Define column '{}' or remove it from the index",
                                column_name
                            )),
                        });
                    }
                }
            }

            // 制約のカラム存在確認
            for constraint in &table.constraints {
                match constraint {
                    Constraint::PRIMARY_KEY { columns }
                    | Constraint::UNIQUE { columns }
                    | Constraint::CHECK { columns, .. } => {
                        for column_name in columns {
                            if table.get_column(column_name).is_none() {
                                result.add_error(ValidationError::Reference {
                                    message: format!(
                                        "Constraint references column '{}' which does not exist in table '{}'",
                                        column_name, table_name
                                    ),
                                    location: Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column_name.clone()),
                                        line: None,
                                    }),
                                    suggestion: Some(format!(
                                        "Define column '{}'",
                                        column_name
                                    )),
                                });
                            }
                        }
                    }
                    Constraint::FOREIGN_KEY {
                        columns,
                        referenced_table,
                        referenced_columns,
                    } => {
                        // 外部キーのソースカラム存在確認
                        for column_name in columns {
                            if table.get_column(column_name).is_none() {
                                result.add_error(ValidationError::Reference {
                                    message: format!(
                                        "Foreign key constraint references column '{}' which does not exist in table '{}'",
                                        column_name, table_name
                                    ),
                                    location: Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column_name.clone()),
                                        line: None,
                                    }),
                                    suggestion: Some(format!(
                                        "Define column '{}'",
                                        column_name
                                    )),
                                });
                            }
                        }

                        // Check if referenced table exists
                        if !schema.has_table(referenced_table) {
                            result.add_error(ValidationError::Reference {
                                message: format!(
                                    "Foreign key constraint references table '{}' which does not exist",
                                    referenced_table
                                ),
                                location: Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: None,
                                    line: None,
                                }),
                                suggestion: Some(format!(
                                    "Define table '{}'",
                                    referenced_table
                                )),
                            });
                        } else {
                            // Check if referenced columns exist
                            if let Some(ref_table) = schema.get_table(referenced_table) {
                                for ref_column_name in referenced_columns {
                                    if ref_table.get_column(ref_column_name).is_none() {
                                        result.add_error(ValidationError::Reference {
                                            message: format!(
                                                "Foreign key constraint references column '{}' which does not exist in table '{}'",
                                                ref_column_name, referenced_table
                                            ),
                                            location: Some(ErrorLocation {
                                                table: Some(referenced_table.clone()),
                                                column: Some(ref_column_name.clone()),
                                                line: None,
                                            }),
                                            suggestion: Some(format!(
                                                "Define column '{}' in table '{}'",
                                                ref_column_name, referenced_table
                                            )),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        result
    }

    /// カラムの型固有バリデーション
    ///
    /// # Arguments
    ///
    /// * `column_type` - 検証対象のカラム型
    /// * `table_name` - テーブル名
    /// * `column_name` - カラム名
    /// * `result` - バリデーション結果（エラーを追加）
    fn validate_column_type(
        &self,
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
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column_name.to_string()),
                            line: None,
                        }),
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
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column_name.to_string()),
                            line: None,
                        }),
                        suggestion: Some(
                            "Set precision to 65 or less for MySQL compatibility".to_string()
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
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column_name.to_string()),
                            line: None,
                        }),
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
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column_name.to_string()),
                            line: None,
                        }),
                        suggestion: Some("Set length to at least 1".to_string()),
                    });
                }

                if *length > 255 {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "CHAR type in column '{}.{}' has length ({}) exceeding maximum (255)",
                            table_name, column_name, length
                        ),
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column_name.to_string()),
                            line: None,
                        }),
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

    /// 方言固有の警告を生成
    ///
    /// # Arguments
    ///
    /// * `schema` - 検証対象のスキーマ
    /// * `dialect` - 対象データベース方言
    ///
    /// # Returns
    ///
    /// 方言固有の警告のリスト
    pub fn generate_dialect_warnings(
        &self,
        schema: &Schema,
        dialect: &Dialect,
    ) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        for (table_name, table) in &schema.tables {
            for column in &table.columns {
                match &column.column_type {
                    ColumnType::DECIMAL { precision, scale } => {
                        // SQLiteでは精度損失の警告
                        if matches!(dialect, Dialect::SQLite) {
                            warnings.push(ValidationWarning::precision_loss(
                                format!(
                                    "DECIMAL({}, {}) in column '{}.{}' will be stored as TEXT in SQLite. Numeric operations may not work as expected.",
                                    precision, scale, table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                    }
                    ColumnType::UUID => {
                        // MySQLではCHAR(36)へのフォールバック警告
                        if matches!(dialect, Dialect::MySQL) {
                            warnings.push(ValidationWarning::dialect_specific(
                                format!(
                                    "UUID in column '{}.{}' will be stored as CHAR(36) in MySQL (native UUID type not available in older versions).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                        // SQLiteではTEXTへのフォールバック警告
                        if matches!(dialect, Dialect::SQLite) {
                            warnings.push(ValidationWarning::dialect_specific(
                                format!(
                                    "UUID in column '{}.{}' will be stored as TEXT in SQLite (native UUID type not available).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                    }
                    ColumnType::JSONB => {
                        // MySQLではJSONへのフォールバック警告
                        if matches!(dialect, Dialect::MySQL) {
                            warnings.push(ValidationWarning::dialect_specific(
                                format!(
                                    "JSONB in column '{}.{}' will be stored as JSON in MySQL (JSONB type not available, binary optimization not applied).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                        // SQLiteではTEXTへのフォールバック警告
                        if matches!(dialect, Dialect::SQLite) {
                            warnings.push(ValidationWarning::dialect_specific(
                                format!(
                                    "JSONB in column '{}.{}' will be stored as TEXT in SQLite (native JSON/JSONB types not available).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                    }
                    ColumnType::TIME { with_time_zone } => {
                        // MySQLとSQLiteではタイムゾーン情報が失われる警告
                        if *with_time_zone == Some(true) {
                            if matches!(dialect, Dialect::MySQL) {
                                warnings.push(ValidationWarning::dialect_specific(
                                    format!(
                                        "TIME WITH TIME ZONE in column '{}.{}' will be stored as TIME in MySQL (timezone information will be lost).",
                                        table_name, column.name
                                    ),
                                    Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column.name.clone()),
                                        line: None,
                                    }),
                                ));
                            }
                            if matches!(dialect, Dialect::SQLite) {
                                warnings.push(ValidationWarning::precision_loss(
                                    format!(
                                        "TIME WITH TIME ZONE in column '{}.{}' will be stored as TEXT in SQLite (timezone information will be lost).",
                                        table_name, column.name
                                    ),
                                    Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column.name.clone()),
                                        line: None,
                                    }),
                                ));
                            }
                        }
                    }
                    ColumnType::DATE => {
                        // SQLiteではTEXT保存の警告
                        if matches!(dialect, Dialect::SQLite) {
                            warnings.push(ValidationWarning::dialect_specific(
                                format!(
                                    "DATE in column '{}.{}' will be stored as TEXT in SQLite (native DATE type not available).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }

        warnings
    }

    /// 外部キー制約の参照整合性を検証
    ///
    /// # Arguments
    ///
    /// * `schema` - Schema to validate
    ///
    /// # Returns
    ///
    /// List of referential integrity errors
    pub fn validate_referential_integrity(&self, schema: &Schema) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        for (table_name, table) in &schema.tables {
            for constraint in &table.constraints {
                if let Constraint::FOREIGN_KEY {
                    referenced_table,
                    referenced_columns,
                    ..
                } = constraint
                {
                    // Check if referenced table exists
                    if !schema.has_table(referenced_table) {
                        errors.push(ValidationError::Reference {
                            message: format!(
                                "Foreign key constraint references table '{}' which does not exist",
                                referenced_table
                            ),
                            location: Some(ErrorLocation::with_table(table_name.clone())),
                            suggestion: Some(format!("Define table '{}'", referenced_table)),
                        });
                    } else if let Some(ref_table) = schema.get_table(referenced_table) {
                        // Check if referenced columns exist
                        for ref_column_name in referenced_columns {
                            if ref_table.get_column(ref_column_name).is_none() {
                                errors.push(ValidationError::Reference {
                                    message: format!(
                                        "Foreign key constraint references column '{}' which does not exist in table '{}'",
                                        ref_column_name, referenced_table
                                    ),
                                    location: Some(ErrorLocation {
                                        table: Some(referenced_table.clone()),
                                        column: Some(ref_column_name.clone()),
                                        line: None,
                                    }),
                                    suggestion: Some(format!(
                                        "Define column '{}' in table '{}'",
                                        ref_column_name, referenced_table
                                    )),
                                });
                            }
                        }
                    }
                }
            }
        }

        errors
    }
}

impl Default for SchemaValidatorService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Dialect;
    use crate::core::schema::{Column, ColumnType, EnumDefinition, Table};

    #[test]
    fn test_new_service() {
        let service = SchemaValidatorService::new();
        // サービスが正常に作成されることを確認
        assert!(format!("{:?}", service).contains("SchemaValidatorService"));
    }

    #[test]
    fn test_validate_empty_schema() {
        let schema = Schema::new("1.0".to_string());
        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_validate_table_without_columns() {
        let mut schema = Schema::new("1.0".to_string());
        let table = Table::new("empty_table".to_string());
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
    }

    #[test]
    fn test_validate_table_without_primary_key() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
    }

    #[test]
    fn test_validate_valid_schema() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_validate_enum_empty_values() {
        let mut schema = Schema::new("1.0".to_string());

        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec![],
        });

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::Enum {
                name: "status".to_string(),
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("ENUM") && e.to_string().contains("no values")));
    }

    #[test]
    fn test_validate_enum_duplicate_values() {
        let mut schema = Schema::new("1.0".to_string());

        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "active".to_string()],
        });

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::Enum {
                name: "status".to_string(),
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("ENUM") && e.to_string().contains("duplicate")));
    }

    #[test]
    fn test_validate_enum_reference_missing() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::Enum {
                name: "status".to_string(),
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("undefined ENUM")));
    }

    #[test]
    fn test_validate_enum_non_postgres_dialect() {
        let mut schema = Schema::new("1.0".to_string());

        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string()],
        });

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::Enum {
                name: "status".to_string(),
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_with_dialect(&schema, Dialect::MySQL);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("PostgreSQL") && e.to_string().contains("ENUM")));
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

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

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

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

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

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

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

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

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

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
        assert!(result.errors[0]
            .to_string()
            .contains("length (300) exceeding maximum (255)"));
    }

    #[test]
    fn test_generate_dialect_warnings_sqlite_decimal() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 10,
                scale: 2,
            },
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let warnings = validator.generate_dialect_warnings(&schema, &Dialect::SQLite);

        assert!(!warnings.is_empty());
        assert!(warnings[0].message.contains("will be stored as TEXT"));
    }

    #[test]
    fn test_generate_dialect_warnings_mysql_uuid() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new("uuid".to_string(), ColumnType::UUID, false));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let warnings = validator.generate_dialect_warnings(&schema, &Dialect::MySQL);

        assert!(!warnings.is_empty());
        assert!(warnings[0].message.contains("CHAR(36)"));
    }

    #[test]
    fn test_generate_dialect_warnings_mysql_jsonb() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("documents".to_string());
        table.add_column(Column::new("data".to_string(), ColumnType::JSONB, false));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let warnings = validator.generate_dialect_warnings(&schema, &Dialect::MySQL);

        assert!(!warnings.is_empty());
        assert!(warnings[0]
            .message
            .contains("will be stored as JSON in MySQL"));
    }

    #[test]
    fn test_validate_dialect_specific_type_skip_validation() {
        // DialectSpecific バリアントは検証をスキップする（データベースに委譲）
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());

        // PostgreSQL SERIAL型（方言固有型）
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::DialectSpecific {
                kind: "SERIAL".to_string(),
                params: serde_json::Value::Null,
            },
            false,
        ));

        // プライマリキーを追加（必須）
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // DialectSpecific型は検証エラーを生成しない
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_validate_dialect_specific_type_with_params() {
        // パラメータ付きDialectSpecific型（MySQL ENUM）も検証スキップ
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());

        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        // MySQL ENUM型（パラメータ付き方言固有型）
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::DialectSpecific {
                kind: "ENUM".to_string(),
                params: serde_json::json!({
                    "values": ["active", "inactive", "pending"]
                }),
            },
            false,
        ));

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // DialectSpecific型は検証エラーを生成しない
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_validate_dialect_specific_type_invalid_kind() {
        // 無効な型名（INVALID_TYPE）でも検証をスキップ
        // データベース実行時にエラーが検出される
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());

        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::DialectSpecific {
                kind: "INVALID_TYPE".to_string(), // 存在しない型
                params: serde_json::Value::Null,
            },
            false,
        ));

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // Stratum内部では検証しない（データベースに委譲）
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_validate_mixed_common_and_dialect_specific_types() {
        // 共通型と方言固有型の混在スキーマ
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());

        // 方言固有型（PostgreSQL SERIAL）
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::DialectSpecific {
                kind: "SERIAL".to_string(),
                params: serde_json::Value::Null,
            },
            false,
        ));

        // 共通型（VARCHAR）
        table.add_column(Column::new(
            "username".to_string(),
            ColumnType::VARCHAR { length: 50 },
            false,
        ));

        // 方言固有型（PostgreSQL INET）
        table.add_column(Column::new(
            "ip_address".to_string(),
            ColumnType::DialectSpecific {
                kind: "INET".to_string(),
                params: serde_json::Value::Null,
            },
            true,
        ));

        // 共通型（TIMESTAMP）
        table.add_column(Column::new(
            "created_at".to_string(),
            ColumnType::TIMESTAMP {
                with_time_zone: Some(true),
            },
            false,
        ));

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // 混在スキーマも有効
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }
}
