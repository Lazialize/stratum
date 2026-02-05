use super::*;
use crate::adapters::database_introspector::{RawEnumInfo, RawViewInfo};
use crate::core::config::Dialect;
use crate::core::schema::{ColumnType, Constraint};
use std::collections::HashSet;

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
        auto_increment: None,
        enum_values: None,
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
        auto_increment: None,
        enum_values: None,
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
        auto_increment: None,
        enum_values: None,
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
        auto_increment: None,
        enum_values: None,
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
        auto_increment: None,
        enum_values: None,
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
            auto_increment: None,
            enum_values: None,
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
                auto_increment: None,
                enum_values: None,
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
                auto_increment: None,
                enum_values: None,
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
                auto_increment: None,
                enum_values: None,
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
                auto_increment: None,
                enum_values: None,
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
                auto_increment: None,
                enum_values: None,
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
                auto_increment: None,
                enum_values: None,
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
                auto_increment: None,
                enum_values: None,
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

// =========================================================================
// マテリアライズドビューのエラーテスト
// =========================================================================

#[test]
fn test_build_schema_with_materialized_view_returns_error() {
    let service = SchemaConversionService::new(Dialect::PostgreSQL);
    let raw_views = vec![RawViewInfo {
        name: "user_stats".to_string(),
        definition: "SELECT count(*) FROM users".to_string(),
        is_materialized: true,
    }];

    let result = service.build_schema_with_views(Vec::new(), Vec::new(), raw_views);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Materialized view"));
}

#[test]
fn test_build_schema_with_regular_view_succeeds() {
    let service = SchemaConversionService::new(Dialect::PostgreSQL);
    let raw_views = vec![RawViewInfo {
        name: "active_users".to_string(),
        definition: "SELECT * FROM users WHERE active = true".to_string(),
        is_materialized: false,
    }];

    let result = service.build_schema_with_views(Vec::new(), Vec::new(), raw_views);
    assert!(result.is_ok());
    let schema = result.unwrap();
    assert!(schema.views.contains_key("active_users"));
}
