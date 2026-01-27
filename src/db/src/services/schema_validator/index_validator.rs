// インデックスの検証

use crate::core::error::{ErrorLocation, ValidationError, ValidationResult};
use crate::core::schema::Schema;

/// インデックスのカラム参照整合性検証
pub fn validate_index_references(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (table_name, table) in &schema.tables {
        for index in &table.indexes {
            for column_name in &index.columns {
                if table.get_column(column_name).is_none() {
                    result.add_error(ValidationError::Reference {
                        message: format!(
                            "Index '{}' references column '{}' which does not exist in table '{}'",
                            index.name, column_name, table_name
                        ),
                        location: Some(ErrorLocation::with_table_and_column(
                            table_name,
                            column_name,
                        )),
                        suggestion: Some(format!(
                            "Define column '{}' or remove it from the index",
                            column_name
                        )),
                    });
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::core::schema::{Column, ColumnType, Index, Table};

    use super::*;

    #[test]
    fn test_validate_index_references_invalid_column() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["nonexistent_column".to_string()],
            false,
        ));
        schema.add_table(table);

        let result = validate_index_references(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("Index")));
    }

    #[test]
    fn test_validate_index_references_valid() {
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
        table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            false,
        ));
        schema.add_table(table);

        let result = validate_index_references(&schema);

        assert!(result.is_valid());
    }
}
