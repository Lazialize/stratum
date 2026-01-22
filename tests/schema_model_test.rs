/// スキーマドメインモデルのテスト
///
/// このテストは、スキーマ定義を表現する型システム（Schema, Table, Column, Index, Constraint）が
/// 正しく動作し、YAML形式とのシリアライズ/デシリアライズが可能であることを確認します。

#[cfg(test)]
mod schema_model_tests {
    use stratum::core::schema::{
        Column, ColumnType, Constraint, Index, Schema, Table,
    };
    use std::collections::HashMap;

    /// Schema構造体が正しくデシリアライズできることを確認
    #[test]
    fn test_schema_deserialization() {
        let yaml = r#"
version: "1.0"
tables:
  users:
    name: users
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: true
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        default_value: null
        auto_increment: null
    indexes:
      - name: idx_users_email
        columns: [email]
        unique: true
    constraints:
      - type: PRIMARY_KEY
        columns: [id]
"#;

        let schema: Schema = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(schema.version, "1.0");
        assert_eq!(schema.tables.len(), 1);
        assert!(schema.tables.contains_key("users"));

        let users_table = &schema.tables["users"];
        assert_eq!(users_table.name, "users");
        assert_eq!(users_table.columns.len(), 2);
        assert_eq!(users_table.indexes.len(), 1);
        assert_eq!(users_table.constraints.len(), 1);
    }

    /// Table構造体が正しく機能することを確認
    #[test]
    fn test_table_structure() {
        let table = Table {
            name: "products".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    column_type: ColumnType::INTEGER { precision: None },
                    nullable: false,
                    default_value: None,
                    auto_increment: Some(true),
                },
                Column {
                    name: "name".to_string(),
                    column_type: ColumnType::VARCHAR { length: 255 },
                    nullable: false,
                    default_value: None,
                    auto_increment: None,
                },
            ],
            indexes: vec![],
            constraints: vec![Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            }],
        };

        assert_eq!(table.name, "products");
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.get_primary_key_columns().unwrap(), vec!["id"]);
    }

    /// ColumnType列挙型が正しくシリアライズ/デシリアライズできることを確認
    #[test]
    fn test_column_type_serialization() {
        let integer_yaml = r#"
kind: INTEGER
precision: 8
"#;
        let col_type: ColumnType = serde_saphyr::from_str(integer_yaml).unwrap();
        match col_type {
            ColumnType::INTEGER { precision } => assert_eq!(precision, Some(8)),
            _ => panic!("Expected INTEGER type"),
        }

        let varchar_yaml = r#"
kind: VARCHAR
length: 100
"#;
        let col_type: ColumnType = serde_saphyr::from_str(varchar_yaml).unwrap();
        match col_type {
            ColumnType::VARCHAR { length } => assert_eq!(length, 100),
            _ => panic!("Expected VARCHAR type"),
        }

        let text_yaml = r#"
kind: TEXT
"#;
        let col_type: ColumnType = serde_saphyr::from_str(text_yaml).unwrap();
        assert!(matches!(col_type, ColumnType::TEXT));

        let boolean_yaml = r#"
kind: BOOLEAN
"#;
        let col_type: ColumnType = serde_saphyr::from_str(boolean_yaml).unwrap();
        assert!(matches!(col_type, ColumnType::BOOLEAN));

        let timestamp_yaml = r#"
kind: TIMESTAMP
with_time_zone: true
"#;
        let col_type: ColumnType = serde_saphyr::from_str(timestamp_yaml).unwrap();
        match col_type {
            ColumnType::TIMESTAMP { with_time_zone } => assert_eq!(with_time_zone, Some(true)),
            _ => panic!("Expected TIMESTAMP type"),
        }

        let json_yaml = r#"
kind: JSON
"#;
        let col_type: ColumnType = serde_saphyr::from_str(json_yaml).unwrap();
        assert!(matches!(col_type, ColumnType::JSON));
    }

    /// Constraint列挙型が正しくシリアライズ/デシリアライズできることを確認
    #[test]
    fn test_constraint_serialization() {
        let pk_yaml = r#"
type: PRIMARY_KEY
columns: [id]
"#;
        let constraint: Constraint = serde_saphyr::from_str(pk_yaml).unwrap();
        match constraint {
            Constraint::PRIMARY_KEY { columns } => assert_eq!(columns, vec!["id"]),
            _ => panic!("Expected PRIMARY_KEY constraint"),
        }

        let fk_yaml = r#"
type: FOREIGN_KEY
columns: [user_id]
referenced_table: users
referenced_columns: [id]
"#;
        let constraint: Constraint = serde_saphyr::from_str(fk_yaml).unwrap();
        match constraint {
            Constraint::FOREIGN_KEY {
                columns,
                referenced_table,
                referenced_columns,
            } => {
                assert_eq!(columns, vec!["user_id"]);
                assert_eq!(referenced_table, "users");
                assert_eq!(referenced_columns, vec!["id"]);
            }
            _ => panic!("Expected FOREIGN_KEY constraint"),
        }

        let unique_yaml = r#"
type: UNIQUE
columns: [email]
"#;
        let constraint: Constraint = serde_saphyr::from_str(unique_yaml).unwrap();
        match constraint {
            Constraint::UNIQUE { columns } => assert_eq!(columns, vec!["email"]),
            _ => panic!("Expected UNIQUE constraint"),
        }

        let check_yaml = r#"
type: CHECK
columns: [age]
check_expression: "age >= 0"
"#;
        let constraint: Constraint = serde_saphyr::from_str(check_yaml).unwrap();
        match constraint {
            Constraint::CHECK {
                columns,
                check_expression,
            } => {
                assert_eq!(columns, vec!["age"]);
                assert_eq!(check_expression, "age >= 0");
            }
            _ => panic!("Expected CHECK constraint"),
        }
    }

    /// Index構造体が正しく機能することを確認
    #[test]
    fn test_index_structure() {
        let index = Index {
            name: "idx_user_email".to_string(),
            columns: vec!["email".to_string()],
            unique: true,
        };

        assert_eq!(index.name, "idx_user_email");
        assert_eq!(index.columns, vec!["email"]);
        assert!(index.unique);
    }

    /// 複数テーブルのスキーマが正しく扱えることを確認
    #[test]
    fn test_multi_table_schema() {
        let mut tables = HashMap::new();

        tables.insert(
            "users".to_string(),
            Table {
                name: "users".to_string(),
                columns: vec![Column {
                    name: "id".to_string(),
                    column_type: ColumnType::INTEGER { precision: None },
                    nullable: false,
                    default_value: None,
                    auto_increment: Some(true),
                }],
                indexes: vec![],
                constraints: vec![],
            },
        );

        tables.insert(
            "posts".to_string(),
            Table {
                name: "posts".to_string(),
                columns: vec![
                    Column {
                        name: "id".to_string(),
                        column_type: ColumnType::INTEGER { precision: None },
                        nullable: false,
                        default_value: None,
                        auto_increment: Some(true),
                    },
                    Column {
                        name: "user_id".to_string(),
                        column_type: ColumnType::INTEGER { precision: None },
                        nullable: false,
                        default_value: None,
                        auto_increment: None,
                    },
                ],
                indexes: vec![],
                constraints: vec![Constraint::FOREIGN_KEY {
                    columns: vec!["user_id".to_string()],
                    referenced_table: "users".to_string(),
                    referenced_columns: vec!["id".to_string()],
                }],
            },
        );

        let schema = Schema {
            version: "1.0".to_string(),
            tables,
        };

        assert_eq!(schema.tables.len(), 2);
        assert!(schema.tables.contains_key("users"));
        assert!(schema.tables.contains_key("posts"));
    }

    /// Schemaのヘルパーメソッドが正しく動作することを確認
    #[test]
    fn test_schema_helper_methods() {
        let mut tables = HashMap::new();
        tables.insert(
            "users".to_string(),
            Table {
                name: "users".to_string(),
                columns: vec![],
                indexes: vec![],
                constraints: vec![],
            },
        );

        let schema = Schema {
            version: "1.0".to_string(),
            tables,
        };

        assert!(schema.has_table("users"));
        assert!(!schema.has_table("products"));
        assert_eq!(schema.get_table("users").unwrap().name, "users");
        assert!(schema.get_table("products").is_none());
    }

    // Phase 4: 新規データ型のシリアライゼーションテスト

    #[test]
    fn test_decimal_type_serialization() {
        let yaml = r#"
kind: DECIMAL
precision: 10
scale: 2
"#;
        let col_type: ColumnType = serde_saphyr::from_str(yaml).unwrap();
        match col_type {
            ColumnType::DECIMAL { precision, scale } => {
                assert_eq!(precision, 10);
                assert_eq!(scale, 2);
            }
            _ => panic!("Expected DECIMAL type"),
        }

        let serialized = serde_saphyr::to_string(&col_type).unwrap();
        assert!(serialized.contains("DECIMAL"));
        assert!(serialized.contains("precision: 10"));
        assert!(serialized.contains("scale: 2"));
    }

    #[test]
    fn test_float_type_serialization() {
        let yaml = r#"
kind: FLOAT
"#;
        let col_type: ColumnType = serde_saphyr::from_str(yaml).unwrap();
        match col_type {
            ColumnType::FLOAT => {}
            _ => panic!("Expected FLOAT type"),
        }

        let serialized = serde_saphyr::to_string(&col_type).unwrap();
        assert!(serialized.contains("FLOAT"));
    }

    #[test]
    fn test_double_type_serialization() {
        let yaml = r#"
kind: DOUBLE
"#;
        let col_type: ColumnType = serde_saphyr::from_str(yaml).unwrap();
        match col_type {
            ColumnType::DOUBLE => {}
            _ => panic!("Expected DOUBLE type"),
        }

        let serialized = serde_saphyr::to_string(&col_type).unwrap();
        assert!(serialized.contains("DOUBLE"));
    }

    #[test]
    fn test_char_type_serialization() {
        let yaml = r#"
kind: CHAR
length: 50
"#;
        let col_type: ColumnType = serde_saphyr::from_str(yaml).unwrap();
        match col_type {
            ColumnType::CHAR { length } => assert_eq!(length, 50),
            _ => panic!("Expected CHAR type"),
        }

        let serialized = serde_saphyr::to_string(&col_type).unwrap();
        assert!(serialized.contains("CHAR"));
        assert!(serialized.contains("length: 50"));
    }

    #[test]
    fn test_date_type_serialization() {
        let yaml = r#"
kind: DATE
"#;
        let col_type: ColumnType = serde_saphyr::from_str(yaml).unwrap();
        match col_type {
            ColumnType::DATE => {}
            _ => panic!("Expected DATE type"),
        }

        let serialized = serde_saphyr::to_string(&col_type).unwrap();
        assert!(serialized.contains("DATE"));
    }

    #[test]
    fn test_time_type_serialization() {
        let yaml_without_tz = r#"
kind: TIME
with_time_zone: null
"#;
        let col_type: ColumnType = serde_saphyr::from_str(yaml_without_tz).unwrap();
        match col_type {
            ColumnType::TIME { with_time_zone } => assert_eq!(with_time_zone, None),
            _ => panic!("Expected TIME type"),
        }

        let yaml_with_tz = r#"
kind: TIME
with_time_zone: true
"#;
        let col_type: ColumnType = serde_saphyr::from_str(yaml_with_tz).unwrap();
        match col_type {
            ColumnType::TIME { with_time_zone } => assert_eq!(with_time_zone, Some(true)),
            _ => panic!("Expected TIME type"),
        }
    }

    #[test]
    fn test_blob_type_serialization() {
        let yaml = r#"
kind: BLOB
"#;
        let col_type: ColumnType = serde_saphyr::from_str(yaml).unwrap();
        match col_type {
            ColumnType::BLOB => {}
            _ => panic!("Expected BLOB type"),
        }

        let serialized = serde_saphyr::to_string(&col_type).unwrap();
        assert!(serialized.contains("BLOB"));
    }

    #[test]
    fn test_uuid_type_serialization() {
        let yaml = r#"
kind: UUID
"#;
        let col_type: ColumnType = serde_saphyr::from_str(yaml).unwrap();
        match col_type {
            ColumnType::UUID => {}
            _ => panic!("Expected UUID type"),
        }

        let serialized = serde_saphyr::to_string(&col_type).unwrap();
        assert!(serialized.contains("UUID"));
    }

    #[test]
    fn test_jsonb_type_serialization() {
        let yaml = r#"
kind: JSONB
"#;
        let col_type: ColumnType = serde_saphyr::from_str(yaml).unwrap();
        match col_type {
            ColumnType::JSONB => {}
            _ => panic!("Expected JSONB type"),
        }

        let serialized = serde_saphyr::to_string(&col_type).unwrap();
        assert!(serialized.contains("JSONB"));
    }
}
