// 制約の検証（PK, FK, UNIQUE）

use super::validation_helpers::check_column_exists;
use crate::core::error::{ErrorLocation, ValidationError, ValidationResult};
use crate::core::schema::{Constraint, Schema};

/// プライマリキーの存在確認
pub fn validate_primary_keys(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (table_name, table) in &schema.tables {
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
    }

    result
}

/// 制約のカラム/テーブル参照整合性検証
pub fn validate_constraint_references(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (table_name, table) in &schema.tables {
        for constraint in &table.constraints {
            match constraint {
                Constraint::PRIMARY_KEY { columns }
                | Constraint::UNIQUE { columns }
                | Constraint::CHECK { columns, .. } => {
                    for column_name in columns {
                        check_column_exists(
                            table,
                            table_name,
                            column_name,
                            &mut result,
                            "Constraint references",
                        );
                    }
                }
                Constraint::FOREIGN_KEY {
                    columns,
                    referenced_table,
                    referenced_columns,
                } => {
                    // 外部キーのソースカラム存在確認
                    for column_name in columns {
                        check_column_exists(
                            table,
                            table_name,
                            column_name,
                            &mut result,
                            "Foreign key constraint references",
                        );
                    }

                    // 参照先テーブルの存在確認
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
                            suggestion: Some(format!("Define table '{}'", referenced_table)),
                        });
                    } else if let Some(ref_table) = schema.get_table(referenced_table) {
                        // 参照先カラムの存在確認
                        for ref_column_name in referenced_columns {
                            if ref_table.get_column(ref_column_name).is_none() {
                                result.add_error(ValidationError::Reference {
                                    message: format!(
                                        "Foreign key constraint references column '{}' which does not exist in table '{}'",
                                        ref_column_name, referenced_table
                                    ),
                                    location: Some(ErrorLocation::with_table_and_column(referenced_table, ref_column_name)),
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

    result
}

/// CHECK制約のexpressionが空でないことを検証
pub fn validate_check_expressions(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (table_name, table) in &schema.tables {
        for constraint in &table.constraints {
            if let Constraint::CHECK {
                check_expression, ..
            } = constraint
            {
                if check_expression.trim().is_empty() {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "テーブル '{}' のCHECK制約の check_expression が空です",
                            table_name
                        ),
                        location: Some(ErrorLocation::with_table(table_name.clone())),
                        suggestion: Some("CHECK制約には有効なSQL式を指定してください".to_string()),
                    });
                }
            }
        }
    }

    result
}

/// 同一テーブル内に同じカラム構成のUNIQUE制約が重複していないか検証
pub fn validate_duplicate_unique_constraints(schema: &Schema) -> ValidationResult {
    use crate::core::error::ValidationWarning;

    let mut result = ValidationResult::new();

    for (table_name, table) in &schema.tables {
        let unique_column_sets: Vec<Vec<String>> = table
            .constraints
            .iter()
            .filter_map(|c| {
                if let Constraint::UNIQUE { columns } = c {
                    let mut sorted = columns.clone();
                    sorted.sort();
                    Some(sorted)
                } else {
                    None
                }
            })
            .collect();

        // 重複チェック
        for i in 0..unique_column_sets.len() {
            for j in (i + 1)..unique_column_sets.len() {
                if unique_column_sets[i] == unique_column_sets[j] {
                    let columns_str = unique_column_sets[i].join(", ");
                    result.add_warning(ValidationWarning::compatibility(
                        format!(
                            "テーブル '{}' に同じカラム構成 ({}) のUNIQUE制約が複数定義されています",
                            table_name, columns_str
                        ),
                        Some(ErrorLocation::with_table(table_name.clone())),
                    ));
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
pub fn validate_referential_integrity(schema: &Schema) -> Vec<ValidationError> {
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
                                location: Some(ErrorLocation::with_table_and_column(referenced_table, ref_column_name)),
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

#[cfg(test)]
mod tests {
    use crate::core::schema::{Column, ColumnType, Table};

    use super::*;

    #[test]
    fn test_validate_primary_keys_missing() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema.add_table(table);

        let result = validate_primary_keys(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("primary key")));
    }

    #[test]
    fn test_validate_primary_keys_present() {
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

        let result = validate_primary_keys(&schema);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_constraint_references_invalid_fk_table() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("posts".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "user_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "nonexistent_table".to_string(),
            referenced_columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let result = validate_constraint_references(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("does not exist")));
    }

    #[test]
    fn test_validate_constraint_references_invalid_pk_column() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["nonexistent_column".to_string()],
        });
        schema.add_table(table);

        let result = validate_constraint_references(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("Constraint references column")));
    }

    // ==========================================
    // CHECK式空文字列バリデーション
    // ==========================================

    #[test]
    fn test_validate_check_expression_empty() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "".to_string(),
        });
        schema.add_table(table);

        let result = validate_check_expressions(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("products")
                && e.to_string().contains("check_expression")));
    }

    #[test]
    fn test_validate_check_expression_whitespace_only() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "   ".to_string(),
        });
        schema.add_table(table);

        let result = validate_check_expressions(&schema);

        assert!(!result.is_valid());
    }

    #[test]
    fn test_validate_check_expression_valid() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        });
        schema.add_table(table);

        let result = validate_check_expressions(&schema);

        assert!(result.is_valid());
    }

    // ==========================================
    // 重複UNIQUE制約バリデーション
    // ==========================================

    #[test]
    fn test_validate_duplicate_unique_constraints() {
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
        // 同じカラム構成のUNIQUE制約が2つ
        table.add_constraint(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        table.add_constraint(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        schema.add_table(table);

        let result = validate_duplicate_unique_constraints(&schema);

        assert!(result.warning_count() > 0);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("users") && w.message.contains("email")));
    }

    #[test]
    fn test_validate_duplicate_unique_constraints_different_order() {
        // カラム順序が違っても同じ構成として検出する
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "first_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        table.add_column(Column::new(
            "last_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        table.add_constraint(Constraint::UNIQUE {
            columns: vec!["first_name".to_string(), "last_name".to_string()],
        });
        table.add_constraint(Constraint::UNIQUE {
            columns: vec!["last_name".to_string(), "first_name".to_string()],
        });
        schema.add_table(table);

        let result = validate_duplicate_unique_constraints(&schema);

        assert!(result.warning_count() > 0);
    }

    #[test]
    fn test_validate_no_duplicate_unique_constraints() {
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
        table.add_column(Column::new(
            "username".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        table.add_constraint(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        table.add_constraint(Constraint::UNIQUE {
            columns: vec!["username".to_string()],
        });
        schema.add_table(table);

        let result = validate_duplicate_unique_constraints(&schema);

        assert_eq!(result.warning_count(), 0);
    }

    #[test]
    fn test_validate_constraint_references_valid() {
        let mut schema = Schema::new("1.0".to_string());

        // users table
        let mut users_table = Table::new("users".to_string());
        users_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        users_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(users_table);

        // posts table with valid FK
        let mut posts_table = Table::new("posts".to_string());
        posts_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts_table.add_column(Column::new(
            "user_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts_table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
        });
        schema.add_table(posts_table);

        let result = validate_constraint_references(&schema);

        assert!(result.is_valid());
    }
}
