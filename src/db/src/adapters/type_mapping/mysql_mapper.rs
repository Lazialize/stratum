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
                // MySQL の display width（INT(10) 等）はストリップする。
                // Strata の precision は意味的な精度であり、表示幅ではない。
                precision: None,
            }),
            "smallint" => Some(ColumnType::INTEGER { precision: Some(2) }),
            "bigint" => Some(ColumnType::INTEGER { precision: Some(8) }),
            "tinyint" => {
                // UNSIGNED 修飾子がある場合は DialectSpecific として返す
                if metadata.is_unsigned {
                    return Some(ColumnType::DialectSpecific {
                        kind: "TINYINT".to_string(),
                        params: serde_json::json!({ "unsigned": true }),
                    });
                }
                // MySQL の BOOLEAN は TINYINT(1) として格納される。
                // information_schema では data_type="tinyint", numeric_precision=3。
                // precision が 3 の場合は BOOLEAN として認識する。
                if metadata.numeric_precision == Some(3) {
                    Some(ColumnType::BOOLEAN)
                } else {
                    Some(ColumnType::INTEGER {
                        // MySQL の display width をストリップする
                        precision: None,
                    })
                }
            }
            "mediumint" => {
                // MEDIUMINT は MySQL 固有の型
                let mut params = serde_json::Map::new();
                if metadata.is_unsigned {
                    params.insert("unsigned".to_string(), serde_json::json!(true));
                }
                Some(ColumnType::DialectSpecific {
                    kind: "MEDIUMINT".to_string(),
                    params: serde_json::Value::Object(params),
                })
            }
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
            "year" => {
                // YEAR は MySQL 固有の型
                Some(ColumnType::DialectSpecific {
                    kind: "YEAR".to_string(),
                    params: serde_json::json!({}),
                })
            }
            "blob" | "longblob" | "mediumblob" | "tinyblob" => Some(ColumnType::BLOB),
            "enum" => {
                // MySQL の ENUM 型を DialectSpecific として返す
                if let Some(ref values) = metadata.enum_values {
                    Some(ColumnType::DialectSpecific {
                        kind: "ENUM".to_string(),
                        params: serde_json::json!({ "values": values }),
                    })
                } else {
                    // ENUM値が取得できない場合は TEXT にフォールバック
                    Some(ColumnType::TEXT)
                }
            }
            "set" => {
                // MySQL の SET 型を DialectSpecific として返す
                if let Some(ref values) = metadata.set_values {
                    Some(ColumnType::DialectSpecific {
                        kind: "SET".to_string(),
                        params: serde_json::json!({ "values": values }),
                    })
                } else {
                    // SET値が取得できない場合は TEXT にフォールバック
                    Some(ColumnType::TEXT)
                }
            }
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

    #[test]
    fn test_mysql_parse_smallint() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();
        let result = mapper.parse_sql_type("smallint", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { precision: Some(2) }));
    }

    #[test]
    fn test_mysql_parse_bigint() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();
        let result = mapper.parse_sql_type("bigint", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { precision: Some(8) }));
    }

    #[test]
    fn test_mysql_parse_tinyint() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            numeric_precision: Some(1),
            ..Default::default()
        };
        let result = mapper.parse_sql_type("tinyint", &metadata).unwrap();
        // display width はストリップされるので precision: None になる
        assert!(matches!(result, ColumnType::INTEGER { precision: None }));
    }

    #[test]
    fn test_mysql_parse_tinyint_1_is_boolean() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();
        let result = mapper.parse_sql_type("tinyint(1)", &metadata).unwrap();
        assert!(matches!(result, ColumnType::BOOLEAN));
    }

    #[test]
    fn test_mysql_parse_text_types() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();

        let result = mapper.parse_sql_type("text", &metadata).unwrap();
        assert!(matches!(result, ColumnType::TEXT));

        let result = mapper.parse_sql_type("longtext", &metadata).unwrap();
        assert!(matches!(result, ColumnType::TEXT));

        let result = mapper.parse_sql_type("mediumtext", &metadata).unwrap();
        assert!(matches!(result, ColumnType::TEXT));
    }

    #[test]
    fn test_mysql_parse_blob_types() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();

        let result = mapper.parse_sql_type("blob", &metadata).unwrap();
        assert!(matches!(result, ColumnType::BLOB));

        let result = mapper.parse_sql_type("longblob", &metadata).unwrap();
        assert!(matches!(result, ColumnType::BLOB));

        let result = mapper.parse_sql_type("mediumblob", &metadata).unwrap();
        assert!(matches!(result, ColumnType::BLOB));
    }

    #[test]
    fn test_mysql_parse_date() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();
        let result = mapper.parse_sql_type("date", &metadata).unwrap();
        assert!(matches!(result, ColumnType::DATE));
    }

    #[test]
    fn test_mysql_parse_time() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();
        let result = mapper.parse_sql_type("time", &metadata).unwrap();
        assert!(matches!(
            result,
            ColumnType::TIME {
                with_time_zone: None
            }
        ));
    }

    #[test]
    fn test_mysql_parse_datetime_and_timestamp() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();

        let result = mapper.parse_sql_type("datetime", &metadata).unwrap();
        assert!(matches!(
            result,
            ColumnType::TIMESTAMP {
                with_time_zone: None
            }
        ));

        let result = mapper.parse_sql_type("timestamp", &metadata).unwrap();
        assert!(matches!(
            result,
            ColumnType::TIMESTAMP {
                with_time_zone: None
            }
        ));
    }

    #[test]
    fn test_mysql_parse_json() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();
        let result = mapper.parse_sql_type("json", &metadata).unwrap();
        assert!(matches!(result, ColumnType::JSON));
    }

    #[test]
    fn test_mysql_parse_decimal() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            numeric_precision: Some(10),
            numeric_scale: Some(2),
            ..Default::default()
        };
        let result = mapper.parse_sql_type("decimal", &metadata).unwrap();
        assert!(matches!(
            result,
            ColumnType::DECIMAL {
                precision: 10,
                scale: 2
            }
        ));
    }

    #[test]
    fn test_mysql_parse_float_double() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();

        let result = mapper.parse_sql_type("float", &metadata).unwrap();
        assert!(matches!(result, ColumnType::FLOAT));

        let result = mapper.parse_sql_type("double", &metadata).unwrap();
        assert!(matches!(result, ColumnType::DOUBLE));
    }

    #[test]
    fn test_mysql_parse_char() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            char_max_length: Some(10),
            ..Default::default()
        };
        let result = mapper.parse_sql_type("char", &metadata).unwrap();
        assert!(matches!(result, ColumnType::CHAR { length: 10 }));
    }

    #[test]
    fn test_mysql_parse_unknown_returns_none() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();
        let result = mapper.parse_sql_type("unknowntype", &metadata);
        assert!(result.is_none());
    }

    #[test]
    fn test_mysql_format_timestamp() {
        let mapper = MySqlTypeMapper;
        let result = mapper.format_sql_type(
            &ColumnType::TIMESTAMP {
                with_time_zone: None,
            },
            None,
        );
        assert_eq!(result, "TIMESTAMP");
    }

    #[test]
    fn test_mysql_format_time() {
        let mapper = MySqlTypeMapper;
        let result = mapper.format_sql_type(
            &ColumnType::TIME {
                with_time_zone: None,
            },
            None,
        );
        assert_eq!(result, "TIME");
    }

    #[test]
    fn test_mysql_format_blob() {
        let mapper = MySqlTypeMapper;
        let result = mapper.format_sql_type(&ColumnType::BLOB, None);
        assert_eq!(result, "BLOB");
    }

    #[test]
    fn test_mysql_format_float_double() {
        let mapper = MySqlTypeMapper;
        assert_eq!(mapper.format_sql_type(&ColumnType::FLOAT, None), "FLOAT");
        assert_eq!(mapper.format_sql_type(&ColumnType::DOUBLE, None), "DOUBLE");
    }

    #[test]
    fn test_mysql_format_decimal() {
        let mapper = MySqlTypeMapper;
        let result = mapper.format_sql_type(
            &ColumnType::DECIMAL {
                precision: 10,
                scale: 2,
            },
            None,
        );
        assert_eq!(result, "DECIMAL(10, 2)");
    }

    #[test]
    fn test_mysql_format_enum_fallback() {
        let mapper = MySqlTypeMapper;
        let result = mapper.format_sql_type(
            &ColumnType::Enum {
                name: "status".to_string(),
            },
            None,
        );
        assert_eq!(result, "TEXT");
    }

    #[test]
    fn test_mysql_dialect_specific_with_length() {
        let mapper = MySqlTypeMapper;
        let params = serde_json::json!({ "length": 20 });
        let result = mapper.format_dialect_specific("VARBINARY", &params);
        assert_eq!(result, "VARBINARY(20)");
    }

    #[test]
    fn test_mysql_dialect_specific_no_params() {
        let mapper = MySqlTypeMapper;
        let params = serde_json::json!({});
        let result = mapper.format_dialect_specific("TINYINT", &params);
        assert_eq!(result, "TINYINT");
    }

    #[test]
    fn test_mysql_parse_enum_with_values() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            enum_values: Some(vec![
                "draft".to_string(),
                "published".to_string(),
                "archived".to_string(),
            ]),
            ..Default::default()
        };

        let result = mapper.parse_sql_type("enum", &metadata).unwrap();
        match result {
            ColumnType::DialectSpecific { kind, params } => {
                assert_eq!(kind, "ENUM");
                let values = params.get("values").unwrap().as_array().unwrap();
                assert_eq!(values.len(), 3);
                assert_eq!(values[0].as_str().unwrap(), "draft");
                assert_eq!(values[1].as_str().unwrap(), "published");
                assert_eq!(values[2].as_str().unwrap(), "archived");
            }
            _ => panic!("Expected DialectSpecific, got {:?}", result),
        }
    }

    #[test]
    fn test_mysql_parse_enum_without_values() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();

        let result = mapper.parse_sql_type("enum", &metadata).unwrap();
        assert!(matches!(result, ColumnType::TEXT));
    }

    // =========================================================================
    // Issue #26: MySQL INTEGER display width regression tests
    // =========================================================================

    #[test]
    fn test_mysql_parse_int_strips_display_width() {
        // MySQL の information_schema は INT に numeric_precision=10 を返す。
        // Strata ではこの display width をストリップして precision: None にすべき。
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            numeric_precision: Some(10),
            ..Default::default()
        };
        let result = mapper.parse_sql_type("int", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { precision: None }));
    }

    #[test]
    fn test_mysql_parse_integer_strips_display_width() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            numeric_precision: Some(10),
            ..Default::default()
        };
        let result = mapper.parse_sql_type("integer", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { precision: None }));
    }

    #[test]
    fn test_mysql_parse_int_11_strips_display_width() {
        // INT(11) (signed INT のデフォルト display width) もストリップされる
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            numeric_precision: Some(11),
            ..Default::default()
        };
        let result = mapper.parse_sql_type("int", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { precision: None }));
    }

    #[test]
    fn test_mysql_parse_tinyint_non_boolean_strips_display_width() {
        // TINYINT で precision != 3 の場合も display width をストリップする
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            numeric_precision: Some(4),
            ..Default::default()
        };
        let result = mapper.parse_sql_type("tinyint", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { precision: None }));
    }

    #[test]
    fn test_mysql_int_roundtrip_no_spurious_precision() {
        // INT → export → re-import で precision が付与されないことを確認
        let service = TypeMappingService::new(Dialect::MySQL);
        let mapper = MySqlTypeMapper;

        // MySQL introspection が返す metadata をシミュレート
        let metadata = TypeMetadata {
            numeric_precision: Some(10),
            ..Default::default()
        };

        // parse: "int" + precision=10 → INTEGER { precision: None }
        let parsed = mapper.parse_sql_type("int", &metadata).unwrap();
        assert!(matches!(parsed, ColumnType::INTEGER { precision: None }));

        // format: INTEGER { precision: None } → "INT"
        let sql = service.to_sql_type(&parsed);
        assert_eq!(sql, "INT");
    }

    // =========================================================================
    // Issue #25: MySQL dialect-specific type regression tests
    // =========================================================================

    #[test]
    fn test_mysql_parse_tinyint_unsigned() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            numeric_precision: Some(3),
            is_unsigned: true,
            ..Default::default()
        };
        let result = mapper.parse_sql_type("tinyint", &metadata).unwrap();
        match result {
            ColumnType::DialectSpecific { kind, params } => {
                assert_eq!(kind, "TINYINT");
                assert_eq!(params.get("unsigned").and_then(|v| v.as_bool()), Some(true));
            }
            _ => panic!("Expected DialectSpecific TINYINT, got {:?}", result),
        }
    }

    #[test]
    fn test_mysql_parse_tinyint_signed_is_boolean() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            numeric_precision: Some(3),
            is_unsigned: false,
            ..Default::default()
        };
        let result = mapper.parse_sql_type("tinyint", &metadata).unwrap();
        assert!(matches!(result, ColumnType::BOOLEAN));
    }

    #[test]
    fn test_mysql_parse_mediumint() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();
        let result = mapper.parse_sql_type("mediumint", &metadata).unwrap();
        match result {
            ColumnType::DialectSpecific { kind, params } => {
                assert_eq!(kind, "MEDIUMINT");
                assert!(params.get("unsigned").is_none());
            }
            _ => panic!("Expected DialectSpecific MEDIUMINT, got {:?}", result),
        }
    }

    #[test]
    fn test_mysql_parse_mediumint_unsigned() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            is_unsigned: true,
            ..Default::default()
        };
        let result = mapper.parse_sql_type("mediumint", &metadata).unwrap();
        match result {
            ColumnType::DialectSpecific { kind, params } => {
                assert_eq!(kind, "MEDIUMINT");
                assert_eq!(params.get("unsigned").and_then(|v| v.as_bool()), Some(true));
            }
            _ => panic!(
                "Expected DialectSpecific MEDIUMINT UNSIGNED, got {:?}",
                result
            ),
        }
    }

    #[test]
    fn test_mysql_parse_set_with_values() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata {
            set_values: Some(vec![
                "read".to_string(),
                "write".to_string(),
                "execute".to_string(),
                "admin".to_string(),
            ]),
            ..Default::default()
        };
        let result = mapper.parse_sql_type("set", &metadata).unwrap();
        match result {
            ColumnType::DialectSpecific { kind, params } => {
                assert_eq!(kind, "SET");
                let values = params.get("values").unwrap().as_array().unwrap();
                assert_eq!(values.len(), 4);
                assert_eq!(values[0].as_str().unwrap(), "read");
                assert_eq!(values[1].as_str().unwrap(), "write");
                assert_eq!(values[2].as_str().unwrap(), "execute");
                assert_eq!(values[3].as_str().unwrap(), "admin");
            }
            _ => panic!("Expected DialectSpecific SET, got {:?}", result),
        }
    }

    #[test]
    fn test_mysql_parse_set_without_values() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();
        let result = mapper.parse_sql_type("set", &metadata).unwrap();
        assert!(matches!(result, ColumnType::TEXT));
    }

    #[test]
    fn test_mysql_parse_year() {
        let mapper = MySqlTypeMapper;
        let metadata = TypeMetadata::default();
        let result = mapper.parse_sql_type("year", &metadata).unwrap();
        match result {
            ColumnType::DialectSpecific { kind, .. } => {
                assert_eq!(kind, "YEAR");
            }
            _ => panic!("Expected DialectSpecific YEAR, got {:?}", result),
        }
    }

    #[test]
    fn test_mysql_tinyint_unsigned_roundtrip() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let col_type = ColumnType::DialectSpecific {
            kind: "TINYINT".to_string(),
            params: serde_json::json!({ "unsigned": true }),
        };
        assert_eq!(service.to_sql_type(&col_type), "TINYINT UNSIGNED");
    }

    #[test]
    fn test_mysql_mediumint_unsigned_roundtrip() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let col_type = ColumnType::DialectSpecific {
            kind: "MEDIUMINT".to_string(),
            params: serde_json::json!({ "unsigned": true }),
        };
        assert_eq!(service.to_sql_type(&col_type), "MEDIUMINT UNSIGNED");
    }

    #[test]
    fn test_mysql_set_roundtrip() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let col_type = ColumnType::DialectSpecific {
            kind: "SET".to_string(),
            params: serde_json::json!({ "values": ["read", "write", "execute"] }),
        };
        assert_eq!(
            service.to_sql_type(&col_type),
            "SET('read', 'write', 'execute')"
        );
    }

    #[test]
    fn test_mysql_year_roundtrip() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let col_type = ColumnType::DialectSpecific {
            kind: "YEAR".to_string(),
            params: serde_json::json!({}),
        };
        assert_eq!(service.to_sql_type(&col_type), "YEAR");
    }

    #[test]
    fn test_mysql_mediumint_signed_roundtrip() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let col_type = ColumnType::DialectSpecific {
            kind: "MEDIUMINT".to_string(),
            params: serde_json::json!({}),
        };
        assert_eq!(service.to_sql_type(&col_type), "MEDIUMINT");
    }
}
