// 方言固有の検証

use crate::core::config::Dialect;
use crate::core::error::{ErrorLocation, ValidationWarning};
use crate::core::schema::{ColumnType, Schema};

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
pub fn generate_dialect_warnings(schema: &Schema, dialect: &Dialect) -> Vec<ValidationWarning> {
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
                            Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
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
                            Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
                        ));
                    }
                    // SQLiteではTEXTへのフォールバック警告
                    if matches!(dialect, Dialect::SQLite) {
                        warnings.push(ValidationWarning::dialect_specific(
                            format!(
                                "UUID in column '{}.{}' will be stored as TEXT in SQLite (native UUID type not available).",
                                table_name, column.name
                            ),
                            Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
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
                            Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
                        ));
                    }
                    // SQLiteではTEXTへのフォールバック警告
                    if matches!(dialect, Dialect::SQLite) {
                        warnings.push(ValidationWarning::dialect_specific(
                            format!(
                                "JSONB in column '{}.{}' will be stored as TEXT in SQLite (native JSON/JSONB types not available).",
                                table_name, column.name
                            ),
                            Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
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
                                Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
                            ));
                        }
                        if matches!(dialect, Dialect::SQLite) {
                            warnings.push(ValidationWarning::precision_loss(
                                format!(
                                    "TIME WITH TIME ZONE in column '{}.{}' will be stored as TEXT in SQLite (timezone information will be lost).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
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
                            Some(ErrorLocation::with_table_and_column(table_name, &column.name)),
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use crate::core::schema::{Column, Table};

    use super::*;

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

        let warnings = generate_dialect_warnings(&schema, &Dialect::SQLite);

        assert!(!warnings.is_empty());
        assert!(warnings[0].message.contains("will be stored as TEXT"));
    }

    #[test]
    fn test_generate_dialect_warnings_mysql_uuid() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new("uuid".to_string(), ColumnType::UUID, false));
        schema.add_table(table);

        let warnings = generate_dialect_warnings(&schema, &Dialect::MySQL);

        assert!(!warnings.is_empty());
        assert!(warnings[0].message.contains("CHAR(36)"));
    }

    #[test]
    fn test_generate_dialect_warnings_mysql_jsonb() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("documents".to_string());
        table.add_column(Column::new("data".to_string(), ColumnType::JSONB, false));
        schema.add_table(table);

        let warnings = generate_dialect_warnings(&schema, &Dialect::MySQL);

        assert!(!warnings.is_empty());
        assert!(warnings[0]
            .message
            .contains("will be stored as JSON in MySQL"));
    }
}
