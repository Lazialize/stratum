// スキーマ変換サービス
//
// DatabaseIntrospector から取得した生データを内部モデルに変換するサービス。
// TypeMappingService を使用して SQL 型文字列を ColumnType に変換します。

use crate::adapters::database_introspector::{
    RawColumnInfo, RawConstraintInfo, RawEnumInfo, RawIndexInfo,
};
use crate::adapters::type_mapping::{TypeMappingService, TypeMetadata};
use crate::core::config::Dialect;
use crate::core::schema::{Column, Constraint, EnumDefinition, Index, Schema, Table};
use anyhow::{Context, Result};
use std::collections::HashSet;

/// 生のテーブル情報
///
/// DatabaseIntrospector から取得した全テーブル情報を保持します。
#[derive(Debug, Clone)]
pub struct RawTableInfo {
    /// テーブル名
    pub name: String,
    /// カラム情報
    pub columns: Vec<RawColumnInfo>,
    /// インデックス情報
    pub indexes: Vec<RawIndexInfo>,
    /// 制約情報
    pub constraints: Vec<RawConstraintInfo>,
}

/// スキーマ変換サービス
///
/// 生のデータベース情報を内部スキーマモデルに変換します。
pub struct SchemaConversionService {
    type_mapping: TypeMappingService,
    /// 既知のENUM名（PostgreSQL用）
    enum_names: HashSet<String>,
}

impl SchemaConversionService {
    /// 新しい SchemaConversionService を作成
    pub fn new(dialect: Dialect) -> Self {
        Self {
            type_mapping: TypeMappingService::new(dialect),
            enum_names: HashSet::new(),
        }
    }

    /// ENUM名を設定
    ///
    /// PostgreSQL の ENUM 型を正しく認識するために、
    /// ENUM 名のセットを事前に設定します。
    pub fn with_enum_names(mut self, enum_names: HashSet<String>) -> Self {
        self.enum_names = enum_names;
        self
    }

    /// 生のカラム情報を内部モデルに変換
    ///
    /// TypeMappingService を使用して SQL 型文字列を ColumnType に変換します。
    pub fn convert_column(&self, raw: &RawColumnInfo) -> Result<Column> {
        let metadata = TypeMetadata {
            char_max_length: raw.char_max_length.map(|l| l as u32),
            numeric_precision: raw.numeric_precision.map(|p| p as u32),
            numeric_scale: raw.numeric_scale.map(|s| s as u32),
            udt_name: raw.udt_name.clone(),
            enum_names: if self.enum_names.is_empty() {
                None
            } else {
                Some(self.enum_names.clone())
            },
        };

        let column_type = self
            .type_mapping
            .from_sql_type(&raw.data_type, &metadata)
            .with_context(|| format!("Failed to parse column type for '{}'", raw.name))?;

        let mut column = Column::new(raw.name.clone(), column_type, raw.is_nullable);

        // PostgreSQL の SERIAL カラムは nextval('...') をデフォルト値として持つ
        // これを auto_increment: true として認識し、default_value は省略する
        if let Some(ref default) = raw.default_value {
            if default.contains("nextval(") {
                column.auto_increment = Some(true);
            } else {
                column.default_value = Some(default.clone());
            }
        }

        Ok(column)
    }

    /// 生のインデックス情報を内部モデルに変換
    pub fn convert_index(&self, raw: &RawIndexInfo) -> Result<Index> {
        Ok(Index {
            name: raw.name.clone(),
            columns: raw.columns.clone(),
            unique: raw.unique,
        })
    }

    /// 生の制約情報を内部モデルに変換
    pub fn convert_constraint(&self, raw: &RawConstraintInfo) -> Result<Constraint> {
        let constraint = match raw {
            RawConstraintInfo::PrimaryKey { columns } => Constraint::PRIMARY_KEY {
                columns: columns.clone(),
            },
            RawConstraintInfo::ForeignKey {
                columns,
                referenced_table,
                referenced_columns,
                on_delete,
            } => {
                let on_delete_action = on_delete.as_deref().and_then(|s| match s {
                    "CASCADE" => Some(crate::core::schema::ReferentialAction::Cascade),
                    "SET NULL" => Some(crate::core::schema::ReferentialAction::SetNull),
                    "SET DEFAULT" => Some(crate::core::schema::ReferentialAction::SetDefault),
                    "RESTRICT" => Some(crate::core::schema::ReferentialAction::Restrict),
                    // NO ACTION はデフォルトなので省略
                    _ => None,
                });
                Constraint::FOREIGN_KEY {
                    columns: columns.clone(),
                    referenced_table: referenced_table.clone(),
                    referenced_columns: referenced_columns.clone(),
                    on_delete: on_delete_action,
                    on_update: None,
                }
            }
            RawConstraintInfo::Unique { columns } => Constraint::UNIQUE {
                columns: columns.clone(),
            },
            RawConstraintInfo::Check {
                columns,
                expression,
            } => Constraint::CHECK {
                columns: columns.clone(),
                check_expression: expression.clone(),
            },
        };

        Ok(constraint)
    }

    /// 生のENUM情報を内部モデルに変換
    pub fn convert_enum(&self, raw: &RawEnumInfo) -> Result<EnumDefinition> {
        Ok(EnumDefinition {
            name: raw.name.clone(),
            values: raw.values.clone(),
        })
    }

    /// 生のテーブル情報を内部モデルに変換
    pub fn convert_table(&self, raw: &RawTableInfo) -> Result<Table> {
        let mut table = Table::new(raw.name.clone());

        // カラムを変換
        for raw_column in &raw.columns {
            let column = self
                .convert_column(raw_column)
                .with_context(|| format!("Failed to convert column in table '{}'", raw.name))?;
            table.add_column(column);
        }

        // インデックスを変換
        for raw_index in &raw.indexes {
            let index = self
                .convert_index(raw_index)
                .with_context(|| format!("Failed to convert index in table '{}'", raw.name))?;
            table.add_index(index);
        }

        // 制約を変換
        for raw_constraint in &raw.constraints {
            let constraint = self
                .convert_constraint(raw_constraint)
                .with_context(|| format!("Failed to convert constraint in table '{}'", raw.name))?;
            table.add_constraint(constraint);
        }

        Ok(table)
    }

    /// 複数のテーブル情報から Schema を構築
    pub fn build_schema(
        &self,
        raw_tables: Vec<RawTableInfo>,
        raw_enums: Vec<RawEnumInfo>,
    ) -> Result<Schema> {
        let mut schema = Schema::new("1.0".to_string());

        // ENUMを変換
        for raw_enum in raw_enums {
            let enum_def = self
                .convert_enum(&raw_enum)
                .with_context(|| format!("Failed to convert enum '{}'", raw_enum.name))?;
            schema.add_enum(enum_def);
        }

        // テーブルを変換
        for raw_table in raw_tables {
            let table = self
                .convert_table(&raw_table)
                .with_context(|| format!("Failed to convert table '{}'", raw_table.name))?;
            schema.add_table(table);
        }

        Ok(schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::ColumnType;

    // =========================================================================
    // SchemaConversionService 基本テスト
    // =========================================================================

    #[test]
    fn test_new_service() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        assert!(service.enum_names.is_empty());
    }

    #[test]
    fn test_with_enum_names() {
        let mut enum_names = HashSet::new();
        enum_names.insert("status".to_string());

        let service = SchemaConversionService::new(Dialect::PostgreSQL).with_enum_names(enum_names);

        assert!(service.enum_names.contains("status"));
    }

    // =========================================================================
    // convert_column テスト
    // =========================================================================

    #[test]
    fn test_convert_column_integer() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw = RawColumnInfo {
            name: "id".to_string(),
            data_type: "integer".to_string(),
            is_nullable: false,
            default_value: None,
            char_max_length: None,
            numeric_precision: Some(32),
            numeric_scale: None,
            udt_name: None,
        };

        let column = service.convert_column(&raw).unwrap();

        assert_eq!(column.name, "id");
        assert!(!column.nullable);
        assert!(matches!(column.column_type, ColumnType::INTEGER { .. }));
    }

    #[test]
    fn test_convert_column_varchar() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw = RawColumnInfo {
            name: "email".to_string(),
            data_type: "character varying".to_string(),
            is_nullable: true,
            default_value: Some("''".to_string()),
            char_max_length: Some(255),
            numeric_precision: None,
            numeric_scale: None,
            udt_name: None,
        };

        let column = service.convert_column(&raw).unwrap();

        assert_eq!(column.name, "email");
        assert!(column.nullable);
        assert!(matches!(
            column.column_type,
            ColumnType::VARCHAR { length: 255 }
        ));
        assert_eq!(column.default_value, Some("''".to_string()));
    }

    #[test]
    fn test_convert_column_enum() {
        let mut enum_names = HashSet::new();
        enum_names.insert("status".to_string());

        let service = SchemaConversionService::new(Dialect::PostgreSQL).with_enum_names(enum_names);

        let raw = RawColumnInfo {
            name: "status".to_string(),
            data_type: "USER-DEFINED".to_string(),
            is_nullable: false,
            default_value: None,
            char_max_length: None,
            numeric_precision: None,
            numeric_scale: None,
            udt_name: Some("status".to_string()),
        };

        let column = service.convert_column(&raw).unwrap();

        assert!(matches!(
            column.column_type,
            ColumnType::Enum { name } if name == "status"
        ));
    }

    #[test]
    fn test_convert_column_sqlite_integer() {
        let service = SchemaConversionService::new(Dialect::SQLite);
        let raw = RawColumnInfo {
            name: "id".to_string(),
            data_type: "INTEGER".to_string(),
            is_nullable: false,
            default_value: None,
            char_max_length: None,
            numeric_precision: None,
            numeric_scale: None,
            udt_name: None,
        };

        let column = service.convert_column(&raw).unwrap();

        assert_eq!(column.name, "id");
        assert!(matches!(column.column_type, ColumnType::INTEGER { .. }));
    }

    #[test]
    fn test_convert_column_mysql_varchar() {
        let service = SchemaConversionService::new(Dialect::MySQL);
        let raw = RawColumnInfo {
            name: "name".to_string(),
            data_type: "varchar".to_string(),
            is_nullable: true,
            default_value: None,
            char_max_length: Some(100),
            numeric_precision: None,
            numeric_scale: None,
            udt_name: None,
        };

        let column = service.convert_column(&raw).unwrap();

        assert_eq!(column.name, "name");
        assert!(matches!(
            column.column_type,
            ColumnType::VARCHAR { length: 100 }
        ));
    }

    // =========================================================================
    // convert_index テスト
    // =========================================================================

    #[test]
    fn test_convert_index_simple() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw = RawIndexInfo {
            name: "idx_email".to_string(),
            columns: vec!["email".to_string()],
            unique: true,
        };

        let index = service.convert_index(&raw).unwrap();

        assert_eq!(index.name, "idx_email");
        assert_eq!(index.columns, vec!["email"]);
        assert!(index.unique);
    }

    #[test]
    fn test_convert_index_composite() {
        let service = SchemaConversionService::new(Dialect::MySQL);
        let raw = RawIndexInfo {
            name: "idx_user_role".to_string(),
            columns: vec!["user_id".to_string(), "role_id".to_string()],
            unique: false,
        };

        let index = service.convert_index(&raw).unwrap();

        assert_eq!(index.columns.len(), 2);
        assert!(!index.unique);
    }

    // =========================================================================
    // convert_constraint テスト
    // =========================================================================

    #[test]
    fn test_convert_constraint_primary_key() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw = RawConstraintInfo::PrimaryKey {
            columns: vec!["id".to_string()],
        };

        let constraint = service.convert_constraint(&raw).unwrap();

        assert!(matches!(
            constraint,
            Constraint::PRIMARY_KEY { columns } if columns == vec!["id"]
        ));
    }

    #[test]
    fn test_convert_constraint_composite_primary_key() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw = RawConstraintInfo::PrimaryKey {
            columns: vec!["user_id".to_string(), "role_id".to_string()],
        };

        let constraint = service.convert_constraint(&raw).unwrap();

        if let Constraint::PRIMARY_KEY { columns } = constraint {
            assert_eq!(columns.len(), 2);
        } else {
            panic!("Expected PRIMARY_KEY");
        }
    }

    #[test]
    fn test_convert_constraint_foreign_key() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw = RawConstraintInfo::ForeignKey {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
        };

        let constraint = service.convert_constraint(&raw).unwrap();

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
    fn test_convert_constraint_unique() {
        let service = SchemaConversionService::new(Dialect::MySQL);
        let raw = RawConstraintInfo::Unique {
            columns: vec!["email".to_string()],
        };

        let constraint = service.convert_constraint(&raw).unwrap();

        assert!(matches!(
            constraint,
            Constraint::UNIQUE { columns } if columns == vec!["email"]
        ));
    }

    #[test]
    fn test_convert_constraint_check() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw = RawConstraintInfo::Check {
            columns: vec!["age".to_string()],
            expression: "age >= 0".to_string(),
        };

        let constraint = service.convert_constraint(&raw).unwrap();

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

    // =========================================================================
    // convert_enum テスト
    // =========================================================================

    #[test]
    fn test_convert_enum() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw = RawEnumInfo {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        };

        let enum_def = service.convert_enum(&raw).unwrap();

        assert_eq!(enum_def.name, "status");
        assert_eq!(enum_def.values, vec!["active", "inactive"]);
    }

    // =========================================================================
    // convert_table テスト
    // =========================================================================

    #[test]
    fn test_convert_table_minimal() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw = RawTableInfo {
            name: "users".to_string(),
            columns: vec![RawColumnInfo {
                name: "id".to_string(),
                data_type: "integer".to_string(),
                is_nullable: false,
                default_value: None,
                char_max_length: None,
                numeric_precision: None,
                numeric_scale: None,
                udt_name: None,
            }],
            indexes: vec![],
            constraints: vec![],
        };

        let table = service.convert_table(&raw).unwrap();

        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 1);
        assert!(table.indexes.is_empty());
        assert!(table.constraints.is_empty());
    }

    #[test]
    fn test_convert_table_with_all_elements() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw = RawTableInfo {
            name: "posts".to_string(),
            columns: vec![
                RawColumnInfo {
                    name: "id".to_string(),
                    data_type: "integer".to_string(),
                    is_nullable: false,
                    default_value: None,
                    char_max_length: None,
                    numeric_precision: None,
                    numeric_scale: None,
                    udt_name: None,
                },
                RawColumnInfo {
                    name: "title".to_string(),
                    data_type: "character varying".to_string(),
                    is_nullable: false,
                    default_value: None,
                    char_max_length: Some(200),
                    numeric_precision: None,
                    numeric_scale: None,
                    udt_name: None,
                },
                RawColumnInfo {
                    name: "user_id".to_string(),
                    data_type: "integer".to_string(),
                    is_nullable: false,
                    default_value: None,
                    char_max_length: None,
                    numeric_precision: None,
                    numeric_scale: None,
                    udt_name: None,
                },
            ],
            indexes: vec![RawIndexInfo {
                name: "idx_title".to_string(),
                columns: vec!["title".to_string()],
                unique: false,
            }],
            constraints: vec![
                RawConstraintInfo::PrimaryKey {
                    columns: vec!["id".to_string()],
                },
                RawConstraintInfo::ForeignKey {
                    columns: vec!["user_id".to_string()],
                    referenced_table: "users".to_string(),
                    referenced_columns: vec!["id".to_string()],
                    on_delete: None,
                },
            ],
        };

        let table = service.convert_table(&raw).unwrap();

        assert_eq!(table.name, "posts");
        assert_eq!(table.columns.len(), 3);
        assert_eq!(table.indexes.len(), 1);
        assert_eq!(table.constraints.len(), 2);
    }

    // =========================================================================
    // build_schema テスト
    // =========================================================================

    #[test]
    fn test_build_schema_empty() {
        let service = SchemaConversionService::new(Dialect::SQLite);

        let schema = service.build_schema(vec![], vec![]).unwrap();

        assert_eq!(schema.version, "1.0");
        assert!(schema.tables.is_empty());
        assert!(schema.enums.is_empty());
    }

    #[test]
    fn test_build_schema_with_tables() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw_tables = vec![
            RawTableInfo {
                name: "users".to_string(),
                columns: vec![RawColumnInfo {
                    name: "id".to_string(),
                    data_type: "integer".to_string(),
                    is_nullable: false,
                    default_value: None,
                    char_max_length: None,
                    numeric_precision: None,
                    numeric_scale: None,
                    udt_name: None,
                }],
                indexes: vec![],
                constraints: vec![],
            },
            RawTableInfo {
                name: "posts".to_string(),
                columns: vec![RawColumnInfo {
                    name: "id".to_string(),
                    data_type: "integer".to_string(),
                    is_nullable: false,
                    default_value: None,
                    char_max_length: None,
                    numeric_precision: None,
                    numeric_scale: None,
                    udt_name: None,
                }],
                indexes: vec![],
                constraints: vec![],
            },
        ];

        let schema = service.build_schema(raw_tables, vec![]).unwrap();

        assert_eq!(schema.tables.len(), 2);
        assert!(schema.get_table("users").is_some());
        assert!(schema.get_table("posts").is_some());
    }

    #[test]
    fn test_build_schema_with_enums() {
        let service = SchemaConversionService::new(Dialect::PostgreSQL);
        let raw_enums = vec![
            RawEnumInfo {
                name: "status".to_string(),
                values: vec!["active".to_string(), "inactive".to_string()],
            },
            RawEnumInfo {
                name: "role".to_string(),
                values: vec!["admin".to_string(), "user".to_string()],
            },
        ];

        let schema = service.build_schema(vec![], raw_enums).unwrap();

        assert_eq!(schema.enums.len(), 2);
        assert!(schema.enums.contains_key("status"));
        assert!(schema.enums.contains_key("role"));
    }

    #[test]
    fn test_build_schema_complex() {
        let mut enum_names = HashSet::new();
        enum_names.insert("status".to_string());

        let service = SchemaConversionService::new(Dialect::PostgreSQL).with_enum_names(enum_names);

        let raw_tables = vec![RawTableInfo {
            name: "users".to_string(),
            columns: vec![
                RawColumnInfo {
                    name: "id".to_string(),
                    data_type: "integer".to_string(),
                    is_nullable: false,
                    default_value: None,
                    char_max_length: None,
                    numeric_precision: None,
                    numeric_scale: None,
                    udt_name: None,
                },
                RawColumnInfo {
                    name: "status".to_string(),
                    data_type: "USER-DEFINED".to_string(),
                    is_nullable: false,
                    default_value: Some("'active'".to_string()),
                    char_max_length: None,
                    numeric_precision: None,
                    numeric_scale: None,
                    udt_name: Some("status".to_string()),
                },
            ],
            indexes: vec![RawIndexInfo {
                name: "idx_status".to_string(),
                columns: vec!["status".to_string()],
                unique: false,
            }],
            constraints: vec![RawConstraintInfo::PrimaryKey {
                columns: vec!["id".to_string()],
            }],
        }];

        let raw_enums = vec![RawEnumInfo {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        }];

        let schema = service.build_schema(raw_tables, raw_enums).unwrap();

        assert_eq!(schema.tables.len(), 1);
        assert_eq!(schema.enums.len(), 1);

        let users = schema.get_table("users").unwrap();
        assert_eq!(users.columns.len(), 2);
        assert_eq!(users.indexes.len(), 1);
        assert_eq!(users.constraints.len(), 1);

        // status カラムが ENUM 型になっていることを確認
        let status_col = users.columns.iter().find(|c| c.name == "status").unwrap();
        assert!(matches!(status_col.column_type, ColumnType::Enum { .. }));
    }
}
