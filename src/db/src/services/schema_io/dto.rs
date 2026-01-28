// スキーマDTO
//
// YAML構造と内部モデルを分離するためのDTO層。
// 新構文のYAML（テーブル名はキー名、primary_keyは独立フィールド）をサポートします。

use crate::core::schema::{Column, EnumDefinition, Index, ReferentialAction};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ENUM再作成許可フラグのシリアライズ判定用ヘルパー
fn is_false(value: &bool) -> bool {
    !*value
}

/// YAML スキーマ用DTO
///
/// YAML構造を忠実に表現する中間データ型。
/// デシリアライズ・シリアライズ両方向で使用します。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDto {
    /// スキーマのバージョン
    pub version: String,

    /// ENUM再作成の許可フラグ（デフォルト: false）
    #[serde(default, skip_serializing_if = "is_false")]
    pub enum_recreate_allowed: bool,

    /// ENUM定義のマップ（型名 -> EnumDefinition）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub enums: HashMap<String, EnumDefinition>,

    /// テーブル定義のマップ（テーブル名 -> TableDto）
    pub tables: HashMap<String, TableDto>,
}

/// YAML テーブル定義用DTO
///
/// テーブル定義の中間表現。
/// `name`フィールドを持たず、キー名からテーブル名を取得します。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDto {
    /// カラム定義（必須）
    pub columns: Vec<Column>,

    /// 主キーカラム名のリスト（オプショナル）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_key: Option<Vec<String>>,

    /// インデックス定義（オプショナル、デフォルト: 空）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indexes: Vec<Index>,

    /// 制約定義（オプショナル、デフォルト: 空）
    /// PRIMARY_KEYはここに含まない
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<ConstraintDto>,

    /// リネーム元のテーブル名（オプショナル）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
}

/// 制約DTO（PRIMARY_KEY以外）
///
/// YAML内の制約定義を表現します。
/// PRIMARY_KEYは別フィールドで定義するため、ここには含みません。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum ConstraintDto {
    /// 外部キー制約
    FOREIGN_KEY {
        /// 対象カラム
        columns: Vec<String>,
        /// 参照先テーブル
        referenced_table: String,
        /// 参照先カラム
        referenced_columns: Vec<String>,
        /// 参照先レコード削除時のアクション
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_delete: Option<ReferentialAction>,
        /// 参照先レコード更新時のアクション
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_update: Option<ReferentialAction>,
    },
    /// ユニーク制約
    UNIQUE {
        /// 対象カラム
        columns: Vec<String>,
    },
    /// チェック制約
    CHECK {
        /// 対象カラム
        columns: Vec<String>,
        /// チェック式
        check_expression: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::ColumnType;

    // ======================================
    // Task 1.1: SchemaDto テスト
    // ======================================

    #[test]
    fn test_schema_dto_deserialize_minimal() {
        let yaml = r#"
version: "1.0"
tables: {}
"#;
        let dto: SchemaDto = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(dto.version, "1.0");
        assert!(!dto.enum_recreate_allowed);
        assert!(dto.enums.is_empty());
        assert!(dto.tables.is_empty());
    }

    #[test]
    fn test_schema_dto_deserialize_with_enum_recreate_allowed() {
        let yaml = r#"
version: "1.0"
enum_recreate_allowed: true
tables: {}
"#;
        let dto: SchemaDto = serde_saphyr::from_str(yaml).unwrap();

        assert!(dto.enum_recreate_allowed);
    }

    #[test]
    fn test_schema_dto_deserialize_with_enums() {
        let yaml = r#"
version: "1.0"
enums:
  status:
    name: status
    values: ["active", "inactive"]
tables: {}
"#;
        let dto: SchemaDto = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(dto.enums.len(), 1);
        assert!(dto.enums.contains_key("status"));
        let status_enum = dto.enums.get("status").unwrap();
        assert_eq!(status_enum.values, vec!["active", "inactive"]);
    }

    #[test]
    fn test_schema_dto_serialize_skips_empty_enums() {
        let dto = SchemaDto {
            version: "1.0".to_string(),
            enum_recreate_allowed: false,
            enums: HashMap::new(),
            tables: HashMap::new(),
        };

        let yaml = serde_saphyr::to_string(&dto).unwrap();

        // enums フィールドは出力されないはず
        assert!(!yaml.contains("enums:"));
        // enum_recreate_allowed もfalseなので出力されない
        assert!(!yaml.contains("enum_recreate_allowed"));
    }

    #[test]
    fn test_schema_dto_serialize_includes_enum_recreate_allowed_when_true() {
        let dto = SchemaDto {
            version: "1.0".to_string(),
            enum_recreate_allowed: true,
            enums: HashMap::new(),
            tables: HashMap::new(),
        };

        let yaml = serde_saphyr::to_string(&dto).unwrap();

        assert!(yaml.contains("enum_recreate_allowed: true"));
    }

    // ======================================
    // Task 1.2: TableDto テスト
    // ======================================

    #[test]
    fn test_table_dto_deserialize_minimal() {
        let yaml = r#"
columns:
  - name: id
    type:
      kind: INTEGER
    nullable: false
"#;
        let dto: TableDto = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(dto.columns.len(), 1);
        assert!(dto.primary_key.is_none());
        assert!(dto.indexes.is_empty());
        assert!(dto.constraints.is_empty());
    }

    #[test]
    fn test_table_dto_deserialize_with_primary_key() {
        let yaml = r#"
columns:
  - name: id
    type:
      kind: INTEGER
    nullable: false
primary_key:
  - id
"#;
        let dto: TableDto = serde_saphyr::from_str(yaml).unwrap();

        assert!(dto.primary_key.is_some());
        assert_eq!(dto.primary_key.unwrap(), vec!["id"]);
    }

    #[test]
    fn test_table_dto_deserialize_with_composite_primary_key() {
        let yaml = r#"
columns:
  - name: user_id
    type:
      kind: INTEGER
    nullable: false
  - name: role_id
    type:
      kind: INTEGER
    nullable: false
primary_key:
  - user_id
  - role_id
"#;
        let dto: TableDto = serde_saphyr::from_str(yaml).unwrap();

        let pk = dto.primary_key.unwrap();
        assert_eq!(pk.len(), 2);
        assert_eq!(pk[0], "user_id");
        assert_eq!(pk[1], "role_id");
    }

    #[test]
    fn test_table_dto_deserialize_with_indexes() {
        let yaml = r#"
columns:
  - name: email
    type:
      kind: VARCHAR
      length: 255
    nullable: false
indexes:
  - name: idx_email
    columns:
      - email
    unique: true
"#;
        let dto: TableDto = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(dto.indexes.len(), 1);
        assert_eq!(dto.indexes[0].name, "idx_email");
        assert!(dto.indexes[0].unique);
    }

    #[test]
    fn test_table_dto_serialize_skips_empty_fields() {
        let dto = TableDto {
            columns: vec![Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            )],
            primary_key: None,
            indexes: vec![],
            constraints: vec![],
            renamed_from: None,
        };

        let yaml = serde_saphyr::to_string(&dto).unwrap();

        // primary_key, indexes, constraints は出力されないはず
        assert!(!yaml.contains("primary_key:"));
        assert!(!yaml.contains("indexes:"));
        assert!(!yaml.contains("constraints:"));
    }

    #[test]
    fn test_table_dto_serialize_includes_primary_key_when_present() {
        let dto = TableDto {
            columns: vec![Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            )],
            primary_key: Some(vec!["id".to_string()]),
            indexes: vec![],
            constraints: vec![],
            renamed_from: None,
        };

        let yaml = serde_saphyr::to_string(&dto).unwrap();

        assert!(yaml.contains("primary_key:"));
    }

    // ======================================
    // Task 1.3: ConstraintDto テスト
    // ======================================

    #[test]
    fn test_constraint_dto_deserialize_foreign_key() {
        let yaml = r#"
type: FOREIGN_KEY
columns:
  - user_id
referenced_table: users
referenced_columns:
  - id
"#;
        let dto: ConstraintDto = serde_saphyr::from_str(yaml).unwrap();

        if let ConstraintDto::FOREIGN_KEY {
            columns,
            referenced_table,
            referenced_columns,
            ..
        } = dto
        {
            assert_eq!(columns, vec!["user_id"]);
            assert_eq!(referenced_table, "users");
            assert_eq!(referenced_columns, vec!["id"]);
        } else {
            panic!("Expected FOREIGN_KEY constraint");
        }
    }

    #[test]
    fn test_constraint_dto_deserialize_unique() {
        let yaml = r#"
type: UNIQUE
columns:
  - email
"#;
        let dto: ConstraintDto = serde_saphyr::from_str(yaml).unwrap();

        if let ConstraintDto::UNIQUE { columns } = dto {
            assert_eq!(columns, vec!["email"]);
        } else {
            panic!("Expected UNIQUE constraint");
        }
    }

    #[test]
    fn test_constraint_dto_deserialize_check() {
        let yaml = r#"
type: CHECK
columns:
  - age
check_expression: "age >= 0"
"#;
        let dto: ConstraintDto = serde_saphyr::from_str(yaml).unwrap();

        if let ConstraintDto::CHECK {
            columns,
            check_expression,
        } = dto
        {
            assert_eq!(columns, vec!["age"]);
            assert_eq!(check_expression, "age >= 0");
        } else {
            panic!("Expected CHECK constraint");
        }
    }

    #[test]
    fn test_constraint_dto_serialize_foreign_key() {
        let dto = ConstraintDto::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        };

        let yaml = serde_saphyr::to_string(&dto).unwrap();

        assert!(yaml.contains("type: FOREIGN_KEY"));
        assert!(yaml.contains("referenced_table: users"));
    }

    #[test]
    fn test_constraint_dto_serialize_unique() {
        let dto = ConstraintDto::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let yaml = serde_saphyr::to_string(&dto).unwrap();

        assert!(yaml.contains("type: UNIQUE"));
    }

    #[test]
    fn test_constraint_dto_serialize_check() {
        let dto = ConstraintDto::CHECK {
            columns: vec!["age".to_string()],
            check_expression: "age >= 0".to_string(),
        };

        let yaml = serde_saphyr::to_string(&dto).unwrap();

        assert!(yaml.contains("type: CHECK"));
        assert!(yaml.contains("check_expression:"));
    }

    // ======================================
    // 統合テスト: 新構文の完全なYAML
    // ======================================

    #[test]
    fn test_full_schema_dto_new_syntax() {
        let yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_users_email
        columns:
          - email
        unique: true
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: user_id
        type:
          kind: INTEGER
        nullable: false
      - name: title
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - user_id
        referenced_table: users
        referenced_columns:
          - id
"#;
        let dto: SchemaDto = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(dto.version, "1.0");
        assert_eq!(dto.tables.len(), 2);

        // users テーブル
        let users = dto.tables.get("users").unwrap();
        assert_eq!(users.columns.len(), 2);
        assert_eq!(users.primary_key.as_ref().unwrap(), &vec!["id"]);
        assert_eq!(users.indexes.len(), 1);
        assert!(users.constraints.is_empty());

        // posts テーブル
        let posts = dto.tables.get("posts").unwrap();
        assert_eq!(posts.columns.len(), 3);
        assert_eq!(posts.primary_key.as_ref().unwrap(), &vec!["id"]);
        assert!(posts.indexes.is_empty());
        assert_eq!(posts.constraints.len(), 1);

        if let ConstraintDto::FOREIGN_KEY {
            referenced_table, ..
        } = &posts.constraints[0]
        {
            assert_eq!(referenced_table, "users");
        } else {
            panic!("Expected FOREIGN_KEY constraint");
        }
    }

    #[test]
    fn test_round_trip_serialization() {
        // DTOを作成
        let original = SchemaDto {
            version: "1.0".to_string(),
            enum_recreate_allowed: false,
            enums: HashMap::new(),
            tables: {
                let mut tables = HashMap::new();
                tables.insert(
                    "users".to_string(),
                    TableDto {
                        columns: vec![Column::new(
                            "id".to_string(),
                            ColumnType::INTEGER { precision: None },
                            false,
                        )],
                        primary_key: Some(vec!["id".to_string()]),
                        indexes: vec![],
                        constraints: vec![],
                        renamed_from: None,
                    },
                );
                tables
            },
        };

        // シリアライズ
        let yaml = serde_saphyr::to_string(&original).unwrap();

        // デシリアライズ
        let parsed: SchemaDto = serde_saphyr::from_str(&yaml).unwrap();

        // 比較
        assert_eq!(parsed.version, original.version);
        assert_eq!(parsed.tables.len(), original.tables.len());
        let parsed_users = parsed.tables.get("users").unwrap();
        let original_users = original.tables.get("users").unwrap();
        assert_eq!(parsed_users.columns.len(), original_users.columns.len());
        assert_eq!(parsed_users.primary_key, original_users.primary_key);
    }
}
