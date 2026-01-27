// 制約の検証（PK, FK, UNIQUE）

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
                        if table.get_column(column_name).is_none() {
                            result.add_error(ValidationError::Reference {
                                message: format!(
                                    "Constraint references column '{}' which does not exist in table '{}'",
                                    column_name, table_name
                                ),
                                location: Some(ErrorLocation::with_table_and_column(table_name, column_name)),
                                suggestion: Some(format!("Define column '{}'", column_name)),
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
                                location: Some(ErrorLocation::with_table_and_column(table_name, column_name)),
                                suggestion: Some(format!("Define column '{}'", column_name)),
                            });
                        }
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
