// PostgreSQL用型マッパー

use super::common::format_common_sql_type;
use super::TypeMapper;
use super::TypeMetadata;
use crate::adapters::sql_quote::quote_identifier_postgres;
use crate::core::schema::ColumnType;

/// PostgreSQL用型マッパー
pub struct PostgresTypeMapper;

impl TypeMapper for PostgresTypeMapper {
    fn parse_sql_type(&self, sql_type: &str, metadata: &TypeMetadata) -> Option<ColumnType> {
        match sql_type {
            "integer" | "int4" => Some(ColumnType::INTEGER { precision: None }),
            "smallint" | "int2" => Some(ColumnType::INTEGER { precision: Some(2) }),
            "bigint" | "int8" => Some(ColumnType::INTEGER { precision: Some(8) }),
            "character varying" | "varchar" => Some(ColumnType::VARCHAR {
                length: metadata.char_max_length.unwrap_or(255),
            }),
            "text" => Some(ColumnType::TEXT),
            "boolean" | "bool" => Some(ColumnType::BOOLEAN),
            "timestamp with time zone" | "timestamptz" => Some(ColumnType::TIMESTAMP {
                with_time_zone: Some(true),
            }),
            "timestamp without time zone" | "timestamp" => Some(ColumnType::TIMESTAMP {
                with_time_zone: None,
            }),
            "json" => Some(ColumnType::JSON),
            "jsonb" => Some(ColumnType::JSONB),
            "numeric" | "decimal" => Some(ColumnType::DECIMAL {
                precision: metadata.numeric_precision.unwrap_or(10),
                scale: metadata.numeric_scale.unwrap_or(0),
            }),
            "real" | "float4" => Some(ColumnType::FLOAT),
            "double precision" | "float8" => Some(ColumnType::DOUBLE),
            "character" | "char" => Some(ColumnType::CHAR {
                length: metadata.char_max_length.unwrap_or(1),
            }),
            "date" => Some(ColumnType::DATE),
            "time with time zone" | "timetz" => Some(ColumnType::TIME {
                with_time_zone: Some(true),
            }),
            "time without time zone" | "time" => Some(ColumnType::TIME {
                with_time_zone: None,
            }),
            "bytea" => Some(ColumnType::BLOB),
            "uuid" => Some(ColumnType::UUID),
            "USER-DEFINED" => {
                // ENUM型のチェック
                if let (Some(enum_names), Some(udt_name)) =
                    (&metadata.enum_names, &metadata.udt_name)
                {
                    if enum_names.contains(udt_name) {
                        return Some(ColumnType::Enum {
                            name: udt_name.clone(),
                        });
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn format_sql_type(&self, column_type: &ColumnType, auto_increment: Option<bool>) -> String {
        // 共通型を先にチェック（ただしauto_incrementの場合はINTEGER系を先に処理）
        if !auto_increment.unwrap_or(false) {
            if let Some(sql) = format_common_sql_type(column_type) {
                return sql;
            }
        }

        match column_type {
            ColumnType::INTEGER { precision } => {
                if auto_increment.unwrap_or(false) {
                    match precision {
                        Some(8) => "BIGSERIAL".to_string(),
                        Some(2) => "SMALLSERIAL".to_string(),
                        _ => "SERIAL".to_string(),
                    }
                } else {
                    match precision {
                        Some(2) => "SMALLINT".to_string(),
                        Some(8) => "BIGINT".to_string(),
                        _ => "INTEGER".to_string(),
                    }
                }
            }
            ColumnType::TIMESTAMP { with_time_zone } => {
                if with_time_zone.unwrap_or(false) {
                    "TIMESTAMP WITH TIME ZONE".to_string()
                } else {
                    "TIMESTAMP".to_string()
                }
            }
            ColumnType::JSONB => "JSONB".to_string(),
            ColumnType::DECIMAL { precision, scale } => {
                format!("NUMERIC({}, {})", precision, scale)
            }
            ColumnType::FLOAT => "REAL".to_string(),
            ColumnType::DOUBLE => "DOUBLE PRECISION".to_string(),
            ColumnType::TIME { with_time_zone } => {
                if with_time_zone.unwrap_or(false) {
                    "TIME WITH TIME ZONE".to_string()
                } else {
                    "TIME".to_string()
                }
            }
            ColumnType::BLOB => "BYTEA".to_string(),
            ColumnType::UUID => "UUID".to_string(),
            ColumnType::Enum { name } => quote_identifier_postgres(name),
            ColumnType::DialectSpecific { kind, params } => {
                self.format_dialect_specific(kind, params)
            }
            // 共通型はformat_common_sql_typeで処理済み
            _ => format_common_sql_type(column_type).unwrap_or_else(|| "TEXT".to_string()),
        }
    }

    fn format_dialect_specific(&self, kind: &str, params: &serde_json::Value) -> String {
        // lengthパラメータがある場合（例: VARBIT(16)）
        if let Some(length) = params.get("length").and_then(|v| v.as_u64()) {
            return format!("{}({})", kind, length);
        }

        // arrayパラメータがtrueの場合（例: TEXT[]）
        if params
            .get("array")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return format!("{}[]", kind);
        }

        // valuesパラメータがある場合（例: ENUM('a', 'b', 'c')）
        if let Some(values) = params.get("values").and_then(|v| v.as_array()) {
            let values_str = values
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("'{}'", s.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(", ");
            return format!("{}({})", kind, values_str);
        }

        kind.to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::adapters::type_mapping::TypeMappingService;
    use crate::core::config::Dialect;
    use crate::core::schema::ColumnType;
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn test_postgres_integer() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: None }),
            "INTEGER"
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
    fn test_postgres_serial() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type_with_auto_increment(
                &ColumnType::INTEGER { precision: None },
                Some(true)
            ),
            "SERIAL"
        );
        assert_eq!(
            service.to_sql_type_with_auto_increment(
                &ColumnType::INTEGER { precision: Some(8) },
                Some(true)
            ),
            "BIGSERIAL"
        );
        assert_eq!(
            service.to_sql_type_with_auto_increment(
                &ColumnType::INTEGER { precision: Some(2) },
                Some(true)
            ),
            "SMALLSERIAL"
        );
    }

    #[test]
    fn test_postgres_varchar() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::VARCHAR { length: 255 }),
            "VARCHAR(255)"
        );
    }

    #[test]
    fn test_postgres_timestamp() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::TIMESTAMP {
                with_time_zone: Some(true)
            }),
            "TIMESTAMP WITH TIME ZONE"
        );
        assert_eq!(
            service.to_sql_type(&ColumnType::TIMESTAMP {
                with_time_zone: Some(false)
            }),
            "TIMESTAMP"
        );
    }

    #[test]
    fn test_postgres_decimal() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::DECIMAL {
                precision: 10,
                scale: 2
            }),
            "NUMERIC(10, 2)"
        );
    }

    #[test]
    fn test_postgres_float_double() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(service.to_sql_type(&ColumnType::FLOAT), "REAL");
        assert_eq!(service.to_sql_type(&ColumnType::DOUBLE), "DOUBLE PRECISION");
    }

    #[test]
    fn test_postgres_blob() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(service.to_sql_type(&ColumnType::BLOB), "BYTEA");
    }

    #[test]
    fn test_postgres_uuid() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(service.to_sql_type(&ColumnType::UUID), "UUID");
    }

    #[test]
    fn test_postgres_json_jsonb() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(service.to_sql_type(&ColumnType::JSON), "JSON");
        assert_eq!(service.to_sql_type(&ColumnType::JSONB), "JSONB");
    }

    #[test]
    fn test_postgres_parse_integer() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("integer", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { precision: None }));

        let result = service.from_sql_type("bigint", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { precision: Some(8) }));
    }

    #[test]
    fn test_postgres_parse_varchar() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let metadata = TypeMetadata {
            char_max_length: Some(100),
            ..Default::default()
        };

        let result = service
            .from_sql_type("character varying", &metadata)
            .unwrap();
        assert!(matches!(result, ColumnType::VARCHAR { length: 100 }));
    }

    #[test]
    fn test_postgres_parse_timestamp() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let metadata = TypeMetadata::default();

        let result = service
            .from_sql_type("timestamp with time zone", &metadata)
            .unwrap();
        assert!(matches!(
            result,
            ColumnType::TIMESTAMP {
                with_time_zone: Some(true)
            }
        ));

        let result = service
            .from_sql_type("timestamp without time zone", &metadata)
            .unwrap();
        assert!(matches!(
            result,
            ColumnType::TIMESTAMP {
                with_time_zone: None
            }
        ));
    }

    #[test]
    fn test_postgres_parse_enum() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let mut enum_names = HashSet::new();
        enum_names.insert("status".to_string());

        let metadata = TypeMetadata {
            udt_name: Some("status".to_string()),
            enum_names: Some(enum_names),
            ..Default::default()
        };

        let result = service.from_sql_type("USER-DEFINED", &metadata).unwrap();
        assert!(matches!(result, ColumnType::Enum { name } if name == "status"));
    }

    #[test]
    fn test_postgres_parse_unknown_returns_text() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("unknown_type", &metadata).unwrap();
        assert!(matches!(result, ColumnType::TEXT));
    }

    #[test]
    fn test_postgres_dialect_specific_with_length() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let params = serde_json::json!({ "length": 16 });
        let col_type = ColumnType::DialectSpecific {
            kind: "VARBIT".to_string(),
            params,
        };

        assert_eq!(service.to_sql_type(&col_type), "VARBIT(16)");
    }

    #[test]
    fn test_postgres_dialect_specific_array() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let params = serde_json::json!({ "array": true });
        let col_type = ColumnType::DialectSpecific {
            kind: "TEXT".to_string(),
            params,
        };

        assert_eq!(service.to_sql_type(&col_type), "TEXT[]");
    }

    #[test]
    fn test_postgres_dialect_specific_values() {
        let mapper = PostgresTypeMapper;
        let params = serde_json::json!({ "values": ["a", "b", "c"] });
        let result = mapper.format_dialect_specific("CUSTOM", &params);
        assert_eq!(result, "CUSTOM('a', 'b', 'c')");
    }

    #[test]
    fn test_postgres_dialect_specific_no_params() {
        let mapper = PostgresTypeMapper;
        let params = serde_json::json!({});
        let result = mapper.format_dialect_specific("CITEXT", &params);
        assert_eq!(result, "CITEXT");
    }

    #[test]
    fn test_postgres_parse_alternative_type_names() {
        let mapper = PostgresTypeMapper;
        let meta = TypeMetadata::default();

        // int aliases
        assert!(matches!(
            mapper.parse_sql_type("int4", &meta),
            Some(ColumnType::INTEGER { precision: None })
        ));
        assert!(matches!(
            mapper.parse_sql_type("int2", &meta),
            Some(ColumnType::INTEGER { precision: Some(2) })
        ));
        assert!(matches!(
            mapper.parse_sql_type("smallint", &meta),
            Some(ColumnType::INTEGER { precision: Some(2) })
        ));
        assert!(matches!(
            mapper.parse_sql_type("int8", &meta),
            Some(ColumnType::INTEGER { precision: Some(8) })
        ));

        // varchar alias
        let meta_len = TypeMetadata {
            char_max_length: Some(50),
            ..Default::default()
        };
        assert!(matches!(
            mapper.parse_sql_type("varchar", &meta_len),
            Some(ColumnType::VARCHAR { length: 50 })
        ));

        // boolean aliases
        assert!(matches!(
            mapper.parse_sql_type("boolean", &meta),
            Some(ColumnType::BOOLEAN)
        ));
        assert!(matches!(
            mapper.parse_sql_type("bool", &meta),
            Some(ColumnType::BOOLEAN)
        ));

        // text
        assert!(matches!(
            mapper.parse_sql_type("text", &meta),
            Some(ColumnType::TEXT)
        ));

        // timestamptz alias
        assert!(matches!(
            mapper.parse_sql_type("timestamptz", &meta),
            Some(ColumnType::TIMESTAMP {
                with_time_zone: Some(true)
            })
        ));
        assert!(matches!(
            mapper.parse_sql_type("timestamp", &meta),
            Some(ColumnType::TIMESTAMP {
                with_time_zone: None
            })
        ));

        // json/jsonb
        assert!(matches!(
            mapper.parse_sql_type("json", &meta),
            Some(ColumnType::JSON)
        ));
        assert!(matches!(
            mapper.parse_sql_type("jsonb", &meta),
            Some(ColumnType::JSONB)
        ));

        // numeric/decimal
        let meta_num = TypeMetadata {
            numeric_precision: Some(8),
            numeric_scale: Some(2),
            ..Default::default()
        };
        assert!(matches!(
            mapper.parse_sql_type("numeric", &meta_num),
            Some(ColumnType::DECIMAL {
                precision: 8,
                scale: 2
            })
        ));
        assert!(matches!(
            mapper.parse_sql_type("decimal", &meta_num),
            Some(ColumnType::DECIMAL {
                precision: 8,
                scale: 2
            })
        ));

        // float types
        assert!(matches!(
            mapper.parse_sql_type("real", &meta),
            Some(ColumnType::FLOAT)
        ));
        assert!(matches!(
            mapper.parse_sql_type("float4", &meta),
            Some(ColumnType::FLOAT)
        ));
        assert!(matches!(
            mapper.parse_sql_type("double precision", &meta),
            Some(ColumnType::DOUBLE)
        ));
        assert!(matches!(
            mapper.parse_sql_type("float8", &meta),
            Some(ColumnType::DOUBLE)
        ));

        // char
        let meta_char = TypeMetadata {
            char_max_length: Some(10),
            ..Default::default()
        };
        assert!(matches!(
            mapper.parse_sql_type("character", &meta_char),
            Some(ColumnType::CHAR { length: 10 })
        ));
        assert!(matches!(
            mapper.parse_sql_type("char", &meta_char),
            Some(ColumnType::CHAR { length: 10 })
        ));

        // date
        assert!(matches!(
            mapper.parse_sql_type("date", &meta),
            Some(ColumnType::DATE)
        ));

        // time variants
        assert!(matches!(
            mapper.parse_sql_type("time with time zone", &meta),
            Some(ColumnType::TIME {
                with_time_zone: Some(true)
            })
        ));
        assert!(matches!(
            mapper.parse_sql_type("timetz", &meta),
            Some(ColumnType::TIME {
                with_time_zone: Some(true)
            })
        ));
        assert!(matches!(
            mapper.parse_sql_type("time without time zone", &meta),
            Some(ColumnType::TIME {
                with_time_zone: None
            })
        ));
        assert!(matches!(
            mapper.parse_sql_type("time", &meta),
            Some(ColumnType::TIME {
                with_time_zone: None
            })
        ));

        // bytea
        assert!(matches!(
            mapper.parse_sql_type("bytea", &meta),
            Some(ColumnType::BLOB)
        ));

        // uuid
        assert!(matches!(
            mapper.parse_sql_type("uuid", &meta),
            Some(ColumnType::UUID)
        ));

        // unknown
        assert!(mapper.parse_sql_type("unknown_xyz", &meta).is_none());
    }

    #[test]
    fn test_postgres_format_time() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::TIME {
                with_time_zone: Some(true)
            }),
            "TIME WITH TIME ZONE"
        );
        assert_eq!(
            service.to_sql_type(&ColumnType::TIME {
                with_time_zone: Some(false)
            }),
            "TIME"
        );
    }

    #[test]
    fn test_postgres_format_enum() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::Enum {
                name: "status".to_string()
            }),
            "\"status\""
        );
    }

    #[test]
    fn test_postgres_user_defined_not_enum() {
        let mapper = PostgresTypeMapper;
        // USER-DEFINED but no enum_names match
        let meta = TypeMetadata {
            udt_name: Some("geometry".to_string()),
            enum_names: Some(HashSet::new()),
            ..Default::default()
        };
        assert!(mapper.parse_sql_type("USER-DEFINED", &meta).is_none());

        // USER-DEFINED with no metadata
        let meta_empty = TypeMetadata::default();
        assert!(mapper.parse_sql_type("USER-DEFINED", &meta_empty).is_none());
    }
}
