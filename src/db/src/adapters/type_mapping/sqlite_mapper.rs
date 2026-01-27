// SQLite用型マッパー

use super::TypeMapper;
use super::TypeMetadata;
use crate::core::schema::ColumnType;

/// SQLite用型マッパー
pub struct SqliteTypeMapper;

impl TypeMapper for SqliteTypeMapper {
    fn parse_sql_type(&self, sql_type: &str, _metadata: &TypeMetadata) -> Option<ColumnType> {
        let upper = sql_type.to_uppercase();

        if upper.contains("INT") {
            Some(ColumnType::INTEGER { precision: None })
        } else if upper.contains("CHAR") || upper.contains("VARCHAR") {
            // VARCHAR(255) のような形式から長さを抽出
            if let Some(start) = sql_type.find('(') {
                if let Some(end) = sql_type.find(')') {
                    if let Ok(length) = sql_type[start + 1..end].parse::<u32>() {
                        return Some(ColumnType::VARCHAR { length });
                    }
                }
            }
            Some(ColumnType::VARCHAR { length: 255 })
        } else if upper == "TEXT" {
            Some(ColumnType::TEXT)
        } else if upper == "REAL" {
            Some(ColumnType::FLOAT)
        } else if upper == "BLOB" {
            Some(ColumnType::BLOB)
        } else {
            Some(ColumnType::TEXT)
        }
    }

    fn format_sql_type(&self, column_type: &ColumnType, _auto_increment: Option<bool>) -> String {
        // SQLiteは型アフィニティによる簡略化された型システムを持つため、
        // 共通型ヘルパーは使用せず、すべてSQLite固有のマッピングを行う
        match column_type {
            ColumnType::INTEGER { .. } => "INTEGER".to_string(),
            ColumnType::VARCHAR { .. } => "TEXT".to_string(),
            ColumnType::TEXT => "TEXT".to_string(),
            ColumnType::BOOLEAN => "INTEGER".to_string(),
            ColumnType::TIMESTAMP { .. } => "TEXT".to_string(),
            ColumnType::JSON => "TEXT".to_string(),
            ColumnType::JSONB => "TEXT".to_string(),
            ColumnType::DECIMAL { .. } => "TEXT".to_string(),
            ColumnType::FLOAT => "REAL".to_string(),
            ColumnType::DOUBLE => "REAL".to_string(),
            ColumnType::CHAR { .. } => "TEXT".to_string(),
            ColumnType::DATE => "TEXT".to_string(),
            ColumnType::TIME { .. } => "TEXT".to_string(),
            ColumnType::BLOB => "BLOB".to_string(),
            ColumnType::UUID => "TEXT".to_string(),
            // SQLiteはENUM型をサポートしないため、TEXTにフォールバック
            ColumnType::Enum { .. } => "TEXT".to_string(),
            ColumnType::DialectSpecific { kind, params } => {
                self.format_dialect_specific(kind, params)
            }
        }
    }

    fn format_dialect_specific(&self, kind: &str, _params: &serde_json::Value) -> String {
        // SQLiteは型アフィニティによる柔軟な型システムを持つため、
        // 方言固有型はそのまま出力
        kind.to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::adapters::type_mapping::TypeMappingService;
    use crate::core::config::Dialect;
    use crate::core::schema::ColumnType;

    use super::*;

    #[test]
    fn test_sqlite_integer() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: None }),
            "INTEGER"
        );
    }

    #[test]
    fn test_sqlite_varchar() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(
            service.to_sql_type(&ColumnType::VARCHAR { length: 255 }),
            "TEXT"
        );
    }

    #[test]
    fn test_sqlite_boolean() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(service.to_sql_type(&ColumnType::BOOLEAN), "INTEGER");
    }

    #[test]
    fn test_sqlite_timestamp() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(
            service.to_sql_type(&ColumnType::TIMESTAMP {
                with_time_zone: Some(true)
            }),
            "TEXT"
        );
    }

    #[test]
    fn test_sqlite_decimal() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(
            service.to_sql_type(&ColumnType::DECIMAL {
                precision: 10,
                scale: 2
            }),
            "TEXT"
        );
    }

    #[test]
    fn test_sqlite_float_double() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(service.to_sql_type(&ColumnType::FLOAT), "REAL");
        assert_eq!(service.to_sql_type(&ColumnType::DOUBLE), "REAL");
    }

    #[test]
    fn test_sqlite_parse_integer() {
        let service = TypeMappingService::new(Dialect::SQLite);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("INTEGER", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { .. }));
    }

    #[test]
    fn test_sqlite_parse_varchar_with_length() {
        let service = TypeMappingService::new(Dialect::SQLite);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("VARCHAR(100)", &metadata).unwrap();
        assert!(matches!(result, ColumnType::VARCHAR { length: 100 }));
    }

    #[test]
    fn test_sqlite_parse_text() {
        let service = TypeMappingService::new(Dialect::SQLite);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("TEXT", &metadata).unwrap();
        assert!(matches!(result, ColumnType::TEXT));
    }

    #[test]
    fn test_sqlite_parse_real() {
        let service = TypeMappingService::new(Dialect::SQLite);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("REAL", &metadata).unwrap();
        assert!(matches!(result, ColumnType::FLOAT));
    }

    #[test]
    fn test_sqlite_parse_blob() {
        let service = TypeMappingService::new(Dialect::SQLite);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("BLOB", &metadata).unwrap();
        assert!(matches!(result, ColumnType::BLOB));
    }
}
