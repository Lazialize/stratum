// テーブル関連パイプラインステージ
//
// テーブルの作成・変更・型変更検証を処理するステージ。

use crate::adapters::sql_generator::{MigrationDirection, SqlGenerator};
use crate::core::config::Dialect;
use crate::core::error::ValidationResult;
use crate::core::schema_diff::ColumnChange;
use crate::services::type_change_validator::TypeChangeValidator;

use super::{MigrationPipeline, PipelineStageError};

impl<'a> MigrationPipeline<'a> {
    /// ステージ1: prepare - 事前検証
    pub(super) fn stage_prepare(&self) -> Result<ValidationResult, PipelineStageError> {
        let mut total_validation_result = ValidationResult::new();

        // スキーマ情報がある場合は型変更の検証を行う
        if self.old_schema.is_some() && self.new_schema.is_some() {
            let validator = TypeChangeValidator::new();

            for table_diff in &self.diff.modified_tables {
                let validation = validator.validate_type_changes(
                    &table_diff.table_name,
                    &table_diff.modified_columns,
                    &self.dialect,
                );
                total_validation_result.merge(validation);
            }
        }

        Ok(total_validation_result)
    }

    /// ステージ3: table_statements - CREATE/ALTER TABLE
    pub(super) fn stage_table_statements(
        &self,
        generator: &dyn SqlGenerator,
    ) -> Result<Vec<String>, PipelineStageError> {
        let mut statements = Vec::new();

        // リネームされたテーブルの処理（最初に実行）
        for renamed_table in &self.diff.renamed_tables {
            statements.push(
                generator
                    .generate_rename_table(&renamed_table.old_name, &renamed_table.new_table.name),
            );
        }

        // 外部キー依存関係を考慮してテーブルをソート
        let sorted_tables = self.diff.sort_added_tables_by_dependency().map_err(|e| {
            PipelineStageError::CircularDependency {
                message: e.to_string(),
            }
        })?;

        // 追加されたテーブルのCREATE TABLE文を生成
        for table in &sorted_tables {
            statements.push(generator.generate_create_table(table));

            // インデックスの作成
            for index in &table.indexes {
                statements.push(generator.generate_create_index(table, index));
            }

            // FOREIGN KEY制約の追加（SQLite以外）
            if !matches!(self.dialect, Dialect::SQLite) {
                for (i, constraint) in table.constraints.iter().enumerate() {
                    if matches!(
                        constraint,
                        crate::core::schema::Constraint::FOREIGN_KEY { .. }
                    ) {
                        let alter_sql = generator.generate_alter_table_add_constraint(table, i);
                        if !alter_sql.is_empty() {
                            statements.push(alter_sql);
                        }
                    }
                }
            }
        }

        // 変更されたテーブルの処理
        for table_diff in &self.diff.modified_tables {
            // カラムの追加
            for column in &table_diff.added_columns {
                statements.push(generator.generate_add_column(&table_diff.table_name, column));
            }

            // カラムの削除
            for column_name in &table_diff.removed_columns {
                statements
                    .push(generator.generate_drop_column(&table_diff.table_name, column_name));
            }

            // リネームカラムの処理（Up方向: リネーム → 型変更の順序）
            for renamed_column in &table_diff.renamed_columns {
                if let Some(new_schema) = self.new_schema {
                    if let Some(table) = new_schema.tables.get(&table_diff.table_name) {
                        // まずリネームSQLを生成
                        let rename_statements = generator.generate_rename_column(
                            table,
                            renamed_column,
                            MigrationDirection::Up,
                        );
                        statements.extend(rename_statements);

                        // リネームと同時に型変更がある場合は、型変更SQLも生成
                        if self.has_type_change_in_renamed(renamed_column) {
                            // リネーム後の新しいカラム名で型変更SQLを生成
                            let column_diff = crate::core::schema_diff::ColumnDiff {
                                column_name: renamed_column.new_column.name.clone(),
                                old_column: renamed_column.old_column.clone(),
                                new_column: renamed_column.new_column.clone(),
                                changes: renamed_column.changes.clone(),
                            };
                            let old_table = self
                                .old_schema
                                .and_then(|s| s.tables.get(&table_diff.table_name));
                            let alter_statements = generator
                                .generate_alter_column_type_with_old_table(
                                    table,
                                    old_table,
                                    &column_diff,
                                    MigrationDirection::Up,
                                );
                            statements.extend(alter_statements);
                        }
                    }
                }
            }

            // 型変更の処理（リネーム以外のカラム）
            for column_diff in &table_diff.modified_columns {
                if self.has_type_change(column_diff) {
                    if let Some(new_schema) = self.new_schema {
                        if let Some(table) = new_schema.tables.get(&table_diff.table_name) {
                            let old_table = self
                                .old_schema
                                .and_then(|s| s.tables.get(&table_diff.table_name));
                            let alter_statements = generator
                                .generate_alter_column_type_with_old_table(
                                    table,
                                    old_table,
                                    column_diff,
                                    MigrationDirection::Up,
                                );
                            statements.extend(alter_statements);
                        }
                    }
                }
            }

            // nullable/default変更の処理（型変更がないカラム、SQLite以外）
            if !matches!(self.dialect, Dialect::SQLite) {
                for column_diff in &table_diff.modified_columns {
                    if !self.has_type_change(column_diff)
                        && self.has_nullable_or_default_change(column_diff)
                    {
                        // new_columnの情報を使ってSQL生成
                        let target_column = &column_diff.new_column;
                        for change in &column_diff.changes {
                            match change {
                                ColumnChange::NullableChanged { new_nullable, .. } => {
                                    statements.extend(generator.generate_alter_column_nullable(
                                        &table_diff.table_name,
                                        target_column,
                                        *new_nullable,
                                    ));
                                }
                                ColumnChange::DefaultValueChanged { new_default, .. } => {
                                    statements.extend(generator.generate_alter_column_default(
                                        &table_diff.table_name,
                                        target_column,
                                        new_default.as_deref(),
                                    ));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Ok(statements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{Column, ColumnType, Constraint, Schema, Table};
    use crate::core::schema_diff::{
        ColumnChange, ColumnDiff, RenamedColumn, SchemaDiff, TableDiff,
    };

    // ==========================================
    // テーブル関連テスト
    // ==========================================

    #[test]
    fn test_pipeline_create_table() {
        let mut diff = SchemaDiff::new();
        let mut table = Table::new("users".to_string());
        table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            true,
        ));
        diff.added_tables.push(table);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains(r#"CREATE TABLE "users""#));
    }

    #[test]
    fn test_pipeline_drop_table() {
        let mut diff = SchemaDiff::new();
        diff.removed_tables.push("users".to_string());

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains(r#"DROP TABLE "users""#));
    }

    // ==========================================
    // 型変更関連テスト
    // ==========================================

    fn create_test_schemas_for_type_change() -> (Schema, Schema) {
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

        (old_schema, new_schema)
    }

    fn create_diff_with_type_change() -> SchemaDiff {
        let mut diff = SchemaDiff::new();

        let old_column = Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            true,
        );
        let new_column = Column::new("age".to_string(), ColumnType::VARCHAR { length: 50 }, true);
        let column_diff = ColumnDiff::new("age".to_string(), old_column, new_column);

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_columns.push(column_diff);
        diff.modified_tables.push(table_diff);

        diff
    }

    #[test]
    fn test_pipeline_type_change_postgresql() {
        let (old_schema, new_schema) = create_test_schemas_for_type_change();
        let diff = create_diff_with_type_change();

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, validation_result) = result.unwrap();
        assert!(sql.contains(r#"ALTER TABLE "users" ALTER COLUMN "age" TYPE"#));
        assert!(validation_result.is_valid());
    }

    #[test]
    fn test_pipeline_type_change_mysql() {
        let (old_schema, new_schema) = create_test_schemas_for_type_change();
        let diff = create_diff_with_type_change();

        let pipeline =
            MigrationPipeline::new(&diff, Dialect::MySQL).with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("ALTER TABLE `users` MODIFY COLUMN `age`"));
    }

    #[test]
    fn test_pipeline_type_change_sqlite() {
        let (old_schema, new_schema) = create_test_schemas_for_type_change();
        let diff = create_diff_with_type_change();

        let pipeline =
            MigrationPipeline::new(&diff, Dialect::SQLite).with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // SQLiteはテーブル再作成パターンを使用
        assert!(sql.contains("PRAGMA foreign_keys=off"));
        assert!(sql.contains("BEGIN TRANSACTION"));
        assert!(sql.contains(r#"CREATE TABLE "_stratum_tmp_recreate_users""#));
    }

    #[test]
    fn test_pipeline_type_change_validation_error() {
        // 互換性のない型変更（JSONB → INTEGER）でエラーが出るケース
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

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        // エラーがある場合はErrが返される
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.stage(), "prepare");
    }

    // ==========================================
    // DOWN SQL 型変更テスト
    // ==========================================

    #[test]
    fn test_pipeline_generate_down_type_change() {
        let (old_schema, new_schema) = create_test_schemas_for_type_change();
        let diff = create_diff_with_type_change();

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // Down方向では元の型(INTEGER)に戻す
        assert!(sql.contains(r#"ALTER TABLE "users" ALTER COLUMN "age" TYPE INTEGER"#));
    }

    // ==========================================
    // リネームカラム関連テスト
    // ==========================================

    fn create_test_table() -> Table {
        let mut table = Table::new("users".to_string());
        table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.columns.push(Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        table
    }

    fn create_old_table_for_rename() -> Table {
        let mut table = Table::new("users".to_string());
        table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        table
    }

    #[test]
    fn test_pipeline_rename_column_up_postgresql() {
        // 単純なリネームのUp方向テスト
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(renamed);

        let mut diff = SchemaDiff::new();
        diff.modified_tables.push(table_diff);

        let mut old_schema = Schema::new("1.0".to_string());
        old_schema
            .tables
            .insert("users".to_string(), create_old_table_for_rename());

        let mut new_schema = Schema::new("1.0".to_string());
        new_schema
            .tables
            .insert("users".to_string(), create_test_table());

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ALTER TABLE "users" RENAME COLUMN "name" TO "user_name""#),
            "Expected rename SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_rename_column_up_mysql() {
        // MySQLでのリネームUp方向テスト
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(renamed);

        let mut diff = SchemaDiff::new();
        diff.modified_tables.push(table_diff);

        let mut old_schema = Schema::new("1.0".to_string());
        old_schema
            .tables
            .insert("users".to_string(), create_old_table_for_rename());

        let mut new_schema = Schema::new("1.0".to_string());
        new_schema
            .tables
            .insert("users".to_string(), create_test_table());

        let pipeline =
            MigrationPipeline::new(&diff, Dialect::MySQL).with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // MySQLではCHANGE COLUMN構文を使用（完全なカラム定義が必要）
        assert!(
            sql.contains("ALTER TABLE `users` CHANGE COLUMN `name` `user_name`"),
            "Expected CHANGE COLUMN SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_rename_column_up_sqlite() {
        // SQLiteでのリネームUp方向テスト
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(renamed);

        let mut diff = SchemaDiff::new();
        diff.modified_tables.push(table_diff);

        let mut old_schema = Schema::new("1.0".to_string());
        old_schema
            .tables
            .insert("users".to_string(), create_old_table_for_rename());

        let mut new_schema = Schema::new("1.0".to_string());
        new_schema
            .tables
            .insert("users".to_string(), create_test_table());

        let pipeline =
            MigrationPipeline::new(&diff, Dialect::SQLite).with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ALTER TABLE "users" RENAME COLUMN "name" TO "user_name""#),
            "Expected rename SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_rename_column_down_postgresql() {
        // Down方向：逆リネームのテスト
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(renamed);

        let mut diff = SchemaDiff::new();
        diff.modified_tables.push(table_diff);

        let mut old_schema = Schema::new("1.0".to_string());
        old_schema
            .tables
            .insert("users".to_string(), create_old_table_for_rename());

        let mut new_schema = Schema::new("1.0".to_string());
        new_schema
            .tables
            .insert("users".to_string(), create_test_table());

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ALTER TABLE "users" RENAME COLUMN "user_name" TO "name""#),
            "Expected reverse rename SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_rename_with_type_change_order_up() {
        // リネーム+型変更: Up方向では「リネーム→型変更」の順序
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 50 },
            false,
        );
        let new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 }, // 型も変更
            false,
        );
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![ColumnChange::TypeChanged {
                old_type: "VARCHAR(50)".to_string(),
                new_type: "VARCHAR(100)".to_string(),
            }],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(renamed);

        let mut diff = SchemaDiff::new();
        diff.modified_tables.push(table_diff);

        // 新スキーマには新しい型のカラム
        let mut new_schema = Schema::new("1.0".to_string());
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.columns.push(Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        new_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        new_schema.tables.insert("users".to_string(), new_table);

        let mut old_schema = Schema::new("1.0".to_string());
        old_schema
            .tables
            .insert("users".to_string(), create_old_table_for_rename());

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // リネームSQLと型変更SQLの両方が含まれる
        assert!(
            sql.contains(r#"RENAME COLUMN "name" TO "user_name""#),
            "Expected rename SQL in: {}",
            sql
        );
        assert!(
            sql.contains(r#"ALTER TABLE "users" ALTER COLUMN "user_name" TYPE"#),
            "Expected type change SQL in: {}",
            sql
        );

        // リネームが型変更より先に出現すること
        let rename_pos = sql.find(r#"RENAME COLUMN "name" TO "user_name""#).unwrap();
        let type_change_pos = sql
            .find(r#"ALTER TABLE "users" ALTER COLUMN "user_name" TYPE"#)
            .unwrap();
        assert!(
            rename_pos < type_change_pos,
            "Rename should come before type change. SQL: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_rename_with_type_change_order_down() {
        // リネーム+型変更: Down方向では「型変更の逆→リネームの逆」の順序
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 50 },
            false,
        );
        let new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![ColumnChange::TypeChanged {
                old_type: "VARCHAR(50)".to_string(),
                new_type: "VARCHAR(100)".to_string(),
            }],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(renamed);

        let mut diff = SchemaDiff::new();
        diff.modified_tables.push(table_diff);

        let mut old_schema = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
        old_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        old_table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 50 },
            false,
        ));
        old_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        old_schema.tables.insert("users".to_string(), old_table);

        let mut new_schema = Schema::new("1.0".to_string());
        new_schema
            .tables
            .insert("users".to_string(), create_test_table());

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // 型変更の逆と逆リネームの両方が含まれる
        assert!(
            sql.contains(r#"ALTER TABLE "users" ALTER COLUMN"#),
            "Expected type change reversal SQL in: {}",
            sql
        );
        assert!(
            sql.contains(r#"RENAME COLUMN "user_name" TO "name""#),
            "Expected reverse rename SQL in: {}",
            sql
        );

        // 型変更がリネームより先に出現すること（Down方向では逆順）
        let type_change_pos = sql.find(r#"ALTER TABLE "users" ALTER COLUMN"#).unwrap();
        let rename_pos = sql.find(r#"RENAME COLUMN "user_name" TO "name""#).unwrap();
        assert!(
            type_change_pos < rename_pos,
            "Type change should come before rename in down direction. SQL: {}",
            sql
        );
    }

    // ==========================================
    // INTEGER→SERIAL変換のテスト
    // ==========================================

    #[test]
    fn test_pipeline_integer_to_serial_postgresql() {
        // PostgreSQLでのINTEGER→SERIAL変換テスト
        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(false);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        new_column.auto_increment = Some(true);

        let column_diff = ColumnDiff {
            column_name: "id".to_string(),
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![ColumnChange::AutoIncrementChanged {
                old_auto_increment: Some(false),
                new_auto_increment: Some(true),
            }],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_columns.push(column_diff);

        let mut diff = SchemaDiff::new();
        diff.modified_tables.push(table_diff);

        // スキーマを作成
        let mut old_schema = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
        old_table.columns.push(old_column);
        old_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        old_schema.tables.insert("users".to_string(), old_table);

        let mut new_schema = Schema::new("1.0".to_string());
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(new_column);
        new_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        new_schema.tables.insert("users".to_string(), new_table);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // シーケンス作成とDEFAULT設定が含まれること
        assert!(
            sql.contains("CREATE SEQUENCE"),
            "Expected CREATE SEQUENCE in: {}",
            sql
        );
        assert!(
            sql.contains("SET DEFAULT nextval"),
            "Expected SET DEFAULT nextval in: {}",
            sql
        );
        assert!(sql.contains("OWNED BY"), "Expected OWNED BY in: {}", sql);
    }

    #[test]
    fn test_pipeline_serial_to_integer_postgresql() {
        // PostgreSQLでのSERIAL→INTEGER変換テスト
        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(true);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        new_column.auto_increment = Some(false);

        let column_diff = ColumnDiff {
            column_name: "id".to_string(),
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![ColumnChange::AutoIncrementChanged {
                old_auto_increment: Some(true),
                new_auto_increment: Some(false),
            }],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_columns.push(column_diff);

        let mut diff = SchemaDiff::new();
        diff.modified_tables.push(table_diff);

        // スキーマを作成
        let mut old_schema = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
        old_table.columns.push(old_column);
        old_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        old_schema.tables.insert("users".to_string(), old_table);

        let mut new_schema = Schema::new("1.0".to_string());
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(new_column);
        new_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        new_schema.tables.insert("users".to_string(), new_table);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // DROP DEFAULTとシーケンス削除が含まれること
        assert!(
            sql.contains("DROP DEFAULT"),
            "Expected DROP DEFAULT in: {}",
            sql
        );
        assert!(
            sql.contains("DROP SEQUENCE"),
            "Expected DROP SEQUENCE in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_integer_to_bigserial_postgresql() {
        // PostgreSQLでのINTEGER→BIGSERIAL変換テスト（型変更＋auto_increment変更）
        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(false);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) }, // BIGINT
            false,
        );
        new_column.auto_increment = Some(true);

        let column_diff = ColumnDiff {
            column_name: "id".to_string(),
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![
                ColumnChange::TypeChanged {
                    old_type: "INTEGER".to_string(),
                    new_type: "BIGINT".to_string(),
                },
                ColumnChange::AutoIncrementChanged {
                    old_auto_increment: Some(false),
                    new_auto_increment: Some(true),
                },
            ],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_columns.push(column_diff);

        let mut diff = SchemaDiff::new();
        diff.modified_tables.push(table_diff);

        // スキーマを作成
        let mut old_schema = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
        old_table.columns.push(old_column);
        old_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        old_schema.tables.insert("users".to_string(), old_table);

        let mut new_schema = Schema::new("1.0".to_string());
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(new_column);
        new_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        new_schema.tables.insert("users".to_string(), new_table);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // シーケンス作成とDEFAULT設定が含まれること
        assert!(
            sql.contains("CREATE SEQUENCE"),
            "Expected CREATE SEQUENCE in: {}",
            sql
        );
        assert!(
            sql.contains("SET DEFAULT nextval"),
            "Expected SET DEFAULT nextval in: {}",
            sql
        );
        // 型変更（INTEGER→BIGINT）も含まれること
        assert!(
            sql.contains(r#"ALTER COLUMN "id" TYPE BIGINT"#),
            "Expected ALTER COLUMN TYPE BIGINT in: {}",
            sql
        );
    }

    // ==========================================
    // Down Migration テスト
    // ==========================================

    #[test]
    fn test_pipeline_down_serial_to_integer_reversal() {
        // Down migrationでSERIAL→INTEGER変換が逆転されること
        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(false);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        new_column.auto_increment = Some(true);

        let column_diff = ColumnDiff {
            column_name: "id".to_string(),
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![ColumnChange::AutoIncrementChanged {
                old_auto_increment: Some(false),
                new_auto_increment: Some(true),
            }],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_columns.push(column_diff);

        let mut diff = SchemaDiff::new();
        diff.modified_tables.push(table_diff);

        let mut old_schema = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
        old_table.columns.push(old_column);
        old_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        old_schema.tables.insert("users".to_string(), old_table);

        let mut new_schema = Schema::new("1.0".to_string());
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(new_column);
        new_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        new_schema.tables.insert("users".to_string(), new_table);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // Down方向ではSERIAL→INTEGERの逆、つまりINTEGER→SERIAL変換が行われる
        // (Up: INTEGER→SERIAL, Down: SERIAL→INTEGER)
        // しかし、down migrationでは old_schema を基準にするため、
        // 実際にはDROP DEFAULTとDROP SEQUENCEが生成される
        assert!(
            sql.contains("DROP DEFAULT") || sql.contains("DROP SEQUENCE"),
            "Expected DROP DEFAULT or DROP SEQUENCE in down SQL: {}",
            sql
        );
    }

    // ==========================================
    // テーブルリネーム関連テスト
    // ==========================================

    #[test]
    fn test_pipeline_rename_table_up_postgresql() {
        use crate::core::schema_diff::RenamedTable;

        let mut diff = SchemaDiff::new();
        let mut new_table = Table::new("accounts".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.renamed_from = Some("users".to_string());

        diff.renamed_tables.push(RenamedTable {
            old_name: "users".to_string(),
            new_table,
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ALTER TABLE "users" RENAME TO "accounts""#),
            "Expected rename SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_rename_table_up_mysql() {
        use crate::core::schema_diff::RenamedTable;

        let mut diff = SchemaDiff::new();
        let mut new_table = Table::new("accounts".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.renamed_from = Some("users".to_string());

        diff.renamed_tables.push(RenamedTable {
            old_name: "users".to_string(),
            new_table,
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains("RENAME TABLE `users` TO `accounts`"),
            "Expected rename SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_rename_table_up_sqlite() {
        use crate::core::schema_diff::RenamedTable;

        let mut diff = SchemaDiff::new();
        let mut new_table = Table::new("accounts".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.renamed_from = Some("users".to_string());

        diff.renamed_tables.push(RenamedTable {
            old_name: "users".to_string(),
            new_table,
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::SQLite);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ALTER TABLE "users" RENAME TO "accounts""#),
            "Expected rename SQL in: {}",
            sql
        );
    }

    #[test]
    fn test_pipeline_rename_table_down_postgresql() {
        use crate::core::schema_diff::RenamedTable;

        let mut diff = SchemaDiff::new();
        let mut new_table = Table::new("accounts".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.renamed_from = Some("users".to_string());

        diff.renamed_tables.push(RenamedTable {
            old_name: "users".to_string(),
            new_table,
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // Down方向では逆リネーム（accounts → users）
        assert!(
            sql.contains(r#"ALTER TABLE "accounts" RENAME TO "users""#),
            "Expected reverse rename SQL in: {}",
            sql
        );
    }
}
