// カラムリネームの検証

use crate::core::error::{ErrorLocation, ValidationError, ValidationResult, ValidationWarning};
use crate::core::schema::{Constraint, Schema};

/// カラムリネーム検証の内部実装
pub fn validate_renames_internal(schema: &Schema, old_schema: Option<&Schema>) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (table_name, table) in &schema.tables {
        // 重複リネーム検出用のマップ（renamed_from -> カラム名のリスト）
        let mut rename_sources: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        // renamed_fromを持つカラムを収集
        for column in &table.columns {
            if let Some(ref old_name) = column.renamed_from {
                rename_sources
                    .entry(old_name.clone())
                    .or_default()
                    .push(column.name.clone());
            }
        }

        // 重複リネームの検出
        for (old_name, new_names) in &rename_sources {
            if new_names.len() > 1 {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "duplicate rename: '{}' is renamed to multiple columns ({}) in table '{}'",
                        old_name,
                        new_names.join(", "),
                        table_name
                    ),
                    location: Some(ErrorLocation::with_table(table_name.clone())),
                    suggestion: Some("Each column can only be renamed once. Remove duplicate renamed_from attributes.".to_string()),
                });
            }
        }

        // 名前衝突の検出
        // renamed_fromが既存のカラム名（リネーム先でない）と衝突する場合
        for column in &table.columns {
            if let Some(ref old_name) = column.renamed_from {
                // 同じテーブル内に old_name と同名のカラムが存在するか確認
                // （ただし、そのカラム自体が新しい名前に変わるのでなければエラー）
                for other_column in &table.columns {
                    // 既存のカラム名がold_nameと一致し、
                    // そのカラムがリネーム対象でない（renamed_fromを持たない）場合
                    if other_column.name == *old_name && other_column.renamed_from.is_none() {
                        result.add_error(ValidationError::Constraint {
                            message: format!(
                                "name collision: renamed_from '{}' conflicts with existing column '{}' in table '{}'",
                                old_name, other_column.name, table_name
                            ),
                            location: Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
                            suggestion: Some(format!(
                                "Remove the existing column '{}' or change the renamed_from value",
                                old_name
                            )),
                        });
                    }
                }

                // 旧カラム存在確認（old_schemaが提供された場合のみ）
                if let Some(old_schema) = old_schema {
                    if let Some(old_table) = old_schema.get_table(table_name) {
                        if old_table.get_column(old_name).is_none() {
                            result.add_warning(ValidationWarning::old_column_not_found(
                                format!(
                                    "Column '{}' in table '{}' has renamed_from='{}', but column '{}' does not exist in the old schema. \
                                    Consider removing the renamed_from attribute.",
                                    column.name, table_name, old_name, old_name
                                ),
                                Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
                            ));
                        }
                    }
                }
            }
        }
    }

    // FK参照カラムのリネーム警告
    // 他のテーブルからFKで参照されているカラムがリネームされる場合
    for (table_name, table) in &schema.tables {
        for column in &table.columns {
            if let Some(ref old_name) = column.renamed_from {
                // このテーブルのこのカラム（old_name）を参照しているFKがあるか確認
                for (other_table_name, other_table) in &schema.tables {
                    for constraint in &other_table.constraints {
                        if let Constraint::FOREIGN_KEY {
                            referenced_table,
                            referenced_columns,
                            ..
                        } = constraint
                        {
                            if referenced_table == table_name
                                && referenced_columns.contains(old_name)
                            {
                                result.add_warning(ValidationWarning::foreign_key_reference(
                                    format!(
                                        "Column '{}' in table '{}' is referenced by a foreign key from table '{}'. \
                                        Renaming this column may break the FK constraint. \
                                        Update the FK reference after migration.",
                                        old_name, table_name, other_table_name
                                    ),
                                    Some(ErrorLocation::with_table_and_column(table_name, old_name)),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::core::schema::{Column, ColumnType, Table};

    use super::*;

    #[test]
    fn test_validate_renames_duplicate_renamed_from_error() {
        // 同じrenamed_fromが複数カラムで指定された場合のエラー検出
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        // 両方のカラムが同じold_nameからリネームしようとしている
        let mut column1 = Column::new(
            "new_email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        column1.renamed_from = Some("old_email".to_string());
        table.add_column(column1);

        let mut column2 = Column::new(
            "email_address".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        column2.renamed_from = Some("old_email".to_string()); // 重複
        table.add_column(column2);

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let result = validate_renames_internal(&schema, None);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("old_email") && e.to_string().contains("duplicate")));
    }

    #[test]
    fn test_validate_renames_name_collision_error() {
        // renamed_fromがリネーム先以外の既存カラム名と一致する場合のエラー
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        // 既存のカラム
        table.add_column(Column::new(
            "existing_column".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));

        // renamed_fromが既存のカラム名と衝突
        let mut renamed_column = Column::new(
            "new_name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        renamed_column.renamed_from = Some("existing_column".to_string());
        table.add_column(renamed_column);

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let result = validate_renames_internal(&schema, None);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("existing_column")
                && e.to_string().contains("collision")));
    }

    #[test]
    fn test_validate_renames_fk_reference_warning() {
        // FK参照カラムのリネーム警告
        let mut schema = Schema::new("1.0".to_string());

        // users table (参照元)
        let mut users_table = Table::new("users".to_string());
        users_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        // リネームされるカラム（FK参照されている）
        let mut renamed_column = Column::new("user_uuid".to_string(), ColumnType::UUID, false);
        renamed_column.renamed_from = Some("uuid".to_string());
        users_table.add_column(renamed_column);
        users_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(users_table);

        // posts table (FK参照を持つ)
        let mut posts_table = Table::new("posts".to_string());
        posts_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts_table.add_column(Column::new(
            "user_uuid".to_string(),
            ColumnType::UUID,
            false,
        ));
        posts_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        // uuid カラムを参照するFK（リネームされるカラム）
        posts_table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_uuid".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["uuid".to_string()], // リネームされるカラムを参照
            on_delete: None,
            on_update: None,
        });
        schema.add_table(posts_table);

        let result = validate_renames_internal(&schema, None);

        // 警告が生成されるべき
        assert!(result.warning_count() > 0);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("foreign key") || w.message.contains("FK")));
    }

    #[test]
    fn test_validate_renames_valid_single_rename() {
        // 有効な単一リネーム
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        let mut renamed_column = Column::new(
            "email_address".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        renamed_column.renamed_from = Some("email".to_string());
        table.add_column(renamed_column);

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let result = validate_renames_internal(&schema, None);

        // エラーなし
        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_renames_no_renames() {
        // リネームなしのスキーマ
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let result = validate_renames_internal(&schema, None);

        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
        assert_eq!(result.warning_count(), 0);
    }

    #[test]
    fn test_validate_renames_multiple_tables_with_same_old_name() {
        // 異なるテーブルで同じrenamed_fromを使用（これはOK）
        let mut schema = Schema::new("1.0".to_string());

        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        let mut column1 = Column::new(
            "new_name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        column1.renamed_from = Some("old_name".to_string());
        table1.add_column(column1);
        table1.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table1);

        let mut table2 = Table::new("posts".to_string());
        table2.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        // 別のテーブルで同じold_name（これは許可される）
        let mut column2 = Column::new(
            "new_name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        column2.renamed_from = Some("old_name".to_string());
        table2.add_column(column2);
        table2.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table2);

        let result = validate_renames_internal(&schema, None);

        // 異なるテーブルでの同名は許可
        assert!(result.is_valid());
    }
}
