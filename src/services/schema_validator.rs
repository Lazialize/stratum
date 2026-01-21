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
                    message: format!("テーブル '{}' にカラムが定義されていません", table_name),
                    location: Some(ErrorLocation::with_table(table_name.clone())),
                    suggestion: Some("少なくとも1つのカラムを定義してください".to_string()),
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
                        "テーブル '{}' にプライマリキーが定義されていません",
                        table_name
                    ),
                    location: Some(ErrorLocation::with_table(table_name.clone())),
                    suggestion: Some("PRIMARY KEY制約を追加してください".to_string()),
                });
            }

            // インデックスのカラム存在確認
            for index in &table.indexes {
                for column_name in &index.columns {
                    if table.get_column(column_name).is_none() {
                        result.add_error(ValidationError::Reference {
                            message: format!(
                                "インデックス '{}' が参照するカラム '{}' がテーブル '{}' に存在しません",
                                index.name, column_name, table_name
                            ),
                            location: Some(ErrorLocation {
                                table: Some(table_name.clone()),
                                column: Some(column_name.clone()),
                                line: None,
                            }),
                            suggestion: Some(format!(
                                "カラム '{}' を定義するか、インデックスから削除してください",
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
                                        "制約が参照するカラム '{}' がテーブル '{}' に存在しません",
                                        column_name, table_name
                                    ),
                                    location: Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column_name.clone()),
                                        line: None,
                                    }),
                                    suggestion: Some(format!(
                                        "カラム '{}' を定義してください",
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
                                        "外部キー制約が参照するカラム '{}' がテーブル '{}' に存在しません",
                                        column_name, table_name
                                    ),
                                    location: Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column_name.clone()),
                                        line: None,
                                    }),
                                    suggestion: Some(format!(
                                        "カラム '{}' を定義してください",
                                        column_name
                                    )),
                                });
                            }
                        }

                        // 参照先テーブルの存在確認
                        if !schema.has_table(referenced_table) {
                            result.add_error(ValidationError::Reference {
                                message: format!(
                                    "外部キー制約が参照するテーブル '{}' が存在しません",
                                    referenced_table
                                ),
                                location: Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: None,
                                    line: None,
                                }),
                                suggestion: Some(format!(
                                    "テーブル '{}' を定義してください",
                                    referenced_table
                                )),
                            });
                        } else {
                            // 参照先カラムの存在確認
                            if let Some(ref_table) = schema.get_table(referenced_table) {
                                for ref_column_name in referenced_columns {
                                    if ref_table.get_column(ref_column_name).is_none() {
                                        result.add_error(ValidationError::Reference {
                                            message: format!(
                                                "外部キー制約が参照するカラム '{}' がテーブル '{}' に存在しません",
                                                ref_column_name, referenced_table
                                            ),
                                            location: Some(ErrorLocation {
                                                table: Some(referenced_table.clone()),
                                                column: Some(ref_column_name.clone()),
                                                line: None,
                                            }),
                                            suggestion: Some(format!(
                                                "テーブル '{}' にカラム '{}' を定義してください",
                                                referenced_table, ref_column_name
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
    /// * `schema` - 検証対象のスキーマ
    ///
    /// # Returns
    ///
    /// 参照整合性エラーのリスト
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
                    // 参照先テーブルの存在確認
                    if !schema.has_table(referenced_table) {
                        errors.push(ValidationError::Reference {
                            message: format!(
                                "外部キー制約が参照するテーブル '{}' が存在しません",
                                referenced_table
                            ),
                            location: Some(ErrorLocation::with_table(table_name.clone())),
                            suggestion: Some(format!(
                                "テーブル '{}' を定義してください",
                                referenced_table
                            )),
                        });
                    } else if let Some(ref_table) = schema.get_table(referenced_table) {
                        // 参照先カラムの存在確認
                        for ref_column_name in referenced_columns {
                            if ref_table.get_column(ref_column_name).is_none() {
                                errors.push(ValidationError::Reference {
                                    message: format!(
                                        "外部キー制約が参照するカラム '{}' がテーブル '{}' に存在しません",
                                        ref_column_name, referenced_table
                                    ),
                                    location: Some(ErrorLocation {
                                        table: Some(referenced_table.clone()),
                                        column: Some(ref_column_name.clone()),
                                        line: None,
                                    }),
                                    suggestion: Some(format!(
                                        "テーブル '{}' にカラム '{}' を定義してください",
                                        referenced_table, ref_column_name
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
