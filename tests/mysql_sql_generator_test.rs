/// MySQL用SQLジェネレーターのテスト
///
/// スキーマ定義からMySQL用のDDL文を正しく生成することを確認します。

#[cfg(test)]
mod mysql_sql_generator_tests {
    use stratum::adapters::sql_generator::mysql::MysqlSqlGenerator;
    use stratum::adapters::sql_generator::SqlGenerator;
    use stratum::core::schema::{Column, ColumnType, Constraint, Index, Table};

    /// ジェネレーターの作成テスト
    #[test]
    fn test_new_generator() {
        let generator = MysqlSqlGenerator::new();
        assert!(format!("{:?}", generator).contains("MysqlSqlGenerator"));
    }

    /// 基本的なCREATE TABLE文の生成テスト
    #[test]
    fn test_generate_create_table_basic() {
        let generator = MysqlSqlGenerator::new();

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
        assert!(sql.contains("id INT NOT NULL"));
        assert!(sql.contains("name VARCHAR(100) NOT NULL"));
    }

    /// PRIMARY KEY制約を含むCREATE TABLE文の生成テスト
    #[test]
    fn test_generate_create_table_with_primary_key() {
        let generator = MysqlSqlGenerator::new();

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
        let generator = MysqlSqlGenerator::new();

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
        let generator = MysqlSqlGenerator::new();

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "bio".to_string(),
            ColumnType::TEXT,
            true, // nullable
        ));

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("bio TEXT"));
        // MySQLでは明示的にNULLを指定しない場合がある
        assert!(!sql.contains("bio TEXT NOT NULL"));
    }

    /// デフォルト値を持つカラムのテスト
    #[test]
    fn test_generate_create_table_with_default_value() {
        let generator = MysqlSqlGenerator::new();

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
        let generator = MysqlSqlGenerator::new();

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
        let generator = MysqlSqlGenerator::new();

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

    /// CHECK制約のテスト（MySQL 8.0.16以降）
    #[test]
    fn test_generate_create_table_with_check_constraint() {
        let generator = MysqlSqlGenerator::new();

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
        let generator = MysqlSqlGenerator::new();

        let table = Table::new("users".to_string());
        let index = Index::new("idx_email".to_string(), vec!["email".to_string()], false);

        let sql = generator.generate_create_index(&table, &index);

        assert!(sql.contains("CREATE INDEX idx_email"));
        assert!(sql.contains("ON users"));
        assert!(sql.contains("(email)"));
    }

    /// CREATE UNIQUE INDEX文の生成テスト
    #[test]
    fn test_generate_create_unique_index() {
        let generator = MysqlSqlGenerator::new();

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
        let generator = MysqlSqlGenerator::new();

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

    /// 様々なMySQL型のマッピングテスト
    #[test]
    fn test_column_type_mapping() {
        let generator = MysqlSqlGenerator::new();

        let mut table = Table::new("test_types".to_string());

        // INT
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

        // BOOLEAN (MySQLではTINYINT(1))
        table.add_column(Column::new(
            "col_bool".to_string(),
            ColumnType::BOOLEAN,
            false,
        ));

        // TIMESTAMP
        table.add_column(Column::new(
            "col_timestamp".to_string(),
            ColumnType::TIMESTAMP {
                with_time_zone: Some(false),
            },
            false,
        ));

        // JSON
        table.add_column(Column::new("col_json".to_string(), ColumnType::JSON, false));

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("col_int INT NOT NULL"));
        assert!(sql.contains("col_bigint BIGINT NOT NULL"));
        assert!(sql.contains("col_varchar VARCHAR(255) NOT NULL"));
        assert!(sql.contains("col_text TEXT NOT NULL"));
        assert!(sql.contains("col_bool BOOLEAN NOT NULL"));
        assert!(sql.contains("col_timestamp TIMESTAMP NOT NULL"));
        assert!(sql.contains("col_json JSON NOT NULL"));
    }

    /// AUTO_INCREMENTのテスト
    #[test]
    fn test_generate_create_table_with_auto_increment() {
        let generator = MysqlSqlGenerator::new();

        let mut table = Table::new("users".to_string());
        let mut id_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        id_column.auto_increment = Some(true);
        table.add_column(id_column);

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("id INT NOT NULL AUTO_INCREMENT"));
    }

    /// BIGINT AUTO_INCREMENTのテスト
    #[test]
    fn test_generate_create_table_with_bigint_auto_increment() {
        let generator = MysqlSqlGenerator::new();

        let mut table = Table::new("logs".to_string());
        let mut id_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) },
            false,
        );
        id_column.auto_increment = Some(true);
        table.add_column(id_column);

        let sql = generator.generate_create_table(&table);

        assert!(sql.contains("id BIGINT NOT NULL AUTO_INCREMENT"));
    }

    /// 複数の制約を含むテーブルのテスト
    #[test]
    fn test_generate_create_table_complex() {
        let generator = MysqlSqlGenerator::new();

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
        assert!(sql.contains("id INT NOT NULL AUTO_INCREMENT"));
        assert!(sql.contains("email VARCHAR(255) NOT NULL"));
        assert!(sql.contains("status VARCHAR(20) NOT NULL DEFAULT 'active'"));
        assert!(sql.contains("PRIMARY KEY (id)"));
        assert!(sql.contains("UNIQUE (email)"));
        assert!(sql.contains("CHECK (status IN ('active', 'inactive'))"));
    }

    /// ENGINE指定のテスト（MySQLのデフォルトはInnoDB）
    #[test]
    fn test_generate_create_table_with_engine() {
        let generator = MysqlSqlGenerator::new();

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        let sql = generator.generate_create_table(&table);

        // ENGINEはデフォルトで指定しない（MySQLのデフォルトを使用）
        // または明示的にENGINE=InnoDBを指定する場合もある
        assert!(sql.contains("CREATE TABLE users"));
    }

    // Phase 4: 新規データ型のマッピングテスト

    #[test]
    fn test_map_column_type_decimal() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 10,
                scale: 2,
            },
            false,
        ));

        let sql = generator.generate_create_table(&table);
        assert!(sql.contains("price DECIMAL(10, 2) NOT NULL"));
    }

    #[test]
    fn test_map_column_type_float() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("measurements".to_string());
        table.add_column(Column::new("value".to_string(), ColumnType::FLOAT, false));

        let sql = generator.generate_create_table(&table);
        assert!(sql.contains("value FLOAT NOT NULL"));
    }

    #[test]
    fn test_map_column_type_double() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("coordinates".to_string());
        table.add_column(Column::new(
            "latitude".to_string(),
            ColumnType::DOUBLE,
            false,
        ));

        let sql = generator.generate_create_table(&table);
        assert!(sql.contains("latitude DOUBLE NOT NULL"));
    }

    #[test]
    fn test_map_column_type_char() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("codes".to_string());
        table.add_column(Column::new(
            "code".to_string(),
            ColumnType::CHAR { length: 10 },
            false,
        ));

        let sql = generator.generate_create_table(&table);
        assert!(sql.contains("code CHAR(10) NOT NULL"));
    }

    #[test]
    fn test_map_column_type_date() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("events".to_string());
        table.add_column(Column::new(
            "event_date".to_string(),
            ColumnType::DATE,
            false,
        ));

        let sql = generator.generate_create_table(&table);
        assert!(sql.contains("event_date DATE NOT NULL"));
    }

    #[test]
    fn test_map_column_type_time() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("schedules".to_string());
        table.add_column(Column::new(
            "start_time".to_string(),
            ColumnType::TIME {
                with_time_zone: None,
            },
            false,
        ));

        let sql = generator.generate_create_table(&table);
        assert!(sql.contains("start_time TIME NOT NULL"));
    }

    #[test]
    fn test_map_column_type_blob() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("documents".to_string());
        table.add_column(Column::new("content".to_string(), ColumnType::BLOB, false));

        let sql = generator.generate_create_table(&table);
        assert!(sql.contains("content BLOB NOT NULL"));
    }

    #[test]
    fn test_map_column_type_uuid() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new("user_id".to_string(), ColumnType::UUID, false));

        let sql = generator.generate_create_table(&table);
        // MySQL では UUID を CHAR(36) にマッピング
        assert!(sql.contains("user_id CHAR(36) NOT NULL"));
    }

    #[test]
    fn test_map_column_type_jsonb() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("settings".to_string());
        table.add_column(Column::new("config".to_string(), ColumnType::JSONB, false));

        let sql = generator.generate_create_table(&table);
        // MySQL では JSONB を JSON にフォールバック
        assert!(sql.contains("config JSON NOT NULL"));
    }
}
