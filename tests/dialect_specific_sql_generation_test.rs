// DialectSpecific型のSQL生成テスト

use strata::adapters::sql_generator::mysql::MysqlSqlGenerator;
use strata::adapters::sql_generator::postgres::PostgresSqlGenerator;
use strata::adapters::sql_generator::sqlite::SqliteSqlGenerator;
use strata::adapters::sql_generator::SqlGenerator;
use strata::core::schema::{Column, ColumnType, Table};

#[cfg(test)]
mod postgres_dialect_specific_tests {
    use super::*;

    /// PostgreSQL SERIAL型のSQL生成
    #[test]
    fn test_postgres_generate_serial_type() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("users".to_string());
        table.add_column(Column {
            name: "id".to_string(),
            column_type: ColumnType::DialectSpecific {
                kind: "SERIAL".to_string(),
                params: serde_json::json!(null),
            },
            nullable: false,
            default_value: None,
            auto_increment: None,
        });

        let sql = generator.generate_create_table(&table);

        // SERIALがそのまま出力されているか確認
        assert!(
            sql.contains("id SERIAL"),
            "Expected 'id SERIAL' in SQL: {}",
            sql
        );
        assert!(
            sql.contains("NOT NULL"),
            "Expected 'NOT NULL' in SQL: {}",
            sql
        );
    }

    /// PostgreSQL VARBIT(n)型のSQL生成（パラメータあり）
    #[test]
    fn test_postgres_generate_varbit_with_length() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("flags_table".to_string());
        table.add_column(Column {
            name: "flags".to_string(),
            column_type: ColumnType::DialectSpecific {
                kind: "VARBIT".to_string(),
                params: serde_json::json!({ "length": 16 }),
            },
            nullable: true,
            default_value: None,
            auto_increment: None,
        });

        let sql = generator.generate_create_table(&table);

        // VARBIT(16)の形式で出力されているか確認
        assert!(
            sql.contains("VARBIT(16)"),
            "Expected 'VARBIT(16)' in SQL: {}",
            sql
        );
    }

    /// PostgreSQL ARRAY型のSQL生成
    #[test]
    fn test_postgres_generate_array_type() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("tags_table".to_string());
        table.add_column(Column {
            name: "tags".to_string(),
            column_type: ColumnType::DialectSpecific {
                kind: "TEXT".to_string(),
                params: serde_json::json!({ "array": true }),
            },
            nullable: true,
            default_value: None,
            auto_increment: None,
        });

        let sql = generator.generate_create_table(&table);

        // TEXT[]の形式で出力されているか確認（配列パラメータの処理）
        assert!(sql.contains("TEXT[]"), "Expected 'TEXT[]' in SQL: {}", sql);
    }
}

#[cfg(test)]
mod mysql_dialect_specific_tests {
    use super::*;

    /// MySQL ENUM型のSQL生成
    #[test]
    fn test_mysql_generate_enum_type() {
        let generator = MysqlSqlGenerator::new();

        let mut table = Table::new("orders".to_string());
        table.add_column(Column {
            name: "status".to_string(),
            column_type: ColumnType::DialectSpecific {
                kind: "ENUM".to_string(),
                params: serde_json::json!({ "values": ["pending", "processing", "completed"] }),
            },
            nullable: false,
            default_value: None,
            auto_increment: None,
        });

        let sql = generator.generate_create_table(&table);

        // ENUM('pending', 'processing', 'completed')の形式で出力されているか確認
        assert!(sql.contains("ENUM("), "Expected 'ENUM(' in SQL: {}", sql);
        assert!(
            sql.contains("'pending'"),
            "Expected 'pending' in SQL: {}",
            sql
        );
        assert!(
            sql.contains("'processing'"),
            "Expected 'processing' in SQL: {}",
            sql
        );
        assert!(
            sql.contains("'completed'"),
            "Expected 'completed' in SQL: {}",
            sql
        );
    }

    /// MySQL TINYINT型のSQL生成
    #[test]
    fn test_mysql_generate_tinyint_type() {
        let generator = MysqlSqlGenerator::new();

        let mut table = Table::new("settings".to_string());
        table.add_column(Column {
            name: "flag".to_string(),
            column_type: ColumnType::DialectSpecific {
                kind: "TINYINT".to_string(),
                params: serde_json::json!(null),
            },
            nullable: false,
            default_value: None,
            auto_increment: None,
        });

        let sql = generator.generate_create_table(&table);

        // TINYINTがそのまま出力されているか確認
        assert!(
            sql.contains("flag TINYINT"),
            "Expected 'flag TINYINT' in SQL: {}",
            sql
        );
    }

    /// MySQL SET型のSQL生成
    #[test]
    fn test_mysql_generate_set_type() {
        let generator = MysqlSqlGenerator::new();

        let mut table = Table::new("permissions".to_string());
        table.add_column(Column {
            name: "roles".to_string(),
            column_type: ColumnType::DialectSpecific {
                kind: "SET".to_string(),
                params: serde_json::json!({ "values": ["read", "write", "delete"] }),
            },
            nullable: false,
            default_value: None,
            auto_increment: None,
        });

        let sql = generator.generate_create_table(&table);

        // SET('read', 'write', 'delete')の形式で出力されているか確認
        assert!(sql.contains("SET("), "Expected 'SET(' in SQL: {}", sql);
        assert!(sql.contains("'read'"), "Expected 'read' in SQL: {}", sql);
        assert!(sql.contains("'write'"), "Expected 'write' in SQL: {}", sql);
        assert!(
            sql.contains("'delete'"),
            "Expected 'delete' in SQL: {}",
            sql
        );
    }
}

#[cfg(test)]
mod sqlite_dialect_specific_tests {
    use super::*;

    /// SQLite方言固有型のSQL生成（パラメータなし）
    #[test]
    fn test_sqlite_generate_dialect_specific_type() {
        let generator = SqliteSqlGenerator::new();

        let mut table = Table::new("items".to_string());
        table.add_column(Column {
            name: "data".to_string(),
            column_type: ColumnType::DialectSpecific {
                kind: "BLOB".to_string(),
                params: serde_json::json!(null),
            },
            nullable: true,
            default_value: None,
            auto_increment: None,
        });

        let sql = generator.generate_create_table(&table);

        // BLOBがそのまま出力されているか確認
        assert!(
            sql.contains("data BLOB"),
            "Expected 'data BLOB' in SQL: {}",
            sql
        );
    }
}

#[cfg(test)]
mod mixed_types_tests {
    use super::*;

    /// 共通型と方言固有型の混在テーブルのSQL生成
    #[test]
    fn test_mixed_common_and_dialect_specific_types() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("products".to_string());

        // 方言固有型（SERIAL）
        table.add_column(Column {
            name: "id".to_string(),
            column_type: ColumnType::DialectSpecific {
                kind: "SERIAL".to_string(),
                params: serde_json::json!(null),
            },
            nullable: false,
            default_value: None,
            auto_increment: None,
        });

        // 共通型（VARCHAR）
        table.add_column(Column {
            name: "name".to_string(),
            column_type: ColumnType::VARCHAR { length: 255 },
            nullable: false,
            default_value: None,
            auto_increment: None,
        });

        // 共通型（DECIMAL）
        table.add_column(Column {
            name: "price".to_string(),
            column_type: ColumnType::DECIMAL {
                precision: 10,
                scale: 2,
            },
            nullable: false,
            default_value: None,
            auto_increment: None,
        });

        let sql = generator.generate_create_table(&table);

        // 3つのカラムが全て正しく生成されているか確認
        assert!(
            sql.contains("id SERIAL"),
            "Expected 'id SERIAL' in SQL: {}",
            sql
        );
        assert!(
            sql.contains("name VARCHAR(255)"),
            "Expected 'name VARCHAR(255)' in SQL: {}",
            sql
        );
        assert!(
            sql.contains("price NUMERIC(10, 2)"),
            "Expected 'price NUMERIC(10, 2)' in SQL: {}",
            sql
        );
    }
}
