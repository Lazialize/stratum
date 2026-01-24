// DialectSpecific カラム型のシリアライゼーション・デシリアライゼーションテスト

use strata::core::schema::{Column, ColumnType};

#[cfg(test)]
mod dialect_specific_tests {
    use super::*;

    /// パラメータなしの方言固有型（SERIAL）のデシリアライゼーション
    #[test]
    fn test_deserialize_dialect_specific_without_params() {
        let yaml = r#"
name: id
type:
  kind: SERIAL
nullable: false
"#;

        let column: Column = serde_saphyr::from_str(yaml).expect("Failed to deserialize");

        assert_eq!(column.name, "id");
        assert!(!column.nullable);

        match column.column_type {
            ColumnType::DialectSpecific {
                ref kind,
                ref params,
            } => {
                assert_eq!(kind, "SERIAL");
                // paramsはnullまたは空オブジェクト
                assert!(
                    params.is_null() || params.as_object().map(|o| o.is_empty()).unwrap_or(false)
                );
            }
            _ => panic!(
                "Expected DialectSpecific variant, got {:?}",
                column.column_type
            ),
        }
    }

    /// パラメータありの方言固有型（ENUM with values）のデシリアライゼーション
    #[test]
    fn test_deserialize_dialect_specific_with_array_params() {
        let yaml = r#"
name: status
type:
  kind: ENUM
  values: ["active", "inactive", "pending"]
nullable: false
"#;

        let column: Column = serde_saphyr::from_str(yaml).expect("Failed to deserialize");

        assert_eq!(column.name, "status");
        assert!(!column.nullable);

        match column.column_type {
            ColumnType::DialectSpecific {
                ref kind,
                ref params,
            } => {
                assert_eq!(kind, "ENUM");

                // paramsにvaluesが含まれているか確認
                let values = params.get("values").expect("Expected 'values' parameter");
                let values_array = values.as_array().expect("Expected array");
                assert_eq!(values_array.len(), 3);
                assert_eq!(values_array[0].as_str().unwrap(), "active");
                assert_eq!(values_array[1].as_str().unwrap(), "inactive");
                assert_eq!(values_array[2].as_str().unwrap(), "pending");
            }
            _ => panic!(
                "Expected DialectSpecific variant, got {:?}",
                column.column_type
            ),
        }
    }

    /// パラメータありの方言固有型（VARBIT with length）のデシリアライゼーション
    #[test]
    fn test_deserialize_dialect_specific_with_numeric_param() {
        let yaml = r#"
name: flags
type:
  kind: VARBIT
  length: 16
nullable: true
"#;

        let column: Column = serde_saphyr::from_str(yaml).expect("Failed to deserialize");

        assert_eq!(column.name, "flags");
        assert!(column.nullable);

        match column.column_type {
            ColumnType::DialectSpecific {
                ref kind,
                ref params,
            } => {
                assert_eq!(kind, "VARBIT");

                // paramsにlengthが含まれているか確認
                let length = params.get("length").expect("Expected 'length' parameter");
                assert_eq!(length.as_u64().unwrap(), 16);
            }
            _ => panic!(
                "Expected DialectSpecific variant, got {:?}",
                column.column_type
            ),
        }
    }

    /// 既存の共通型とDialectSpecificバリアントの混在パターン
    #[test]
    fn test_deserialize_mixed_common_and_dialect_specific_types() {
        // 共通型（INTEGER）
        let yaml_common = r#"
name: count
type:
  kind: INTEGER
nullable: false
"#;

        let column_common: Column =
            serde_saphyr::from_str(yaml_common).expect("Failed to deserialize common type");
        match column_common.column_type {
            ColumnType::INTEGER { .. } => {
                // 既存の共通型は変更なく動作
            }
            _ => panic!("Expected INTEGER variant"),
        }

        // 方言固有型（SERIAL）
        let yaml_dialect = r#"
name: id
type:
  kind: SERIAL
nullable: false
"#;

        let column_dialect: Column = serde_saphyr::from_str(yaml_dialect)
            .expect("Failed to deserialize dialect-specific type");
        match column_dialect.column_type {
            ColumnType::DialectSpecific { ref kind, .. } => {
                assert_eq!(kind, "SERIAL");
            }
            _ => panic!("Expected DialectSpecific variant"),
        }
    }

    /// DialectSpecificバリアントのシリアライゼーション
    #[test]
    fn test_serialize_dialect_specific() {
        let column = Column {
            name: "status".to_string(),
            column_type: ColumnType::DialectSpecific {
                kind: "ENUM".to_string(),
                params: serde_json::json!({
                    "values": ["active", "inactive"]
                }),
            },
            nullable: false,
            default_value: None,
            auto_increment: None,
            renamed_from: None,
        };

        let yaml = serde_saphyr::to_string(&column).expect("Failed to serialize");

        // YAMLにkindとvaluesが含まれているか確認
        assert!(yaml.contains("kind: ENUM"));
        assert!(yaml.contains("values:"));
        assert!(yaml.contains("- active"));
        assert!(yaml.contains("- inactive"));
    }

    /// DialectSpecificバリアントのClone実装
    #[test]
    fn test_dialect_specific_clone() {
        let column_type = ColumnType::DialectSpecific {
            kind: "SERIAL".to_string(),
            params: serde_json::json!(null),
        };

        let cloned = column_type.clone();

        match cloned {
            ColumnType::DialectSpecific { ref kind, .. } => {
                assert_eq!(kind, "SERIAL");
            }
            _ => panic!("Expected DialectSpecific variant"),
        }
    }

    /// DialectSpecificバリアントのPartialEq実装
    #[test]
    fn test_dialect_specific_partial_eq() {
        let col_type1 = ColumnType::DialectSpecific {
            kind: "SERIAL".to_string(),
            params: serde_json::json!(null),
        };

        let col_type2 = ColumnType::DialectSpecific {
            kind: "SERIAL".to_string(),
            params: serde_json::json!(null),
        };

        let col_type3 = ColumnType::DialectSpecific {
            kind: "BIGSERIAL".to_string(),
            params: serde_json::json!(null),
        };

        assert_eq!(col_type1, col_type2);
        assert_ne!(col_type1, col_type3);
    }
}
