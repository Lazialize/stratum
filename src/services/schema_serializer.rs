// スキーマシリアライザーサービス
//
// 内部スキーマモデルをYAML文字列に変換するサービス。
// 新構文形式（nameフィールドなし、primary_key独立フィールド）で出力します。

use crate::core::schema::{Constraint, Schema, Table};
use crate::services::dto::{ConstraintDto, SchemaDto, TableDto};
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// スキーマシリアライザーサービス
///
/// 内部スキーマモデルをYAML形式にシリアライズします。
#[derive(Debug, Clone)]
pub struct SchemaSerializerService;

impl SchemaSerializerService {
    /// 新しいSchemaSerializerServiceを作成
    pub fn new() -> Self {
        Self
    }

    /// SchemaをYAML文字列にシリアライズ
    ///
    /// # Arguments
    ///
    /// * `schema` - シリアライズするスキーマ
    ///
    /// # Returns
    ///
    /// 新構文形式のYAML文字列
    pub fn serialize_to_string(&self, schema: &Schema) -> Result<String> {
        let dto = self.convert_schema_to_dto(schema);
        let yaml = serde_saphyr::to_string(&dto)?;
        Ok(yaml)
    }

    /// SchemaをYAMLファイルに出力
    ///
    /// # Arguments
    ///
    /// * `schema` - シリアライズするスキーマ
    /// * `file_path` - 出力先ファイルパス
    pub fn serialize_to_file(&self, schema: &Schema, file_path: &Path) -> Result<()> {
        let yaml = self.serialize_to_string(schema)?;
        fs::write(file_path, yaml)?;
        Ok(())
    }

    /// Schema → SchemaDto 変換
    fn convert_schema_to_dto(&self, schema: &Schema) -> SchemaDto {
        let mut tables = HashMap::new();

        for (table_name, table) in &schema.tables {
            let table_dto = self.convert_table_to_dto(table);
            tables.insert(table_name.clone(), table_dto);
        }

        SchemaDto {
            version: schema.version.clone(),
            enum_recreate_allowed: schema.enum_recreate_allowed,
            enums: schema.enums.clone(),
            tables,
        }
    }

    /// Table → TableDto 変換
    ///
    /// PRIMARY_KEY制約をprimary_keyフィールドに抽出し、
    /// それ以外の制約をconstraintsフィールドに変換します。
    fn convert_table_to_dto(&self, table: &Table) -> TableDto {
        TableDto {
            columns: table.columns.clone(),
            primary_key: self.extract_primary_key(&table.constraints),
            indexes: table.indexes.clone(),
            constraints: self.convert_constraints_to_dto(&table.constraints),
        }
    }

    /// PRIMARY_KEY制約を抽出
    fn extract_primary_key(&self, constraints: &[Constraint]) -> Option<Vec<String>> {
        constraints.iter().find_map(|c| {
            if let Constraint::PRIMARY_KEY { columns } = c {
                Some(columns.clone())
            } else {
                None
            }
        })
    }

    /// PRIMARY_KEY以外の制約をDTOに変換
    fn convert_constraints_to_dto(&self, constraints: &[Constraint]) -> Vec<ConstraintDto> {
        constraints
            .iter()
            .filter_map(|c| match c {
                Constraint::PRIMARY_KEY { .. } => None, // 除外
                Constraint::FOREIGN_KEY {
                    columns,
                    referenced_table,
                    referenced_columns,
                } => Some(ConstraintDto::FOREIGN_KEY {
                    columns: columns.clone(),
                    referenced_table: referenced_table.clone(),
                    referenced_columns: referenced_columns.clone(),
                }),
                Constraint::UNIQUE { columns } => Some(ConstraintDto::UNIQUE {
                    columns: columns.clone(),
                }),
                Constraint::CHECK {
                    columns,
                    check_expression,
                } => Some(ConstraintDto::CHECK {
                    columns: columns.clone(),
                    check_expression: check_expression.clone(),
                }),
            })
            .collect()
    }
}

impl Default for SchemaSerializerService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{Column, ColumnType, EnumDefinition, Index};
    use tempfile::TempDir;

    // ======================================
    // Task 3.1: SchemaSerializerService 基本テスト
    // ======================================

    #[test]
    fn test_new_service() {
        let service = SchemaSerializerService::new();
        assert!(format!("{:?}", service).contains("SchemaSerializerService"));
    }

    #[test]
    fn test_serialize_minimal_schema() {
        let schema = Schema::new("1.0".to_string());
        let service = SchemaSerializerService::new();

        let yaml = service.serialize_to_string(&schema).unwrap();

        assert!(yaml.contains("version: '1.0'") || yaml.contains("version: \"1.0\""));
        // 空のテーブルマップが出力される
        assert!(yaml.contains("tables:"));
    }

    #[test]
    fn test_serialize_to_file() {
        let schema = Schema::new("1.0".to_string());
        let service = SchemaSerializerService::new();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("schema.yaml");

        service.serialize_to_file(&schema, &file_path).unwrap();

        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("version:"));
    }

    // ======================================
    // Task 3.2: 内部モデル → DTO 変換テスト
    // ======================================

    #[test]
    fn test_serialize_table_name_as_key() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema.add_table(table);

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        // テーブル名がキーとして出力される
        assert!(yaml.contains("users:"));
        // nameフィールドは出力されない
        assert!(!yaml.contains("name: users"));
    }

    #[test]
    fn test_serialize_enum_recreate_allowed() {
        let mut schema = Schema::new("1.0".to_string());
        schema.enum_recreate_allowed = true;

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        assert!(yaml.contains("enum_recreate_allowed: true"));
    }

    #[test]
    fn test_serialize_enum_recreate_allowed_false_not_output() {
        let schema = Schema::new("1.0".to_string());

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        // falseの場合は出力しない
        assert!(!yaml.contains("enum_recreate_allowed"));
    }

    #[test]
    fn test_serialize_with_enums() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        assert!(yaml.contains("enums:"));
        assert!(yaml.contains("status:"));
    }

    // ======================================
    // Task 3.3: PRIMARY_KEY → primary_key 変換テスト
    // ======================================

    #[test]
    fn test_serialize_primary_key_as_field() {
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

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        // primary_keyフィールドとして出力される
        assert!(yaml.contains("primary_key:"));
        assert!(yaml.contains("- id"));
        // constraints内にPRIMARY_KEYは含まれない
        assert!(!yaml.contains("type: PRIMARY_KEY"));
    }

    #[test]
    fn test_serialize_composite_primary_key() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("user_roles".to_string());
        table.add_column(Column::new(
            "user_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "role_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["user_id".to_string(), "role_id".to_string()],
        });
        schema.add_table(table);

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        assert!(yaml.contains("primary_key:"));
        assert!(yaml.contains("- user_id"));
        assert!(yaml.contains("- role_id"));
    }

    #[test]
    fn test_serialize_foreign_key_constraint() {
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
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        // FOREIGN_KEYはconstraintsに出力される
        assert!(yaml.contains("constraints:"));
        assert!(yaml.contains("type: FOREIGN_KEY"));
        assert!(yaml.contains("referenced_table: users"));
    }

    #[test]
    fn test_serialize_unique_constraint() {
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
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        table.add_constraint(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        schema.add_table(table);

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        assert!(yaml.contains("type: UNIQUE"));
    }

    #[test]
    fn test_serialize_check_constraint() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::CHECK {
            columns: vec!["age".to_string()],
            check_expression: "age >= 0".to_string(),
        });
        schema.add_table(table);

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        assert!(yaml.contains("type: CHECK"));
        assert!(yaml.contains("check_expression:"));
    }

    #[test]
    fn test_serialize_empty_indexes_not_output() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        // インデックスなし
        schema.add_table(table);

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        // 空のindexesは出力されない
        assert!(!yaml.contains("indexes:"));
    }

    #[test]
    fn test_serialize_empty_constraints_not_output() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        // PRIMARY_KEYのみ（constraintsには出力されない）
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        // PRIMARY_KEY以外の制約がないのでconstraintsは出力されない
        assert!(!yaml.contains("constraints:"));
    }

    #[test]
    fn test_serialize_with_indexes() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            true,
        ));
        schema.add_table(table);

        let service = SchemaSerializerService::new();
        let yaml = service.serialize_to_string(&schema).unwrap();

        assert!(yaml.contains("indexes:"));
        assert!(yaml.contains("idx_email"));
        assert!(yaml.contains("unique: true"));
    }

    // ======================================
    // 往復テスト（Round-trip）
    // ======================================

    #[test]
    fn test_round_trip_serialize_parse() {
        use crate::services::schema_parser::SchemaParserService;

        // 内部モデルを作成
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
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            true,
        ));
        schema.add_table(table);

        // シリアライズ
        let serializer = SchemaSerializerService::new();
        let yaml = serializer.serialize_to_string(&schema).unwrap();

        // ファイルに書き出し
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("schema.yaml");
        fs::write(&file_path, &yaml).unwrap();

        // パース
        let parser = SchemaParserService::new();
        let parsed = parser.parse_schema_file(&file_path).unwrap();

        // 比較
        assert_eq!(parsed.version, schema.version);
        assert_eq!(parsed.tables.len(), schema.tables.len());

        let parsed_users = parsed.get_table("users").unwrap();
        let original_users = schema.get_table("users").unwrap();

        assert_eq!(parsed_users.columns.len(), original_users.columns.len());
        assert_eq!(
            parsed_users.get_primary_key_columns(),
            original_users.get_primary_key_columns()
        );
        assert_eq!(parsed_users.indexes.len(), original_users.indexes.len());
    }

    #[test]
    fn test_full_schema_round_trip() {
        use crate::services::schema_parser::SchemaParserService;

        // 複雑なスキーマを作成
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        // usersテーブル
        let mut users = Table::new("users".to_string());
        users.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        users.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        users.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        users.add_constraint(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        users.add_index(Index::new(
            "idx_users_email".to_string(),
            vec!["email".to_string()],
            true,
        ));
        schema.add_table(users);

        // postsテーブル
        let mut posts = Table::new("posts".to_string());
        posts.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts.add_column(Column::new(
            "user_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts.add_column(Column::new(
            "title".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        posts.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        posts.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
        });
        schema.add_table(posts);

        // シリアライズ
        let serializer = SchemaSerializerService::new();
        let yaml = serializer.serialize_to_string(&schema).unwrap();

        // ファイルに書き出し
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("schema.yaml");
        fs::write(&file_path, &yaml).unwrap();

        // パース
        let parser = SchemaParserService::new();
        let parsed = parser.parse_schema_file(&file_path).unwrap();

        // 比較
        assert_eq!(parsed.version, schema.version);
        assert_eq!(parsed.tables.len(), 2);
        assert_eq!(parsed.enums.len(), 1);

        // usersテーブル
        let parsed_users = parsed.get_table("users").unwrap();
        assert_eq!(parsed_users.columns.len(), 2);
        assert_eq!(parsed_users.get_primary_key_columns().unwrap(), vec!["id"]);
        // UNIQUE制約 + PRIMARY_KEY制約
        assert_eq!(parsed_users.constraints.len(), 2);

        // postsテーブル
        let parsed_posts = parsed.get_table("posts").unwrap();
        assert_eq!(parsed_posts.columns.len(), 3);
        // FOREIGN_KEY制約 + PRIMARY_KEY制約
        assert_eq!(parsed_posts.constraints.len(), 2);
    }
}
