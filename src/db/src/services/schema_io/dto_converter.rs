// DTO変換サービス
//
// Schema ↔ SchemaDto の双方向変換を一元管理するサービス。
// パース(DTO→Schema)とシリアライズ(Schema→DTO)の整合性を保証します。

use crate::core::schema::{Constraint, Schema, Table};
use crate::services::schema_io::dto::{ConstraintDto, SchemaDto, TableDto};
use anyhow::Result;
use std::collections::HashMap;

/// DTO変換サービス
///
/// Schema と SchemaDto の双方向変換を一元管理します。
/// ラウンドトリップ整合性を保証するため、変換ロジックを単一箇所に集約しています。
#[derive(Debug, Clone)]
pub struct DtoConverterService;

impl DtoConverterService {
    /// 新しいDtoConverterServiceを作成
    pub fn new() -> Self {
        Self
    }

    /// Schema → SchemaDto 変換
    ///
    /// 内部スキーマモデルをDTO形式に変換します。
    /// PRIMARY_KEY制約は primary_key フィールドに抽出されます。
    pub fn schema_to_dto(&self, schema: &Schema) -> SchemaDto {
        let mut tables = HashMap::new();

        for (table_name, table) in &schema.tables {
            let table_dto = self.table_to_dto(table);
            tables.insert(table_name.clone(), table_dto);
        }

        SchemaDto {
            version: schema.version.clone(),
            enum_recreate_allowed: schema.enum_recreate_allowed,
            enums: schema.enums.clone(),
            tables,
        }
    }

    /// SchemaDto → Schema 変換
    ///
    /// DTO形式を内部スキーマモデルに変換します。
    /// primary_key フィールドは Constraint::PRIMARY_KEY に変換されます。
    pub fn dto_to_schema(&self, dto: &SchemaDto) -> Result<Schema> {
        let mut schema = Schema::new(dto.version.clone());
        schema.enum_recreate_allowed = dto.enum_recreate_allowed;

        // ENUM定義をコピー
        for (name, enum_def) in &dto.enums {
            schema.enums.insert(name.clone(), enum_def.clone());
        }

        // テーブルを変換
        for (table_name, table_dto) in &dto.tables {
            let table = self.dto_to_table(table_name, table_dto)?;
            schema.add_table(table);
        }

        Ok(schema)
    }

    /// Table → TableDto 変換
    ///
    /// PRIMARY_KEY制約を primary_key フィールドに抽出し、
    /// それ以外の制約を constraints フィールドに変換します。
    pub fn table_to_dto(&self, table: &Table) -> TableDto {
        TableDto {
            columns: table.columns.clone(),
            primary_key: self.extract_primary_key(&table.constraints),
            indexes: table.indexes.clone(),
            constraints: self.convert_constraints_to_dto(&table.constraints),
            renamed_from: table.renamed_from.clone(),
        }
    }

    /// TableDto → Table 変換
    ///
    /// テーブル名をキーから取得し、primary_key を Constraint::PRIMARY_KEY に変換します。
    pub fn dto_to_table(&self, name: &str, dto: &TableDto) -> Result<Table> {
        let mut table = Table::new(name.to_string());

        // カラムをコピー
        table.columns = dto.columns.clone();

        // インデックスをコピー
        table.indexes = dto.indexes.clone();

        // primary_key → Constraint::PRIMARY_KEY 変換
        if let Some(pk_columns) = &dto.primary_key {
            table.add_constraint(Constraint::PRIMARY_KEY {
                columns: pk_columns.clone(),
            });
        }

        // ConstraintDto → Constraint 変換
        for constraint_dto in &dto.constraints {
            let constraint = self.dto_to_constraint(constraint_dto);
            table.add_constraint(constraint);
        }

        // renamed_from をコピー
        table.renamed_from = dto.renamed_from.clone();

        Ok(table)
    }

    /// Constraint → ConstraintDto 変換
    ///
    /// PRIMARY_KEY は None を返します（primary_key フィールドで処理するため）。
    pub fn constraint_to_dto(&self, constraint: &Constraint) -> Option<ConstraintDto> {
        match constraint {
            Constraint::PRIMARY_KEY { .. } => None,
            Constraint::FOREIGN_KEY {
                columns,
                referenced_table,
                referenced_columns,
                on_delete,
                on_update,
            } => Some(ConstraintDto::FOREIGN_KEY {
                columns: columns.clone(),
                referenced_table: referenced_table.clone(),
                referenced_columns: referenced_columns.clone(),
                on_delete: on_delete.clone(),
                on_update: on_update.clone(),
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
        }
    }

    /// ConstraintDto → Constraint 変換
    pub fn dto_to_constraint(&self, dto: &ConstraintDto) -> Constraint {
        match dto {
            ConstraintDto::FOREIGN_KEY {
                columns,
                referenced_table,
                referenced_columns,
                on_delete,
                on_update,
            } => Constraint::FOREIGN_KEY {
                columns: columns.clone(),
                referenced_table: referenced_table.clone(),
                referenced_columns: referenced_columns.clone(),
                on_delete: on_delete.clone(),
                on_update: on_update.clone(),
            },
            ConstraintDto::UNIQUE { columns } => Constraint::UNIQUE {
                columns: columns.clone(),
            },
            ConstraintDto::CHECK {
                columns,
                check_expression,
            } => Constraint::CHECK {
                columns: columns.clone(),
                check_expression: check_expression.clone(),
            },
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
            .filter_map(|c| self.constraint_to_dto(c))
            .collect()
    }
}

impl Default for DtoConverterService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{Column, ColumnType, EnumDefinition, Index};

    // ======================================
    // Task 2.1: DtoConverterService 基本テスト
    // ======================================

    #[test]
    fn test_new_service() {
        let service = DtoConverterService::new();
        assert!(format!("{:?}", service).contains("DtoConverterService"));
    }

    #[test]
    fn test_default_service() {
        let service = DtoConverterService;
        assert!(format!("{:?}", service).contains("DtoConverterService"));
    }

    // ======================================
    // Schema ↔ SchemaDto 変換テスト
    // ======================================

    #[test]
    fn test_schema_to_dto_minimal() {
        let schema = Schema::new("1.0".to_string());
        let service = DtoConverterService::new();

        let dto = service.schema_to_dto(&schema);

        assert_eq!(dto.version, "1.0");
        assert!(!dto.enum_recreate_allowed);
        assert!(dto.enums.is_empty());
        assert!(dto.tables.is_empty());
    }

    #[test]
    fn test_schema_to_dto_with_enum_recreate_allowed() {
        let mut schema = Schema::new("1.0".to_string());
        schema.enum_recreate_allowed = true;
        let service = DtoConverterService::new();

        let dto = service.schema_to_dto(&schema);

        assert!(dto.enum_recreate_allowed);
    }

    #[test]
    fn test_schema_to_dto_with_enums() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });
        let service = DtoConverterService::new();

        let dto = service.schema_to_dto(&schema);

        assert_eq!(dto.enums.len(), 1);
        assert!(dto.enums.contains_key("status"));
    }

    #[test]
    fn test_dto_to_schema_minimal() {
        let dto = SchemaDto {
            version: "1.0".to_string(),
            enum_recreate_allowed: false,
            enums: HashMap::new(),
            tables: HashMap::new(),
        };
        let service = DtoConverterService::new();

        let schema = service.dto_to_schema(&dto).unwrap();

        assert_eq!(schema.version, "1.0");
        assert!(!schema.enum_recreate_allowed);
        assert!(schema.enums.is_empty());
        assert!(schema.tables.is_empty());
    }

    #[test]
    fn test_dto_to_schema_with_enum_recreate_allowed() {
        let dto = SchemaDto {
            version: "1.0".to_string(),
            enum_recreate_allowed: true,
            enums: HashMap::new(),
            tables: HashMap::new(),
        };
        let service = DtoConverterService::new();

        let schema = service.dto_to_schema(&dto).unwrap();

        assert!(schema.enum_recreate_allowed);
    }

    // ======================================
    // Table ↔ TableDto 変換テスト
    // ======================================

    #[test]
    fn test_table_to_dto_minimal() {
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        let service = DtoConverterService::new();

        let dto = service.table_to_dto(&table);

        assert_eq!(dto.columns.len(), 1);
        assert!(dto.primary_key.is_none());
        assert!(dto.indexes.is_empty());
        assert!(dto.constraints.is_empty());
    }

    #[test]
    fn test_table_to_dto_with_primary_key() {
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        let service = DtoConverterService::new();

        let dto = service.table_to_dto(&table);

        assert!(dto.primary_key.is_some());
        assert_eq!(dto.primary_key.unwrap(), vec!["id"]);
        // PRIMARY_KEY は constraints に含まれない
        assert!(dto.constraints.is_empty());
    }

    #[test]
    fn test_table_to_dto_with_composite_primary_key() {
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
        let service = DtoConverterService::new();

        let dto = service.table_to_dto(&table);

        let pk = dto.primary_key.unwrap();
        assert_eq!(pk.len(), 2);
        assert_eq!(pk[0], "user_id");
        assert_eq!(pk[1], "role_id");
    }

    #[test]
    fn test_table_to_dto_with_indexes() {
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
        let service = DtoConverterService::new();

        let dto = service.table_to_dto(&table);

        assert_eq!(dto.indexes.len(), 1);
        assert_eq!(dto.indexes[0].name, "idx_email");
        assert!(dto.indexes[0].unique);
    }

    #[test]
    fn test_dto_to_table_minimal() {
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
        let service = DtoConverterService::new();

        let table = service.dto_to_table("users", &dto).unwrap();

        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 1);
        assert!(table.constraints.is_empty());
    }

    #[test]
    fn test_dto_to_table_with_primary_key() {
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
        let service = DtoConverterService::new();

        let table = service.dto_to_table("users", &dto).unwrap();

        let pk_columns = table.get_primary_key_columns();
        assert!(pk_columns.is_some());
        assert_eq!(pk_columns.unwrap(), vec!["id"]);
    }

    // ======================================
    // Constraint ↔ ConstraintDto 変換テスト
    // ======================================

    #[test]
    fn test_constraint_to_dto_primary_key_returns_none() {
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };
        let service = DtoConverterService::new();

        let dto = service.constraint_to_dto(&constraint);

        assert!(dto.is_none());
    }

    #[test]
    fn test_constraint_to_dto_foreign_key() {
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        };
        let service = DtoConverterService::new();

        let dto = service.constraint_to_dto(&constraint).unwrap();

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
            panic!("Expected FOREIGN_KEY");
        }
    }

    #[test]
    fn test_constraint_to_dto_unique() {
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };
        let service = DtoConverterService::new();

        let dto = service.constraint_to_dto(&constraint).unwrap();

        if let ConstraintDto::UNIQUE { columns } = dto {
            assert_eq!(columns, vec!["email"]);
        } else {
            panic!("Expected UNIQUE");
        }
    }

    #[test]
    fn test_constraint_to_dto_check() {
        let constraint = Constraint::CHECK {
            columns: vec!["age".to_string()],
            check_expression: "age >= 0".to_string(),
        };
        let service = DtoConverterService::new();

        let dto = service.constraint_to_dto(&constraint).unwrap();

        if let ConstraintDto::CHECK {
            columns,
            check_expression,
        } = dto
        {
            assert_eq!(columns, vec!["age"]);
            assert_eq!(check_expression, "age >= 0");
        } else {
            panic!("Expected CHECK");
        }
    }

    #[test]
    fn test_dto_to_constraint_foreign_key() {
        let dto = ConstraintDto::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        };
        let service = DtoConverterService::new();

        let constraint = service.dto_to_constraint(&dto);

        if let Constraint::FOREIGN_KEY {
            columns,
            referenced_table,
            referenced_columns,
            ..
        } = constraint
        {
            assert_eq!(columns, vec!["user_id"]);
            assert_eq!(referenced_table, "users");
            assert_eq!(referenced_columns, vec!["id"]);
        } else {
            panic!("Expected FOREIGN_KEY");
        }
    }

    #[test]
    fn test_dto_to_constraint_unique() {
        let dto = ConstraintDto::UNIQUE {
            columns: vec!["email".to_string()],
        };
        let service = DtoConverterService::new();

        let constraint = service.dto_to_constraint(&dto);

        if let Constraint::UNIQUE { columns } = constraint {
            assert_eq!(columns, vec!["email"]);
        } else {
            panic!("Expected UNIQUE");
        }
    }

    #[test]
    fn test_dto_to_constraint_check() {
        let dto = ConstraintDto::CHECK {
            columns: vec!["age".to_string()],
            check_expression: "age >= 0".to_string(),
        };
        let service = DtoConverterService::new();

        let constraint = service.dto_to_constraint(&dto);

        if let Constraint::CHECK {
            columns,
            check_expression,
        } = constraint
        {
            assert_eq!(columns, vec!["age"]);
            assert_eq!(check_expression, "age >= 0");
        } else {
            panic!("Expected CHECK");
        }
    }

    // ======================================
    // ラウンドトリップテスト
    // ======================================

    #[test]
    fn test_schema_round_trip_minimal() {
        let original = Schema::new("1.0".to_string());
        let service = DtoConverterService::new();

        let dto = service.schema_to_dto(&original);
        let restored = service.dto_to_schema(&dto).unwrap();

        assert_eq!(original.version, restored.version);
        assert_eq!(
            original.enum_recreate_allowed,
            restored.enum_recreate_allowed
        );
        assert_eq!(original.tables.len(), restored.tables.len());
    }

    #[test]
    fn test_schema_round_trip_with_table() {
        let mut original = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        original.add_table(table);
        let service = DtoConverterService::new();

        let dto = service.schema_to_dto(&original);
        let restored = service.dto_to_schema(&dto).unwrap();

        assert_eq!(original.tables.len(), restored.tables.len());
        let original_table = original.get_table("users").unwrap();
        let restored_table = restored.get_table("users").unwrap();
        assert_eq!(original_table.columns.len(), restored_table.columns.len());
        assert_eq!(
            original_table.get_primary_key_columns(),
            restored_table.get_primary_key_columns()
        );
    }

    #[test]
    fn test_schema_round_trip_complex() {
        let mut original = Schema::new("1.0".to_string());
        original.enum_recreate_allowed = true;
        original.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        // users テーブル
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
            "idx_email".to_string(),
            vec!["email".to_string()],
            true,
        ));
        original.add_table(users);

        // posts テーブル
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
        posts.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        posts.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });
        original.add_table(posts);

        let service = DtoConverterService::new();

        let dto = service.schema_to_dto(&original);
        let restored = service.dto_to_schema(&dto).unwrap();

        // 基本プロパティ
        assert_eq!(original.version, restored.version);
        assert_eq!(
            original.enum_recreate_allowed,
            restored.enum_recreate_allowed
        );
        assert_eq!(original.enums.len(), restored.enums.len());
        assert_eq!(original.tables.len(), restored.tables.len());

        // users テーブル
        let orig_users = original.get_table("users").unwrap();
        let rest_users = restored.get_table("users").unwrap();
        assert_eq!(orig_users.columns.len(), rest_users.columns.len());
        assert_eq!(orig_users.indexes.len(), rest_users.indexes.len());
        assert_eq!(orig_users.constraints.len(), rest_users.constraints.len());

        // posts テーブル
        let orig_posts = original.get_table("posts").unwrap();
        let rest_posts = restored.get_table("posts").unwrap();
        assert_eq!(orig_posts.columns.len(), rest_posts.columns.len());
        assert_eq!(orig_posts.constraints.len(), rest_posts.constraints.len());
    }

    #[test]
    fn test_table_round_trip_with_all_constraint_types() {
        let mut original = Table::new("test_table".to_string());
        original.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        original.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        original.add_column(Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            true,
        ));
        original.add_column(Column::new(
            "user_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        original.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        original.add_constraint(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        original.add_constraint(Constraint::CHECK {
            columns: vec!["age".to_string()],
            check_expression: "age >= 0".to_string(),
        });
        original.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });
        original.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            true,
        ));

        let service = DtoConverterService::new();

        let dto = service.table_to_dto(&original);
        let restored = service.dto_to_table("test_table", &dto).unwrap();

        assert_eq!(original.name, restored.name);
        assert_eq!(original.columns.len(), restored.columns.len());
        assert_eq!(original.indexes.len(), restored.indexes.len());
        assert_eq!(original.constraints.len(), restored.constraints.len());
        assert_eq!(
            original.get_primary_key_columns(),
            restored.get_primary_key_columns()
        );
    }
}
