// インデックス・制約・クリーンアップ関連パイプラインステージ
//
// インデックス作成、制約追加、テーブル/型の削除を処理するステージ。

use crate::adapters::sql_generator::SqlGenerator;
use crate::core::config::Dialect;

use super::{MigrationPipeline, PipelineStageError};

impl<'a> MigrationPipeline<'a> {
    /// ステージ4: index_statements - CREATE INDEX
    pub(super) fn stage_index_statements(&self, generator: &dyn SqlGenerator) -> Vec<String> {
        let mut statements = Vec::new();

        for table_diff in &self.diff.modified_tables {
            for index in &table_diff.added_indexes {
                let table = crate::core::schema::Table::new(table_diff.table_name.clone());
                statements.push(generator.generate_create_index(&table, index));
            }
        }

        statements
    }

    /// ステージ5: constraint_statements - 制約追加
    pub(super) fn stage_constraint_statements(&self, generator: &dyn SqlGenerator) -> Vec<String> {
        let mut statements = Vec::new();

        // 既存テーブルへの制約追加を処理
        for table_diff in &self.diff.modified_tables {
            for constraint in &table_diff.added_constraints {
                // FOREIGN KEY制約のみ処理（SQLiteは空文字列を返す）
                let sql = generator
                    .generate_add_constraint_for_existing_table(&table_diff.table_name, constraint);
                if !sql.is_empty() {
                    statements.push(sql);
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

    #[test]
    fn test_pipeline_down_drops_added_foreign_key_constraint() {
        // Down migrationで追加された外部キー制約が削除されること
        let mut diff = SchemaDiff::new();

        let mut table_diff = TableDiff::new("posts".to_string());
        table_diff.added_constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
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
}
