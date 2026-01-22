// スキーマバリデーターサービス
//
// スキーマ定義の整合性、参照整合性、制約の検証を行うサービス。
// テーブル定義、インデックス、外部キー制約などを検証します。

use crate::core::error::{ErrorLocation, ValidationError, ValidationResult};
use crate::core::schema::{Constraint, Schema};

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
        let mut result = ValidationResult::new();

        // 空のスキーマは有効
        if schema.table_count() == 0 {
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

            // プライマリキーの存在確認
            let has_primary_key = table
                .constraints
                .iter()
                .any(|c| matches!(c, Constraint::PRIMARY_KEY { .. }));

            if !has_primary_key && !table.columns.is_empty() {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "Table '{}' has no primary key defined",
                        table_name
                    ),
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
                            suggestion: Some(format!(
                                "Define table '{}'",
                                referenced_table
                            )),
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
    use crate::core::schema::{Column, ColumnType, Table};

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
}
