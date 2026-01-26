/// 型変更マイグレーションの統合テスト
///
/// PostgreSQL、MySQL、SQLiteでの型変更マイグレーション生成と
/// 型変更検証機能をテストします。
#[cfg(test)]
mod type_change_migration_tests {
    use strata::adapters::sql_generator::mysql::MysqlSqlGenerator;
    use strata::adapters::sql_generator::postgres::PostgresSqlGenerator;
    use strata::adapters::sql_generator::sqlite::SqliteSqlGenerator;
    use strata::adapters::sql_generator::{MigrationDirection, SqlGenerator};
    use strata::core::config::Dialect;
    use strata::core::schema::{Column, ColumnType, Constraint, Index, Schema, Table};
    use strata::core::schema_diff::{ColumnDiff, SchemaDiff, TableDiff};
    use strata::services::migration_generator::MigrationGenerator;
    use strata::services::type_change_validator::TypeChangeValidator;

    // ==========================================
    // Task 9.1: PostgreSQL型変更統合テスト
    // ==========================================

    mod postgres_type_change_tests {
        use super::*;

        /// INTEGER → VARCHAR の型変更SQL生成（USING句不要）
        #[test]
        fn test_postgres_integer_to_varchar_no_using() {
            let generator = PostgresSqlGenerator::new();

            let old_column = Column::new(
                "age".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            );
            let new_column =
                Column::new("age".to_string(), ColumnType::VARCHAR { length: 50 }, true);
            let column_diff = ColumnDiff::new("age".to_string(), old_column, new_column);

            let table = Table::new("users".to_string());

            // Up方向のSQL生成
            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

            assert_eq!(up_sql.len(), 1);
            assert!(up_sql[0].contains(r#"ALTER TABLE "users" ALTER COLUMN "age" TYPE VARCHAR(50)"#));
            // Numeric → String は USING句不要
            assert!(!up_sql[0].contains("USING"));
        }

        /// VARCHAR → INTEGER の型変更SQL生成（USING句必要）
        #[test]
        fn test_postgres_varchar_to_integer_with_using() {
            let generator = PostgresSqlGenerator::new();

            let old_column = Column::new(
                "price".to_string(),
                ColumnType::VARCHAR { length: 50 },
                false,
            );
            let new_column = Column::new(
                "price".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            );
            let column_diff = ColumnDiff::new("price".to_string(), old_column, new_column);

            let table = Table::new("products".to_string());

            // Up方向のSQL生成
            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

            assert_eq!(up_sql.len(), 1);
            assert!(up_sql[0].contains(r#"ALTER TABLE "products" ALTER COLUMN "price" TYPE INTEGER"#));
            // String → Numeric は USING句必要
            assert!(up_sql[0].contains("USING"));
        }

        /// BOOLEAN → TEXT の型変更SQL生成（USING句不要）
        #[test]
        fn test_postgres_boolean_to_text_no_using() {
            let generator = PostgresSqlGenerator::new();

            let old_column = Column::new("is_active".to_string(), ColumnType::BOOLEAN, false);
            let new_column = Column::new("is_active".to_string(), ColumnType::TEXT, false);
            let column_diff = ColumnDiff::new("is_active".to_string(), old_column, new_column);

            let table = Table::new("users".to_string());

            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

            assert_eq!(up_sql.len(), 1);
            assert!(up_sql[0].contains(r#"ALTER TABLE "users" ALTER COLUMN "is_active" TYPE TEXT"#));
            // Boolean → String は USING句不要
            assert!(!up_sql[0].contains("USING"));
        }

        /// TIMESTAMP → VARCHAR の型変更SQL生成（USING句不要）
        #[test]
        fn test_postgres_timestamp_to_varchar_no_using() {
            let generator = PostgresSqlGenerator::new();

            let old_column = Column::new(
                "created_at".to_string(),
                ColumnType::TIMESTAMP {
                    with_time_zone: None,
                },
                false,
            );
            let new_column = Column::new(
                "created_at".to_string(),
                ColumnType::VARCHAR { length: 100 },
                false,
            );
            let column_diff = ColumnDiff::new("created_at".to_string(), old_column, new_column);

            let table = Table::new("events".to_string());

            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

            assert_eq!(up_sql.len(), 1);
            assert!(
                up_sql[0].contains(r#"ALTER TABLE "events" ALTER COLUMN "created_at" TYPE VARCHAR(100)"#)
            );
            // DateTime → String は USING句不要
            assert!(!up_sql[0].contains("USING"));
        }

        /// Up/Down ロールバック検証（INTEGER → VARCHAR → INTEGER）
        #[test]
        fn test_postgres_up_down_rollback() {
            let generator = PostgresSqlGenerator::new();

            let old_column = Column::new(
                "score".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            );
            let new_column = Column::new(
                "score".to_string(),
                ColumnType::VARCHAR { length: 20 },
                true,
            );
            let column_diff = ColumnDiff::new("score".to_string(), old_column, new_column);

            let table = Table::new("results".to_string());

            // Up: INTEGER → VARCHAR
            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);
            assert!(up_sql[0].contains("TYPE VARCHAR(20)"));

            // Down: VARCHAR → INTEGER
            let down_sql = generator.generate_alter_column_type(
                &table,
                &column_diff,
                MigrationDirection::Down,
            );
            assert!(down_sql[0].contains("TYPE INTEGER"));
            // Down方向では String → Numeric なので USING句必要
            assert!(down_sql[0].contains("USING"));
        }

        /// MigrationGeneratorでのPostgreSQL型変更SQL生成統合テスト
        #[test]
        fn test_postgres_migration_generator_type_change() {
            let generator = MigrationGenerator::new();

            // スキーマ作成
            let mut old_schema = Schema::new("1.0".to_string());
            let mut old_table = Table::new("users".to_string());
            old_table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            old_table.columns.push(Column::new(
                "age".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            ));
            old_table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });
            old_schema.tables.insert("users".to_string(), old_table);

            let mut new_schema = Schema::new("1.0".to_string());
            let mut new_table = Table::new("users".to_string());
            new_table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            new_table.columns.push(Column::new(
                "age".to_string(),
                ColumnType::VARCHAR { length: 50 },
                true,
            ));
            new_table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });
            new_schema.tables.insert("users".to_string(), new_table);

            // Diffを作成
            let mut diff = SchemaDiff::new();
            let old_column = Column::new(
                "age".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            );
            let new_column =
                Column::new("age".to_string(), ColumnType::VARCHAR { length: 50 }, true);
            let column_diff = ColumnDiff::new("age".to_string(), old_column, new_column);

            let mut table_diff = TableDiff::new("users".to_string());
            table_diff.modified_columns.push(column_diff);
            diff.modified_tables.push(table_diff);

            // SQL生成
            let result = generator.generate_up_sql_with_schemas(
                &diff,
                &old_schema,
                &new_schema,
                Dialect::PostgreSQL,
            );

            assert!(result.is_ok());
            let (sql, validation_result) = result.unwrap();
            assert!(sql.contains(r#"ALTER TABLE "users" ALTER COLUMN "age" TYPE"#));
            // Numeric → String は安全なので警告なし
            assert!(validation_result.is_valid());
        }

        /// DECIMAL 精度変更のSQL生成
        #[test]
        fn test_postgres_decimal_precision_change() {
            let generator = PostgresSqlGenerator::new();

            let old_column = Column::new(
                "price".to_string(),
                ColumnType::DECIMAL {
                    precision: 10,
                    scale: 2,
                },
                false,
            );
            let new_column = Column::new(
                "price".to_string(),
                ColumnType::DECIMAL {
                    precision: 15,
                    scale: 4,
                },
                false,
            );
            let column_diff = ColumnDiff::new("price".to_string(), old_column, new_column);

            let table = Table::new("products".to_string());

            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

            assert_eq!(up_sql.len(), 1);
            assert!(up_sql[0].contains("TYPE NUMERIC(15, 4)"));
        }
    }

    // ==========================================
    // Task 9.2: MySQL型変更統合テスト
    // ==========================================

    mod mysql_type_change_tests {
        use super::*;

        /// INTEGER → VARCHAR の型変更SQL生成（MODIFY COLUMN）
        #[test]
        fn test_mysql_integer_to_varchar() {
            let generator = MysqlSqlGenerator::new();

            let old_column = Column::new(
                "age".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            );
            let new_column =
                Column::new("age".to_string(), ColumnType::VARCHAR { length: 50 }, true);
            let column_diff = ColumnDiff::new("age".to_string(), old_column, new_column);

            let mut table = Table::new("users".to_string());
            // MySQLではテーブルからカラム情報を取得するので、新しいカラムを追加
            table.columns.push(Column::new(
                "age".to_string(),
                ColumnType::VARCHAR { length: 50 },
                true,
            ));

            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

            assert_eq!(up_sql.len(), 1);
            assert!(up_sql[0].contains("ALTER TABLE `users` MODIFY COLUMN `age`"));
            assert!(up_sql[0].contains("VARCHAR(50)"));
        }

        /// NOT NULL制約の保持確認
        #[test]
        fn test_mysql_preserve_not_null() {
            let generator = MysqlSqlGenerator::new();

            let old_column = Column::new(
                "email".to_string(),
                ColumnType::VARCHAR { length: 100 },
                false, // NOT NULL
            );
            let new_column = Column::new(
                "email".to_string(),
                ColumnType::VARCHAR { length: 255 },
                false, // NOT NULL
            );
            let column_diff = ColumnDiff::new("email".to_string(), old_column, new_column);

            let mut table = Table::new("users".to_string());
            let col = Column::new(
                "email".to_string(),
                ColumnType::VARCHAR { length: 255 },
                false,
            );
            table.columns.push(col);

            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

            assert!(up_sql[0].contains("NOT NULL"));
        }

        /// DEFAULT値の保持確認
        #[test]
        fn test_mysql_preserve_default_value() {
            let generator = MysqlSqlGenerator::new();

            let mut old_column = Column::new(
                "status".to_string(),
                ColumnType::VARCHAR { length: 20 },
                false,
            );
            old_column.default_value = Some("'active'".to_string());

            let mut new_column = Column::new(
                "status".to_string(),
                ColumnType::VARCHAR { length: 50 },
                false,
            );
            new_column.default_value = Some("'active'".to_string());

            let column_diff = ColumnDiff::new("status".to_string(), old_column, new_column.clone());

            let mut table = Table::new("users".to_string());
            table.columns.push(new_column);

            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

            assert!(up_sql[0].contains("DEFAULT 'active'"));
        }

        /// Up/Down ロールバック検証
        #[test]
        fn test_mysql_up_down_rollback() {
            let generator = MysqlSqlGenerator::new();

            let old_column = Column::new(
                "score".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            );
            let new_column = Column::new(
                "score".to_string(),
                ColumnType::VARCHAR { length: 20 },
                true,
            );
            let column_diff =
                ColumnDiff::new("score".to_string(), old_column.clone(), new_column.clone());

            // Up用テーブル（新しい型）
            let mut up_table = Table::new("results".to_string());
            up_table.columns.push(new_column);

            // Down用テーブル（元の型）
            let mut down_table = Table::new("results".to_string());
            down_table.columns.push(old_column);

            // Up: INTEGER → VARCHAR
            let up_sql = generator.generate_alter_column_type(
                &up_table,
                &column_diff,
                MigrationDirection::Up,
            );
            assert!(up_sql[0].contains("MODIFY COLUMN `score`"));
            assert!(up_sql[0].contains("VARCHAR(20)"));

            // Down: VARCHAR → INTEGER
            let down_sql = generator.generate_alter_column_type(
                &down_table,
                &column_diff,
                MigrationDirection::Down,
            );
            assert!(down_sql[0].contains("MODIFY COLUMN `score`"));
            assert!(down_sql[0].contains("INT"));
        }

        /// MigrationGeneratorでのMySQL型変更SQL生成統合テスト
        #[test]
        fn test_mysql_migration_generator_type_change() {
            let generator = MigrationGenerator::new();

            // スキーマ作成
            let mut old_schema = Schema::new("1.0".to_string());
            let mut old_table = Table::new("products".to_string());
            old_table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            old_table.columns.push(Column::new(
                "name".to_string(),
                ColumnType::VARCHAR { length: 100 },
                false,
            ));
            old_schema.tables.insert("products".to_string(), old_table);

            let mut new_schema = Schema::new("1.0".to_string());
            let mut new_table = Table::new("products".to_string());
            new_table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            new_table.columns.push(Column::new(
                "name".to_string(),
                ColumnType::VARCHAR { length: 255 },
                false,
            ));
            new_schema.tables.insert("products".to_string(), new_table);

            // Diffを作成
            let mut diff = SchemaDiff::new();
            let old_column = Column::new(
                "name".to_string(),
                ColumnType::VARCHAR { length: 100 },
                false,
            );
            let new_column = Column::new(
                "name".to_string(),
                ColumnType::VARCHAR { length: 255 },
                false,
            );
            let column_diff = ColumnDiff::new("name".to_string(), old_column, new_column);

            let mut table_diff = TableDiff::new("products".to_string());
            table_diff.modified_columns.push(column_diff);
            diff.modified_tables.push(table_diff);

            let result = generator.generate_up_sql_with_schemas(
                &diff,
                &old_schema,
                &new_schema,
                Dialect::MySQL,
            );

            assert!(result.is_ok());
            let (sql, _) = result.unwrap();
            assert!(sql.contains("MODIFY COLUMN `name`"));
        }
    }

    // ==========================================
    // Task 9.3: SQLiteテーブル再作成統合テスト
    // ==========================================

    mod sqlite_type_change_tests {
        use super::*;

        /// テーブル再作成パターンのSQL生成
        #[test]
        fn test_sqlite_table_recreate_pattern() {
            let generator = SqliteSqlGenerator::new();

            let old_column = Column::new(
                "age".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            );
            let new_column = Column::new("age".to_string(), ColumnType::TEXT, true);
            let column_diff = ColumnDiff::new("age".to_string(), old_column, new_column);

            let mut table = Table::new("users".to_string());
            table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            table
                .columns
                .push(Column::new("age".to_string(), ColumnType::TEXT, true));
            table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });

            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

            // SQLiteはテーブル再作成パターンを使用
            assert!(!up_sql.is_empty());
            let combined_sql = up_sql.join("\n");

            // 外部キー制約の一時無効化
            assert!(combined_sql.contains("PRAGMA foreign_keys=off"));

            // トランザクション
            assert!(combined_sql.contains("BEGIN TRANSACTION"));

            // 新テーブル作成
            assert!(combined_sql.contains(r#"CREATE TABLE "new_users""#));

            // データコピー
            assert!(combined_sql.contains(r#"INSERT INTO "new_users""#));
            assert!(combined_sql.contains("SELECT"));

            // 旧テーブル削除
            assert!(combined_sql.contains(r#"DROP TABLE "users""#));

            // リネーム
            assert!(combined_sql.contains(r#"ALTER TABLE "new_users" RENAME TO "users""#));

            // 外部キー制約の有効化
            assert!(combined_sql.contains("PRAGMA foreign_keys=on"));

            // コミット
            assert!(combined_sql.contains("COMMIT"));
        }

        /// データコピーで共通カラムのみ選択される確認
        #[test]
        fn test_sqlite_data_copy_common_columns() {
            let generator = SqliteSqlGenerator::new();

            let old_column = Column::new(
                "name".to_string(),
                ColumnType::VARCHAR { length: 100 },
                false,
            );
            let new_column = Column::new("name".to_string(), ColumnType::TEXT, false);
            let column_diff = ColumnDiff::new("name".to_string(), old_column, new_column);

            let mut table = Table::new("users".to_string());
            table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            table
                .columns
                .push(Column::new("name".to_string(), ColumnType::TEXT, false));
            table.columns.push(Column::new(
                "email".to_string(),
                ColumnType::VARCHAR { length: 255 },
                false,
            ));
            table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });

            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);
            let combined_sql = up_sql.join("\n");

            // 明示的なカラムリスト
            assert!(combined_sql.contains("id"));
            assert!(combined_sql.contains("name"));
            assert!(combined_sql.contains("email"));
        }

        /// インデックスの再作成確認
        #[test]
        fn test_sqlite_index_recreation() {
            let generator = SqliteSqlGenerator::new();

            let old_column = Column::new(
                "email".to_string(),
                ColumnType::VARCHAR { length: 100 },
                false,
            );
            let new_column = Column::new(
                "email".to_string(),
                ColumnType::VARCHAR { length: 255 },
                false,
            );
            let column_diff = ColumnDiff::new("email".to_string(), old_column, new_column);

            let mut table = Table::new("users".to_string());
            table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            table.columns.push(Column::new(
                "email".to_string(),
                ColumnType::VARCHAR { length: 255 },
                false,
            ));
            table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });
            table.indexes.push(Index::new(
                "idx_email".to_string(),
                vec!["email".to_string()],
                true, // unique
            ));

            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);
            let combined_sql = up_sql.join("\n");

            // インデックスの再作成
            assert!(combined_sql.contains(r#"CREATE UNIQUE INDEX "idx_email""#));
        }

        /// 外部キー整合性チェック
        #[test]
        fn test_sqlite_foreign_key_check() {
            let generator = SqliteSqlGenerator::new();

            let old_column = Column::new(
                "name".to_string(),
                ColumnType::VARCHAR { length: 50 },
                false,
            );
            let new_column = Column::new("name".to_string(), ColumnType::TEXT, false);
            let column_diff = ColumnDiff::new("name".to_string(), old_column, new_column);

            let mut table = Table::new("users".to_string());
            table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            table
                .columns
                .push(Column::new("name".to_string(), ColumnType::TEXT, false));
            table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });

            let up_sql =
                generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);
            let combined_sql = up_sql.join("\n");

            // 外部キー整合性チェック
            assert!(combined_sql.contains("PRAGMA foreign_key_check"));
        }

        /// Up/Down ロールバック検証
        #[test]
        fn test_sqlite_up_down_rollback() {
            let generator = SqliteSqlGenerator::new();

            let old_column = Column::new(
                "score".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            );
            let new_column = Column::new("score".to_string(), ColumnType::TEXT, true);
            let column_diff =
                ColumnDiff::new("score".to_string(), old_column.clone(), new_column.clone());

            // Up用テーブル（新しい型）
            let mut up_table = Table::new("results".to_string());
            up_table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            up_table.columns.push(new_column.clone());
            up_table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });

            // Down用テーブル（元の型）
            let mut down_table = Table::new("results".to_string());
            down_table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            down_table.columns.push(old_column);
            down_table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });

            // Up: INTEGER → TEXT
            let up_sql = generator.generate_alter_column_type(
                &up_table,
                &column_diff,
                MigrationDirection::Up,
            );
            let up_combined = up_sql.join("\n");
            assert!(up_combined.contains(r#"CREATE TABLE "new_results""#));
            assert!(up_combined.contains("TEXT"));

            // Down: TEXT → INTEGER
            let down_sql = generator.generate_alter_column_type(
                &down_table,
                &column_diff,
                MigrationDirection::Down,
            );
            let down_combined = down_sql.join("\n");
            assert!(down_combined.contains(r#"CREATE TABLE "new_results""#));
            assert!(down_combined.contains("INTEGER"));
        }

        /// MigrationGeneratorでのSQLite型変更SQL生成統合テスト
        #[test]
        fn test_sqlite_migration_generator_type_change() {
            let generator = MigrationGenerator::new();

            // スキーマ作成
            let mut old_schema = Schema::new("1.0".to_string());
            let mut old_table = Table::new("users".to_string());
            old_table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            old_table.columns.push(Column::new(
                "age".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            ));
            old_table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });
            old_schema.tables.insert("users".to_string(), old_table);

            let mut new_schema = Schema::new("1.0".to_string());
            let mut new_table = Table::new("users".to_string());
            new_table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            new_table
                .columns
                .push(Column::new("age".to_string(), ColumnType::TEXT, true));
            new_table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });
            new_schema.tables.insert("users".to_string(), new_table);

            // Diffを作成
            let mut diff = SchemaDiff::new();
            let old_column = Column::new(
                "age".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            );
            let new_column = Column::new("age".to_string(), ColumnType::TEXT, true);
            let column_diff = ColumnDiff::new("age".to_string(), old_column, new_column);

            let mut table_diff = TableDiff::new("users".to_string());
            table_diff.modified_columns.push(column_diff);
            diff.modified_tables.push(table_diff);

            let result = generator.generate_up_sql_with_schemas(
                &diff,
                &old_schema,
                &new_schema,
                Dialect::SQLite,
            );

            assert!(result.is_ok());
            let (sql, _) = result.unwrap();
            assert!(sql.contains("PRAGMA foreign_keys=off"));
            assert!(sql.contains(r#"CREATE TABLE "new_users""#));
        }

        /// MigrationGeneratorでのSQLite型変更SQL生成（カラム追加あり）
        /// 旧/新スキーマの列交差が動作することを確認
        #[test]
        fn test_sqlite_migration_generator_type_change_with_column_addition() {
            let generator = MigrationGenerator::new();

            // 旧スキーマ: id, age
            let mut old_schema = Schema::new("1.0".to_string());
            let mut old_table = Table::new("users".to_string());
            old_table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            old_table.columns.push(Column::new(
                "age".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            ));
            old_table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });
            old_schema.tables.insert("users".to_string(), old_table);

            // 新スキーマ: id, age (型変更), bio (新規追加)
            let mut new_schema = Schema::new("1.0".to_string());
            let mut new_table = Table::new("users".to_string());
            new_table.columns.push(Column::new(
                "id".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            new_table
                .columns
                .push(Column::new("age".to_string(), ColumnType::TEXT, true));
            new_table
                .columns
                .push(Column::new("bio".to_string(), ColumnType::TEXT, true)); // 新規追加
            new_table.constraints.push(Constraint::PRIMARY_KEY {
                columns: vec!["id".to_string()],
            });
            new_schema.tables.insert("users".to_string(), new_table);

            // Diffを作成（型変更のみ）
            let mut diff = SchemaDiff::new();
            let old_column = Column::new(
                "age".to_string(),
                ColumnType::INTEGER { precision: None },
                true,
            );
            let new_column = Column::new("age".to_string(), ColumnType::TEXT, true);
            let column_diff = ColumnDiff::new("age".to_string(), old_column, new_column);

            let mut table_diff = TableDiff::new("users".to_string());
            table_diff.modified_columns.push(column_diff);
            diff.modified_tables.push(table_diff);

            let result = generator.generate_up_sql_with_schemas(
                &diff,
                &old_schema,
                &new_schema,
                Dialect::SQLite,
            );

            assert!(result.is_ok());
            let (sql, _) = result.unwrap();

            // 列交差ロジックの検証
            // id, age は共通カラムとしてコピーされる
            // bio は新規追加カラムなのでNULLが入る
            assert!(sql.contains(r#"INSERT INTO "new_users""#));
            assert!(sql.contains("SELECT"));
            // id, ageが含まれる
            assert!(sql.contains("id"));
            assert!(sql.contains("age"));
            // bioカラムが新テーブルに追加される（NULLで）
            assert!(sql.contains("bio"));
            assert!(sql.contains("NULL"));
        }
    }

    // ==========================================
    // Task 9.4: 型変更検証テスト
    // ==========================================

    mod type_change_validation_tests {
        use super::*;

        /// 警告対象の型変更テスト: String → Numeric
        #[test]
        fn test_warning_string_to_numeric() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new(
                "price".to_string(),
                ColumnType::VARCHAR { length: 100 },
                false,
            );
            let new_column = Column::new(
                "price".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            );
            let column_diff = ColumnDiff::new("price".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("products", &[column_diff], &Dialect::PostgreSQL);

            // エラーではなく警告
            assert!(result.is_valid());
            assert_eq!(result.warning_count(), 1);
            assert!(result.warnings[0].message.contains("data loss"));
        }

        /// 警告対象の型変更テスト: Text → Boolean
        #[test]
        fn test_warning_text_to_boolean() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new("is_active".to_string(), ColumnType::TEXT, false);
            let new_column = Column::new("is_active".to_string(), ColumnType::BOOLEAN, false);
            let column_diff = ColumnDiff::new("is_active".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("users", &[column_diff], &Dialect::PostgreSQL);

            // エラーではなく警告
            assert!(result.is_valid());
            assert_eq!(result.warning_count(), 1);
        }

        /// エラー対象の型変更テスト: Json → Numeric
        #[test]
        fn test_error_json_to_numeric() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new("data".to_string(), ColumnType::JSONB, false);
            let new_column = Column::new(
                "data".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            );
            let column_diff = ColumnDiff::new("data".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("documents", &[column_diff], &Dialect::PostgreSQL);

            // エラー
            assert!(!result.is_valid());
            assert_eq!(result.error_count(), 1);
            assert!(result.errors[0].is_type_conversion());
        }

        /// エラー対象の型変更テスト: Numeric → DateTime
        #[test]
        fn test_error_numeric_to_datetime() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new(
                "created_at".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            );
            let new_column = Column::new(
                "created_at".to_string(),
                ColumnType::TIMESTAMP {
                    with_time_zone: None,
                },
                false,
            );
            let column_diff = ColumnDiff::new("created_at".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("events", &[column_diff], &Dialect::PostgreSQL);

            // エラー
            assert!(!result.is_valid());
            assert_eq!(result.error_count(), 1);
        }

        /// エラー対象の型変更テスト: Binary → Boolean
        #[test]
        fn test_error_binary_to_boolean() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new("data".to_string(), ColumnType::BLOB, false);
            let new_column = Column::new("data".to_string(), ColumnType::BOOLEAN, false);
            let column_diff = ColumnDiff::new("data".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("files", &[column_diff], &Dialect::PostgreSQL);

            // エラー
            assert!(!result.is_valid());
            assert_eq!(result.error_count(), 1);
        }

        /// 精度損失警告テスト: VARCHAR サイズ縮小
        #[test]
        fn test_precision_loss_varchar_shrink() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new(
                "email".to_string(),
                ColumnType::VARCHAR { length: 255 },
                false,
            );
            let new_column = Column::new(
                "email".to_string(),
                ColumnType::VARCHAR { length: 100 },
                false,
            );
            let column_diff = ColumnDiff::new("email".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("users", &[column_diff], &Dialect::PostgreSQL);

            // 警告
            assert!(result.is_valid());
            assert_eq!(result.warning_count(), 1);
            assert!(result.warnings[0].message.contains("truncation"));
        }

        /// 精度損失警告テスト: DECIMAL 精度縮小
        #[test]
        fn test_precision_loss_decimal_shrink() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new(
                "price".to_string(),
                ColumnType::DECIMAL {
                    precision: 10,
                    scale: 4,
                },
                false,
            );
            let new_column = Column::new(
                "price".to_string(),
                ColumnType::DECIMAL {
                    precision: 8,
                    scale: 2,
                },
                false,
            );
            let column_diff = ColumnDiff::new("price".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("products", &[column_diff], &Dialect::PostgreSQL);

            // 警告
            assert!(result.is_valid());
            assert_eq!(result.warning_count(), 1);
            assert!(result.warnings[0].message.contains("precision loss"));
        }

        /// 精度損失警告テスト: BIGINT → INTEGER
        #[test]
        fn test_precision_loss_bigint_to_integer() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new(
                "big_id".to_string(),
                ColumnType::INTEGER { precision: Some(8) }, // BIGINT
                false,
            );
            let new_column = Column::new(
                "big_id".to_string(),
                ColumnType::INTEGER { precision: Some(4) }, // INTEGER
                false,
            );
            let column_diff = ColumnDiff::new("big_id".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("large_ids", &[column_diff], &Dialect::PostgreSQL);

            // 警告
            assert!(result.is_valid());
            assert_eq!(result.warning_count(), 1);
            assert!(result.warnings[0].message.contains("overflow"));
        }

        /// 安全な型変換テスト: Numeric → String
        #[test]
        fn test_safe_numeric_to_string() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new(
                "age".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            );
            let new_column = Column::new("age".to_string(), ColumnType::TEXT, false);
            let column_diff = ColumnDiff::new("age".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("users", &[column_diff], &Dialect::PostgreSQL);

            // 安全なので警告もエラーもなし
            assert!(result.is_valid());
            assert_eq!(result.warning_count(), 0);
        }

        /// 安全な型変換テスト: Boolean → Numeric
        #[test]
        fn test_safe_boolean_to_numeric() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new("is_active".to_string(), ColumnType::BOOLEAN, false);
            let new_column = Column::new(
                "is_active".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            );
            let column_diff = ColumnDiff::new("is_active".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("users", &[column_diff], &Dialect::PostgreSQL);

            // 安全なので警告もエラーもなし
            assert!(result.is_valid());
            assert_eq!(result.warning_count(), 0);
        }

        /// 安全な型変換テスト: UUID → String
        #[test]
        fn test_safe_uuid_to_string() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new("user_id".to_string(), ColumnType::UUID, false);
            let new_column = Column::new(
                "user_id".to_string(),
                ColumnType::VARCHAR { length: 36 },
                false,
            );
            let column_diff = ColumnDiff::new("user_id".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("users", &[column_diff], &Dialect::PostgreSQL);

            // 安全なので警告もエラーもなし
            assert!(result.is_valid());
            assert_eq!(result.warning_count(), 0);
        }

        /// 位置情報付きエラーの確認
        #[test]
        fn test_error_location_info() {
            let validator = TypeChangeValidator::new();

            let old_column = Column::new("data".to_string(), ColumnType::JSONB, false);
            let new_column = Column::new(
                "data".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            );
            let column_diff = ColumnDiff::new("data".to_string(), old_column, new_column);

            let result =
                validator.validate_type_changes("documents", &[column_diff], &Dialect::PostgreSQL);

            assert!(!result.is_valid());
            let error = &result.errors[0];
            let location = error.location().expect("Location should be present");
            assert_eq!(location.table.as_deref(), Some("documents"));
            assert_eq!(location.column.as_deref(), Some("data"));
        }

        /// 複数の型変更を含む検証（エラーと警告の混在）
        #[test]
        fn test_multiple_changes_mixed_results() {
            let validator = TypeChangeValidator::new();

            let diffs = vec![
                // 安全な変換
                ColumnDiff::new(
                    "col1".to_string(),
                    Column::new(
                        "col1".to_string(),
                        ColumnType::INTEGER { precision: None },
                        false,
                    ),
                    Column::new("col1".to_string(), ColumnType::TEXT, false),
                ),
                // 警告（データ損失リスク）
                ColumnDiff::new(
                    "col2".to_string(),
                    Column::new("col2".to_string(), ColumnType::TEXT, false),
                    Column::new(
                        "col2".to_string(),
                        ColumnType::INTEGER { precision: None },
                        false,
                    ),
                ),
                // エラー（互換性なし）
                ColumnDiff::new(
                    "col3".to_string(),
                    Column::new("col3".to_string(), ColumnType::JSONB, false),
                    Column::new(
                        "col3".to_string(),
                        ColumnType::INTEGER { precision: None },
                        false,
                    ),
                ),
            ];

            let result =
                validator.validate_type_changes("test_table", &diffs, &Dialect::PostgreSQL);

            // 1つのエラーと1つの警告
            assert!(!result.is_valid());
            assert_eq!(result.error_count(), 1);
            assert_eq!(result.warning_count(), 1);
        }

        /// MigrationGeneratorでの検証エラーによる生成中止
        #[test]
        fn test_migration_generator_aborts_on_validation_error() {
            let generator = MigrationGenerator::new();

            // 互換性のない型変更を含むスキーマ
            let mut old_schema = Schema::new("1.0".to_string());
            let mut old_table = Table::new("documents".to_string());
            old_table
                .columns
                .push(Column::new("data".to_string(), ColumnType::JSONB, false));
            old_schema.tables.insert("documents".to_string(), old_table);

            let mut new_schema = Schema::new("1.0".to_string());
            let mut new_table = Table::new("documents".to_string());
            new_table.columns.push(Column::new(
                "data".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            ));
            new_schema.tables.insert("documents".to_string(), new_table);

            let mut diff = SchemaDiff::new();
            let old_column = Column::new("data".to_string(), ColumnType::JSONB, false);
            let new_column = Column::new(
                "data".to_string(),
                ColumnType::INTEGER { precision: None },
                false,
            );
            let column_diff = ColumnDiff::new("data".to_string(), old_column, new_column);

            let mut table_diff = TableDiff::new("documents".to_string());
            table_diff.modified_columns.push(column_diff);
            diff.modified_tables.push(table_diff);

            let result = generator.generate_up_sql_with_schemas(
                &diff,
                &old_schema,
                &new_schema,
                Dialect::PostgreSQL,
            );

            // 検証エラーによりErrが返される
            assert!(result.is_err());
            let error = result.unwrap_err();
            assert!(error.contains("Type change validation failed"));
        }
    }
}
