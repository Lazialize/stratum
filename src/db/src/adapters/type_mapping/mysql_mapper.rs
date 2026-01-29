// MySQL用型マッパー

use super::common::format_common_sql_type;
use super::TypeMapper;
use super::TypeMetadata;
use crate::core::schema::ColumnType;

/// MySQL用型マッパー
pub struct MySqlTypeMapper;

impl TypeMapper for MySqlTypeMapper {
    fn parse_sql_type(&self, sql_type: &str, metadata: &TypeMetadata) -> Option<ColumnType> {
        match sql_type {
            "int" | "integer" => Some(ColumnType::INTEGER {
                precision: metadata.numeric_precision,
            }),
            "smallint" => Some(ColumnType::INTEGER { precision: Some(2) }),
            "bigint" => Some(ColumnType::INTEGER { precision: Some(8) }),
            "tinyint" => Some(ColumnType::INTEGER {
                precision: metadata.numeric_precision,
            }),
            "varchar" => Some(ColumnType::VARCHAR {
                length: metadata.char_max_length.unwrap_or(255),
            }),
            "text" | "longtext" | "mediumtext" => Some(ColumnType::TEXT),
            "tinyint(1)" => Some(ColumnType::BOOLEAN),
            "datetime" | "timestamp" => Some(ColumnType::TIMESTAMP {
                with_time_zone: None,
            }),
            "json" => Some(ColumnType::JSON),
            "decimal" | "numeric" => Some(ColumnType::DECIMAL {
                precision: metadata.numeric_precision.unwrap_or(10),
                scale: metadata.numeric_scale.unwrap_or(0),
            }),
            "float" => Some(ColumnType::FLOAT),
            "double" => Some(ColumnType::DOUBLE),
            "char" => Some(ColumnType::CHAR {
                length: metadata.char_max_length.unwrap_or(1),
            }),
            "date" => Some(ColumnType::DATE),
            "time" => Some(ColumnType::TIME {
                with_time_zone: None,
            }),
            "blob" | "longblob" | "mediumblob" => Some(ColumnType::BLOB),
            _ => None,
        }
    }

    fn format_sql_type(&self, column_type: &ColumnType, _auto_increment: Option<bool>) -> String {
        // 共通型を先にチェック
        if let Some(sql) = format_common_sql_type(column_type) {
            return sql;
        }

        match column_type {
            ColumnType::INTEGER { precision } => match precision {
                Some(2) => "SMALLINT".to_string(),
                Some(8) => "BIGINT".to_string(),
                _ => "INT".to_string(),
            },
            ColumnType::TIMESTAMP { .. } => "TIMESTAMP".to_string(),
            ColumnType::JSONB => "JSON".to_string(),
            ColumnType::DECIMAL { precision, scale } => {
                format!("DECIMAL({}, {})", precision, scale)
            }
            ColumnType::FLOAT => "FLOAT".to_string(),
            ColumnType::DOUBLE => "DOUBLE".to_string(),
            ColumnType::TIME { .. } => "TIME".to_string(),
            ColumnType::BLOB => "BLOB".to_string(),
            ColumnType::UUID => "CHAR(36)".to_string(),
            // MySQLは名前付きENUM型をサポートしないため、TEXTにフォールバック
            // MySQL固有のインラインENUMはDialectSpecific { kind: "ENUM", ... } を使用
            ColumnType::Enum { .. } => "TEXT".to_string(),
            ColumnType::DialectSpecific { kind, params } => {
                self.format_dialect_specific(kind, params)
            }
            // 共通型はformat_common_sql_typeで処理済み
            _ => format_common_sql_type(column_type).unwrap_or_else(|| "TEXT".to_string()),
        }
    }

    fn format_dialect_specific(&self, kind: &str, params: &serde_json::Value) -> String {
        // valuesパラメータがある場合（例: ENUM('a', 'b', 'c') または SET('a', 'b', 'c')）
        if let Some(values) = params.get("values").and_then(|v| v.as_array()) {
            let values_str = values
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("'{}'", s.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(", ");
            return format!("{}({})", kind, values_str);
        }

        // lengthパラメータがある場合
        if let Some(length) = params.get("length").and_then(|v| v.as_u64()) {
            return format!("{}({})", kind, length);
        }

        // unsignedパラメータがtrueの場合
        if params
            .get("unsigned")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return format!("{} UNSIGNED", kind);
        }

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
    fn test_mysql_integer() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: None }),
            "INT"
        );
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: Some(2) }),
            "SMALLINT"
        );
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: Some(8) }),
            "BIGINT"
        );
    }

    #[test]
    fn test_mysql_varchar() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::VARCHAR { length: 255 }),
            "VARCHAR(255)"
        );
    }

    #[test]
    fn test_mysql_boolean() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(service.to_sql_type(&ColumnType::BOOLEAN), "BOOLEAN");
    }

    #[test]
    fn test_mysql_uuid() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(service.to_sql_type(&ColumnType::UUID), "CHAR(36)");
    }

    #[test]
    fn test_mysql_jsonb_fallback() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(service.to_sql_type(&ColumnType::JSONB), "JSON");
    }

    #[test]
    fn test_mysql_parse_int() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("int", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { .. }));
    }

    #[test]
    fn test_mysql_parse_varchar() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let metadata = TypeMetadata {
            char_max_length: Some(200),
            ..Default::default()
        };

        let result = service.from_sql_type("varchar", &metadata).unwrap();
        assert!(matches!(result, ColumnType::VARCHAR { length: 200 }));
    }

    #[test]
    fn test_mysql_dialect_specific_with_values() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let params = serde_json::json!({ "values": ["a", "b", "c"] });
        let col_type = ColumnType::DialectSpecific {
            kind: "ENUM".to_string(),
            params,
        };

        assert_eq!(service.to_sql_type(&col_type), "ENUM('a', 'b', 'c')");
    }

    #[test]
    fn test_mysql_dialect_specific_unsigned() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let params = serde_json::json!({ "unsigned": true });
        let col_type = ColumnType::DialectSpecific {
            kind: "TINYINT".to_string(),
            params,
        };

        assert_eq!(service.to_sql_type(&col_type), "TINYINT UNSIGNED");
    }
}
