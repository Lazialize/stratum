/// PostgreSQL用SQLジェネレーターのテスト
///
/// スキーマ定義からPostgreSQL用のDDL文を正しく生成することを確認します。

#[cfg(test)]
mod postgres_sql_generator_tests {
    use stratum::adapters::sql_generator::postgres::PostgresSqlGenerator;
    use stratum::adapters::sql_generator::SqlGenerator;
    use stratum::core::schema::{Column, ColumnType, Constraint, Index, Table};

    /// ジェネレーターの作成テスト
    #[test]
    fn test_new_generator() {
        let generator = PostgresSqlGenerator::new();
        assert!(format!("{:?}", generator).contains("PostgresSqlGenerator"));
    }

    /// 基本的なCREATE TABLE文の生成テスト
    #[test]
    fn test_generate_create_table_basic() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("CREATE TABLE users"));
        assert!(sql.contains("id INTEGER NOT NULL"));
        assert!(sql.contains("name VARCHAR(100) NOT NULL"));
    }

    /// PRIMARY KEY制約を含むCREATE TABLE文の生成テスト
    #[test]
    fn test_generate_create_table_with_primary_key() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("CREATE TABLE users"));
        assert!(sql.contains("PRIMARY KEY (id)"));
    }

    /// 複合PRIMARY KEY制約のテスト
    #[test]
    fn test_generate_create_table_with_composite_primary_key() {
        let generator = PostgresSqlGenerator::new();

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

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("PRIMARY KEY (user_id, role_id)"));
    }

    /// NULL許可カラムのテスト
    #[test]
    fn test_generate_create_table_with_nullable_column() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "bio".to_string(),
            ColumnType::TEXT,
            true, // nullable
        ));

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("bio TEXT"));
        assert!(!sql.contains("bio TEXT NOT NULL"));
    }

    /// デフォルト値を持つカラムのテスト
    #[test]
    fn test_generate_create_table_with_default_value() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("users".to_string());
        let mut column = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 20 },
            false,
        );
        column.default_value = Some("'active'".to_string());
        table.add_column(column);

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("status VARCHAR(20) NOT NULL DEFAULT 'active'"));
    }

    /// UNIQUE制約を含むCREATE TABLE文の生成テスト
    #[test]
    fn test_generate_create_table_with_unique_constraint() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table.add_constraint(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("UNIQUE (email)"));
    }

    /// FOREIGN KEY制約のALTER TABLE文生成テスト
    #[test]
    fn test_generate_alter_table_add_foreign_key() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("posts".to_string());
        table.add_column(Column::new(
            "user_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
        });

        let sql = generator.generate_alter_table_add_constraint(&table, 0);

        assert!(sql.contains("ALTER TABLE posts"));
        assert!(sql.contains("ADD CONSTRAINT"));
        assert!(sql.contains("FOREIGN KEY (user_id)"));
        assert!(sql.contains("REFERENCES users (id)"));
    }

    /// CHECK制約のテスト
    #[test]
    fn test_generate_create_table_with_check_constraint() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        });

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("CHECK (price >= 0)"));
    }

    /// CREATE INDEX文の生成テスト（通常のインデックス）
    #[test]
    fn test_generate_create_index() {
        let generator = PostgresSqlGenerator::new();

        let table = Table::new("users".to_string());
        let index = Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            false,
        );

        let sql = generator.generate_create_index(&table, &index);

        assert!(sql.contains("CREATE INDEX idx_email"));
        assert!(sql.contains("ON users"));
        assert!(sql.contains("(email)"));
    }

    /// CREATE UNIQUE INDEX文の生成テスト
    #[test]
    fn test_generate_create_unique_index() {
        let generator = PostgresSqlGenerator::new();

        let table = Table::new("users".to_string());
        let index = Index::new(
            "idx_username".to_string(),
            vec!["username".to_string()],
            true, // unique
        );

        let sql = generator.generate_create_index(&table, &index);

        assert!(sql.contains("CREATE UNIQUE INDEX idx_username"));
        assert!(sql.contains("ON users"));
        assert!(sql.contains("(username)"));
    }

    /// 複合インデックスの生成テスト
    #[test]
    fn test_generate_create_composite_index() {
        let generator = PostgresSqlGenerator::new();

        let table = Table::new("posts".to_string());
        let index = Index::new(
            "idx_user_created".to_string(),
            vec!["user_id".to_string(), "created_at".to_string()],
            false,
        );

        let sql = generator.generate_create_index(&table, &index);

        assert!(sql.contains("CREATE INDEX idx_user_created"));
        assert!(sql.contains("ON posts"));
        assert!(sql.contains("(user_id, created_at)"));
    }

    /// 様々なPostgreSQL型のマッピングテスト
    #[test]
    fn test_column_type_mapping() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("test_types".to_string());

        // INTEGER
        table.add_column(Column::new(
            "col_int".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        // BIGINT (precision: 8)
        table.add_column(Column::new(
            "col_bigint".to_string(),
            ColumnType::INTEGER { precision: Some(8) },
            false,
        ));

        // VARCHAR
        table.add_column(Column::new(
            "col_varchar".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));

        // TEXT
        table.add_column(Column::new("col_text".to_string(), ColumnType::TEXT, false));

        // BOOLEAN
        table.add_column(Column::new(
            "col_bool".to_string(),
            ColumnType::BOOLEAN,
            false,
        ));

        // TIMESTAMP
        table.add_column(Column::new(
            "col_timestamp".to_string(),
            ColumnType::TIMESTAMP {
                with_time_zone: Some(true),
            },
            false,
        ));

        // JSON
        table.add_column(Column::new(
            "col_json".to_string(),
            ColumnType::JSON,
            false,
        ));

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("col_int INTEGER NOT NULL"));
        assert!(sql.contains("col_bigint BIGINT NOT NULL"));
        assert!(sql.contains("col_varchar VARCHAR(255) NOT NULL"));
        assert!(sql.contains("col_text TEXT NOT NULL"));
        assert!(sql.contains("col_bool BOOLEAN NOT NULL"));
        assert!(sql.contains("col_timestamp TIMESTAMP WITH TIME ZONE NOT NULL"));
        assert!(sql.contains("col_json JSON NOT NULL"));
    }

    /// AUTO_INCREMENT（SERIAL）のテスト
    #[test]
    fn test_generate_create_table_with_serial() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("users".to_string());
        let mut id_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        id_column.auto_increment = Some(true);
        table.add_column(id_column);

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("id SERIAL NOT NULL"));
    }

    /// BIGSERIAL（BIGINT AUTO_INCREMENT）のテスト
    #[test]
    fn test_generate_create_table_with_bigserial() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("logs".to_string());
        let mut id_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) },
            false,
        );
        id_column.auto_increment = Some(true);
        table.add_column(id_column);

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("id BIGSERIAL NOT NULL"));
    }

    /// 複数の制約を含むテーブルのテスト
    #[test]
    fn test_generate_create_table_complex() {
        let generator = PostgresSqlGenerator::new();

        let mut table = Table::new("users".to_string());

        let mut id_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        id_column.auto_increment = Some(true);
        table.add_column(id_column);

        table.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));

        let mut status_column = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 20 },
            false,
        );
        status_column.default_value = Some("'active'".to_string());
        table.add_column(status_column);

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        table.add_constraint(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });

        table.add_constraint(Constraint::CHECK {
            columns: vec!["status".to_string()],
            check_expression: "status IN ('active', 'inactive')".to_string(),
        });

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("CREATE TABLE users"));
        assert!(sql.contains("id SERIAL NOT NULL"));
        assert!(sql.contains("email VARCHAR(255) NOT NULL"));
        assert!(sql.contains("status VARCHAR(20) NOT NULL DEFAULT 'active'"));
        assert!(sql.contains("PRIMARY KEY (id)"));
        assert!(sql.contains("UNIQUE (email)"));
        assert!(sql.contains("CHECK (status IN ('active', 'inactive'))"));
    }
}
