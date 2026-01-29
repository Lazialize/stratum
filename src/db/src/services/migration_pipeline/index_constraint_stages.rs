// インデックス・制約・クリーンアップ関連パイプラインステージ
//
// インデックス作成、制約追加、テーブル/型の削除を処理するステージ。

use crate::adapters::sql_generator::SqlGenerator;
use crate::core::config::Dialect;

use super::{MigrationPipeline, PipelineStageError};

impl<'a> MigrationPipeline<'a> {
    /// ステージ4: index_statements - CREATE INDEX / DROP INDEX
    ///
    /// 追加されたインデックスのCREATE INDEX文と、
    /// 削除されたインデックスのDROP INDEX文を生成します。
    /// 変更されたインデックスはDROP後にCREATEします。
    pub(super) fn stage_index_statements(&self, generator: &dyn SqlGenerator) -> Vec<String> {
        let mut statements = Vec::new();

        for table_diff in &self.diff.modified_tables {
            // 削除されたインデックスのDROP INDEX
            for index in &table_diff.removed_indexes {
                statements.push(generator.generate_drop_index(&table_diff.table_name, index));
            }

            // 変更されたインデックス: DROP後にCREATE
            for index_diff in &table_diff.modified_indexes {
                statements.push(
                    generator
                        .generate_drop_index(&table_diff.table_name, &index_diff.old_index.name),
                );
                let table = crate::core::schema::Table::new(table_diff.table_name.clone());
                statements.push(generator.generate_create_index(&table, &index_diff.new_index));
            }

            // 追加されたインデックスのCREATE INDEX
            for index in &table_diff.added_indexes {
                let table = crate::core::schema::Table::new(table_diff.table_name.clone());
                statements.push(generator.generate_create_index(&table, index));
            }
        }

        statements
    }

    /// ステージ5: constraint_statements - 制約追加・削除
    pub(super) fn stage_constraint_statements(&self, generator: &dyn SqlGenerator) -> Vec<String> {
        let mut statements = Vec::new();

        for table_diff in &self.diff.modified_tables {
            if matches!(self.dialect, Dialect::SQLite) {
                // SQLite: 制約変更またはnullable/default変更がある場合はテーブル再作成で処理
                let has_constraint_changes = !table_diff.added_constraints.is_empty()
                    || !table_diff.removed_constraints.is_empty();
                let has_nullable_or_default_changes = table_diff
                    .modified_columns
                    .iter()
                    .any(|cd| self.has_nullable_or_default_change(cd));

                if has_constraint_changes || has_nullable_or_default_changes {
                    // カラム型変更がある場合はステージ3で再作成済み → スキップ
                    let has_type_change = table_diff
                        .modified_columns
                        .iter()
                        .any(|cd| self.has_type_change(cd));
                    let has_renamed_type_change = table_diff
                        .renamed_columns
                        .iter()
                        .any(|rc| self.has_type_change_in_renamed(rc));

                    if !has_type_change && !has_renamed_type_change {
                        // テーブル再作成で制約変更を適用
                        if let Some(new_schema) = self.new_schema {
                            if let Some(new_table) = new_schema.tables.get(&table_diff.table_name) {
                                let old_table = self
                                    .old_schema
                                    .and_then(|s| s.tables.get(&table_diff.table_name));
                                let recreator = crate::adapters::sql_generator::sqlite_table_recreator::SqliteTableRecreator::new();
                                let recreation_stmts = recreator
                                    .generate_table_recreation_with_old_table(new_table, old_table);
                                statements.extend(recreation_stmts);
                            }
                        }
                    }
                }
            } else {
                // PostgreSQL・MySQL: ALTER TABLE で処理
                // 削除された制約のDROP
                for constraint in &table_diff.removed_constraints {
                    let sql = generator.generate_drop_constraint_for_existing_table(
                        &table_diff.table_name,
                        constraint,
                    );
                    if !sql.is_empty() {
                        statements.push(sql);
                    }
                }

                // 追加された制約のADD
                for constraint in &table_diff.added_constraints {
                    let sql = generator.generate_add_constraint_for_existing_table(
                        &table_diff.table_name,
                        constraint,
                    );
                    if !sql.is_empty() {
                        statements.push(sql);
                    }
                }
            }
        }

        statements
    }

    /// ステージ6: cleanup_statements - DROP TABLE/TYPE
    pub(super) fn stage_cleanup_statements(
        &self,
        generator: &dyn SqlGenerator,
    ) -> Result<Vec<String>, PipelineStageError> {
        let mut statements = Vec::new();

        // 削除されたテーブルのDROP TABLE文を生成
        for table_name in &self.diff.removed_tables {
            statements.push(generator.generate_drop_table(table_name));
        }

        // ENUM削除（PostgreSQL）
        if matches!(self.dialect, Dialect::PostgreSQL) {
            for enum_name in &self.diff.removed_enums {
                statements.extend(generator.generate_drop_enum_type(enum_name));
            }
        }

        Ok(statements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::Constraint;
    use crate::core::schema_diff::{SchemaDiff, TableDiff};

    // ==========================================
    // 外部キー制約追加のテスト
    // ==========================================

    #[test]
    fn test_pipeline_add_foreign_key_constraint_to_existing_table() {
        // 既存テーブルへの外部キー制約追加のテスト
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("posts".to_string());
        table_diff.added_constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ALTER TABLE "posts" ADD CONSTRAINT"#),
            "Expected ALTER TABLE ADD CONSTRAINT in: {}",
            sql
        );
        assert!(
            sql.contains(r#"FOREIGN KEY ("user_id")"#),
            "Expected FOREIGN KEY in: {}",
            sql
        );
        assert!(
            sql.contains(r#"REFERENCES "users" ("id")"#),
            "Expected REFERENCES in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_add_foreign_key_constraint_mysql() {
        // MySQLでの外部キー制約追加テスト
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("posts".to_string());
        table_diff.added_constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains("ALTER TABLE `posts` ADD CONSTRAINT"),
            "Expected ALTER TABLE ADD CONSTRAINT in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_add_foreign_key_constraint_sqlite_not_supported() {
        // SQLiteでは外部キー制約追加はサポートされない
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("posts".to_string());
        table_diff.added_constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::SQLite);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // SQLiteではALTER TABLE ADD CONSTRAINTがサポートされないため、空のSQL
        assert!(
            !sql.contains("ALTER TABLE"),
            "SQLite should not generate ALTER TABLE for constraint: {}",
            sql
        );
    }

    // ==========================================
    // Down Migration テスト (制約関連)
    // ==========================================

    // ==========================================
    // UNIQUE・CHECK制約追加のテスト（UP方向）
    // ==========================================

    #[test]
    fn test_pipeline_add_unique_constraint_to_existing_table_postgres() {
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ADD CONSTRAINT "uq_users_email" UNIQUE ("email")"#),
            "Expected UNIQUE constraint SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_add_check_constraint_to_existing_table_postgres() {
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("products".to_string());
        table_diff.added_constraints.push(Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ADD CONSTRAINT "ck_products_price" CHECK (price >= 0)"#),
            "Expected CHECK constraint SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_add_unique_constraint_mysql() {
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains("ADD CONSTRAINT `uq_users_email` UNIQUE (`email`)"),
            "Expected UNIQUE constraint SQL in: {}",
            sql
        );
    }

    // ==========================================
    // 制約削除のテスト（UP方向）
    // ==========================================

    #[test]
    fn test_pipeline_remove_unique_constraint_postgres() {
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.removed_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"DROP CONSTRAINT IF EXISTS "uq_users_email""#),
            "Expected DROP CONSTRAINT SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_remove_check_constraint_postgres() {
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("products".to_string());
        table_diff.removed_constraints.push(Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"DROP CONSTRAINT IF EXISTS "ck_products_price""#),
            "Expected DROP CONSTRAINT SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_remove_unique_constraint_mysql() {
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.removed_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains("DROP INDEX `uq_users_email`"),
            "Expected DROP INDEX SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_remove_check_constraint_mysql() {
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("products".to_string());
        table_diff.removed_constraints.push(Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains("DROP CHECK `ck_products_price`"),
            "Expected DROP CHECK SQL in: {}",
            sql
        );
    }

    // ==========================================
    // DOWN方向のテスト
    // ==========================================

    #[test]
    fn test_pipeline_down_drops_added_unique_constraint() {
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"DROP CONSTRAINT IF EXISTS "uq_users_email""#),
            "Expected DROP CONSTRAINT in down SQL: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_down_restores_removed_unique_constraint() {
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.removed_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ADD CONSTRAINT "uq_users_email" UNIQUE ("email")"#),
            "Expected ADD CONSTRAINT in down SQL: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_down_restores_removed_check_constraint() {
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("products".to_string());
        table_diff.removed_constraints.push(Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ADD CONSTRAINT "ck_products_price" CHECK (price >= 0)"#),
            "Expected ADD CONSTRAINT in down SQL: {}",
            sql
        );
    }

    // ==========================================
    // SQLite制約テスト
    // ==========================================

    #[test]
    fn test_pipeline_sqlite_constraint_change_table_recreation() {
        use crate::core::schema::{Column, ColumnType, Schema, Table};

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        // SQLiteはテーブル再作成が必要
        let mut new_schema = Schema::new("1.0".to_string());
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.columns.push(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        new_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        new_table.constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        new_schema.tables.insert("users".to_string(), new_table);

        let mut old_schema = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
        old_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        old_table.columns.push(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        old_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        old_schema.tables.insert("users".to_string(), old_table);

        let pipeline =
            MigrationPipeline::new(&diff, Dialect::SQLite).with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // SQLiteではテーブル再作成パターンを使用
        assert!(
            sql.contains("PRAGMA foreign_keys=off"),
            "Expected table recreation in: {}",
            sql
        );
        assert!(
            sql.contains(r#"CREATE TABLE "_stratum_tmp_recreate_users""#),
            "Expected new table creation in: {}",
            sql
        );
        // 新しいテーブルにUNIQUE制約が含まれること
        assert!(
            sql.contains("UNIQUE"),
            "Expected UNIQUE constraint in recreated table: {}",
            sql
        );
    }

    // ==========================================
    // タスク5.1: 3方言のパイプラインUP/DOWN統合テスト
    // ==========================================

    #[test]
    fn test_pipeline_add_column_and_unique_constraint_combined_postgres() {
        use crate::core::schema::{Column, ColumnType};

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.added_columns.push(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // カラム追加とUNIQUE制約追加の両方が含まれる
        assert!(
            sql.contains(r#"ALTER TABLE "users" ADD COLUMN"#),
            "Expected ADD COLUMN in: {}",
            sql
        );
        assert!(
            sql.contains(r#"ADD CONSTRAINT "uq_users_email" UNIQUE"#),
            "Expected ADD CONSTRAINT in: {}",
            sql
        );

        // カラム追加がUNIQUE制約追加より先
        let add_col_pos = sql.find("ADD COLUMN").unwrap();
        let add_constr_pos = sql.find("ADD CONSTRAINT").unwrap();
        assert!(add_col_pos < add_constr_pos);
    }

    #[test]
    fn test_pipeline_add_column_and_unique_constraint_combined_mysql() {
        use crate::core::schema::{Column, ColumnType};

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.added_columns.push(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("ADD COLUMN"));
        assert!(sql.contains("ADD CONSTRAINT `uq_users_email` UNIQUE"));
    }

    #[test]
    fn test_pipeline_constraint_up_down_roundtrip_postgres() {
        // UP: UNIQUE追加 → DOWN: UNIQUE削除
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);

        let (up_sql, _) = pipeline.generate_up().unwrap();
        let (down_sql, _) = pipeline.generate_down().unwrap();

        // UPはADD CONSTRAINT
        assert!(up_sql.contains("ADD CONSTRAINT"));
        // DOWNはDROP CONSTRAINT
        assert!(down_sql.contains("DROP CONSTRAINT"));
    }

    #[test]
    fn test_pipeline_constraint_removal_up_down_roundtrip_mysql() {
        // UP: UNIQUE削除 → DOWN: UNIQUE復元
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.removed_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);

        let (up_sql, _) = pipeline.generate_up().unwrap();
        let (down_sql, _) = pipeline.generate_down().unwrap();

        // UPはDROP INDEX
        assert!(up_sql.contains("DROP INDEX"));
        // DOWNはADD CONSTRAINT
        assert!(down_sql.contains("ADD CONSTRAINT"));
    }

    #[test]
    fn test_pipeline_check_constraint_3_dialects_up() {
        // 3方言でのCHECK制約追加
        let create_diff = || {
            let mut diff = SchemaDiff::new();
            let mut table_diff = TableDiff::new("products".to_string());
            table_diff.added_constraints.push(Constraint::CHECK {
                columns: vec!["price".to_string()],
                check_expression: "price >= 0".to_string(),
            });
            diff.modified_tables.push(table_diff);
            diff
        };

        // PostgreSQL
        let diff = create_diff();
        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let (sql, _) = pipeline.generate_up().unwrap();
        assert!(sql.contains("CHECK (price >= 0)"));

        // MySQL
        let diff = create_diff();
        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let (sql, _) = pipeline.generate_up().unwrap();
        assert!(sql.contains("CHECK (price >= 0)"));

        // SQLite: スキーマなしの場合は空（テーブル再作成にはスキーマが必要）
        let diff = create_diff();
        let pipeline = MigrationPipeline::new(&diff, Dialect::SQLite);
        let (sql, _) = pipeline.generate_up().unwrap();
        // スキーマなしの場合はSQLite用テーブル再作成は発生しない
        assert!(sql.is_empty() || !sql.contains("ALTER TABLE"));
    }

    // ==========================================
    // タスク5.2: SQLiteエッジケース・混合変更テスト
    // ==========================================

    #[test]
    fn test_pipeline_sqlite_multiple_constraint_changes_single_recreation() {
        use crate::core::schema::{Column, ColumnType, Schema, Table};

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("products".to_string());
        // UNIQUE追加 + CHECK追加の同時変更
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec!["name".to_string()],
        });
        table_diff.added_constraints.push(Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        });
        diff.modified_tables.push(table_diff);

        let mut new_schema = Schema::new("1.0".to_string());
        let mut new_table = Table::new("products".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        new_table.columns.push(Column::new(
            "price".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        new_table.constraints.push(Constraint::UNIQUE {
            columns: vec!["name".to_string()],
        });
        new_table.constraints.push(Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        });
        new_schema.tables.insert("products".to_string(), new_table);

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
        old_table.columns.push(Column::new(
            "price".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        old_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        old_schema.tables.insert("products".to_string(), old_table);

        let pipeline =
            MigrationPipeline::new(&diff, Dialect::SQLite).with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // テーブル再作成は1回だけ
        let pragma_count = sql.matches("PRAGMA foreign_keys=off").count();
        assert_eq!(
            pragma_count, 1,
            "Expected exactly 1 table recreation, got {} in: {}",
            pragma_count, sql
        );

        // 両方の制約が含まれる
        assert!(sql.contains("UNIQUE"), "Expected UNIQUE in: {}", sql);
        assert!(sql.contains("CHECK"), "Expected CHECK in: {}", sql);
    }

    #[test]
    fn test_pipeline_sqlite_type_change_and_constraint_change_skips_stage5() {
        use crate::core::schema::{Column, ColumnType, Schema, Table};
        use crate::core::schema_diff::{ColumnChange, ColumnDiff};

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());

        // 型変更（ステージ3で再作成される）
        let old_col = Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            true,
        );
        let new_col = Column::new("age".to_string(), ColumnType::VARCHAR { length: 50 }, true);
        let column_diff = ColumnDiff {
            column_name: "age".to_string(),
            old_column: old_col,
            new_column: new_col,
            changes: vec![ColumnChange::TypeChanged {
                old_type: "INTEGER".to_string(),
                new_type: "VARCHAR(50)".to_string(),
            }],
        };
        table_diff.modified_columns.push(column_diff);

        // 制約変更（ステージ3で再作成済みのためスキップされるべき）
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

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
        new_table.columns.push(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        new_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        new_table.constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        new_schema.tables.insert("users".to_string(), new_table);

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
        old_table.columns.push(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        old_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        old_schema.tables.insert("users".to_string(), old_table);

        let pipeline =
            MigrationPipeline::new(&diff, Dialect::SQLite).with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // テーブル再作成は1回だけ（ステージ3の型変更由来のみ、ステージ5はスキップ）
        let pragma_count = sql.matches("PRAGMA foreign_keys=off").count();
        assert_eq!(
            pragma_count, 1,
            "Expected exactly 1 table recreation (from type change), got {} in: {}",
            pragma_count, sql
        );
    }

    #[test]
    fn test_pipeline_sqlite_down_constraint_table_recreation() {
        use crate::core::schema::{Column, ColumnType, Schema, Table};

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        diff.modified_tables.push(table_diff);

        // new_schema: UNIQUE制約あり
        let mut new_schema = Schema::new("1.0".to_string());
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.columns.push(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        new_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        new_table.constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        new_schema.tables.insert("users".to_string(), new_table);

        // old_schema: UNIQUE制約なし
        let mut old_schema = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
        old_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        old_table.columns.push(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        old_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        old_schema.tables.insert("users".to_string(), old_table);

        let pipeline =
            MigrationPipeline::new(&diff, Dialect::SQLite).with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // DOWN方向ではold_schemaに戻す（UNIQUEなし）テーブル再作成
        assert!(
            sql.contains("PRAGMA foreign_keys=off"),
            "Expected table recreation in down SQL: {}",
            sql
        );
        // 旧テーブルにはUNIQUE制約がないので、再作成されたテーブルにはUNIQUEがないはず
        // (新テーブル作成SQL内にUNIQUEが含まれない)
        let create_stmt_start = sql.find(r#"CREATE TABLE "_stratum_tmp_recreate_users""#);
        if let Some(start) = create_stmt_start {
            let create_stmt = &sql[start..sql[start..].find(';').map_or(sql.len(), |p| start + p)];
            assert!(
                !create_stmt.contains("UNIQUE"),
                "DOWN table recreation should NOT have UNIQUE: {}",
                create_stmt
            );
        }
    }

    #[test]
    fn test_pipeline_long_constraint_name_pipeline_works() {
        // 63文字超のハッシュ切り詰め制約名でもパイプラインが正常動作
        let mut diff = SchemaDiff::new();
        let mut table_diff =
            TableDiff::new("very_long_table_name_with_many_characters".to_string());
        table_diff.added_constraints.push(Constraint::UNIQUE {
            columns: vec![
                "organization_id".to_string(),
                "department_id".to_string(),
                "another_long_column".to_string(),
            ],
        });
        diff.modified_tables.push(table_diff);

        // PostgreSQL
        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let (up_sql, _) = pipeline.generate_up().unwrap();
        let (down_sql, _) = pipeline.generate_down().unwrap();
        assert!(up_sql.contains("ADD CONSTRAINT"));
        assert!(down_sql.contains("DROP CONSTRAINT"));

        // MySQL
        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let (up_sql, _) = pipeline.generate_up().unwrap();
        let (down_sql, _) = pipeline.generate_down().unwrap();
        assert!(up_sql.contains("ADD CONSTRAINT"));
        assert!(down_sql.contains("DROP INDEX"));
    }

    #[test]
    fn test_pipeline_down_drops_added_foreign_key_constraint() {
        // Down migrationで追加された外部キー制約が削除されること
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("posts".to_string());
        table_diff.added_constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains("DROP CONSTRAINT"),
            "Expected DROP CONSTRAINT in down SQL: {}",
            sql
        );
        assert!(
            sql.contains("fk_posts_user_id_users"),
            "Expected constraint name in down SQL: {}",
            sql
        );
    }

    // ==========================================
    // P0-1.3: removed_indexes の DROP INDEX 生成テスト
    // ==========================================

    #[test]
    fn test_pipeline_remove_index_generates_drop_index_postgres() {
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff
            .removed_indexes
            .push("idx_users_email".to_string());
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"DROP INDEX "idx_users_email""#),
            "Expected DROP INDEX in UP SQL: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_remove_index_generates_drop_index_mysql() {
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff
            .removed_indexes
            .push("idx_users_email".to_string());
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains("DROP INDEX `idx_users_email` ON `users`"),
            "Expected DROP INDEX with ON clause in UP SQL: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_remove_index_generates_drop_index_sqlite() {
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff
            .removed_indexes
            .push("idx_users_email".to_string());
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::SQLite);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"DROP INDEX "idx_users_email""#),
            "Expected DROP INDEX in UP SQL: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_add_and_remove_indexes_combined() {
        use crate::core::schema::Index;

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        // 削除（インデックス名のみ）
        table_diff.removed_indexes.push("idx_users_old".to_string());
        // 追加（完全なIndex構造体）
        table_diff.added_indexes.push(Index {
            name: "idx_users_new".to_string(),
            columns: vec!["new_column".to_string()],
            unique: true,
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // 削除が先、追加が後
        assert!(
            sql.contains(r#"DROP INDEX "idx_users_old""#),
            "Expected DROP INDEX in UP SQL: {}",
            sql
        );
        assert!(
            sql.contains(r#"CREATE UNIQUE INDEX "idx_users_new""#),
            "Expected CREATE INDEX in UP SQL: {}",
            sql
        );
    }

    // ==========================================
    // インデックス変更のテスト
    // ==========================================

    #[test]
    fn test_pipeline_modify_index_generates_drop_and_create_postgres() {
        use crate::core::schema::Index;
        use crate::core::schema_diff::IndexDiff;

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_indexes.push(IndexDiff {
            index_name: "idx_users_email".to_string(),
            old_index: Index {
                name: "idx_users_email".to_string(),
                columns: vec!["email".to_string()],
                unique: false,
            },
            new_index: Index {
                name: "idx_users_email".to_string(),
                columns: vec!["email".to_string(), "name".to_string()],
                unique: true,
            },
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // DROP INDEX が先
        assert!(
            sql.contains(r#"DROP INDEX "idx_users_email""#),
            "Expected DROP INDEX in UP SQL: {}",
            sql
        );
        // CREATE INDEX が後
        assert!(
            sql.contains(r#"CREATE UNIQUE INDEX "idx_users_email""#),
            "Expected CREATE UNIQUE INDEX in UP SQL: {}",
            sql
        );
        // DROP が CREATE より先に来る
        let drop_pos = sql.find("DROP INDEX").unwrap();
        let create_pos = sql.find("CREATE UNIQUE INDEX").unwrap();
        assert!(
            drop_pos < create_pos,
            "DROP INDEX should come before CREATE INDEX"
        );
    }

    #[test]
    fn test_pipeline_modify_index_mysql() {
        use crate::core::schema::Index;
        use crate::core::schema_diff::IndexDiff;

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_indexes.push(IndexDiff {
            index_name: "idx_users_email".to_string(),
            old_index: Index {
                name: "idx_users_email".to_string(),
                columns: vec!["email".to_string()],
                unique: false,
            },
            new_index: Index {
                name: "idx_users_email".to_string(),
                columns: vec!["email".to_string()],
                unique: true, // unique に変更
            },
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains("DROP INDEX"),
            "Expected DROP INDEX in UP SQL: {}",
            sql
        );
        assert!(
            sql.contains("CREATE UNIQUE INDEX"),
            "Expected CREATE UNIQUE INDEX in UP SQL: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_modify_index_sqlite() {
        use crate::core::schema::Index;
        use crate::core::schema_diff::IndexDiff;

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_indexes.push(IndexDiff {
            index_name: "idx_users_email".to_string(),
            old_index: Index {
                name: "idx_users_email".to_string(),
                columns: vec!["email".to_string()],
                unique: false,
            },
            new_index: Index {
                name: "idx_users_email".to_string(),
                columns: vec!["email".to_string(), "name".to_string()],
                unique: false,
            },
        });
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::SQLite);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"DROP INDEX "idx_users_email""#),
            "Expected DROP INDEX in UP SQL: {}",
            sql
        );
        assert!(
            sql.contains(r#"CREATE INDEX "idx_users_email""#),
            "Expected CREATE INDEX in UP SQL: {}",
            sql
        );
    }
}
