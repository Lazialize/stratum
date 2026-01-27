// マイグレーションパイプラインサービス
//
// スキーマ差分からマイグレーションSQLを生成する共通パイプライン。
// with/without schemas の分岐を統一したパイプライン方式で処理する。

mod enum_stages;
mod index_constraint_stages;
mod table_stages;

use crate::adapters::sql_generator::mysql::MysqlSqlGenerator;
use crate::adapters::sql_generator::postgres::PostgresSqlGenerator;
use crate::adapters::sql_generator::sqlite::SqliteSqlGenerator;
use crate::adapters::sql_generator::{MigrationDirection, SqlGenerator};
use crate::core::config::Dialect;
use crate::core::error::ValidationResult;
use crate::core::schema::Schema;
use crate::core::schema_diff::{ColumnChange, SchemaDiff};

/// パイプラインステージでのエラー
#[derive(Debug, Clone)]
pub struct PipelineStageError {
    /// エラーが発生したステージ名
    pub stage: String,
    /// エラーメッセージ
    pub message: String,
}

impl std::fmt::Display for PipelineStageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.stage, self.message)
    }
}

impl std::error::Error for PipelineStageError {}

/// マイグレーション生成パイプライン
///
/// スキーマ差分からマイグレーションSQLを生成する共通パイプライン。
/// パイプラインは以下のステージで構成される:
/// 1. prepare - SqlGenerator取得、事前検証
/// 2. enum_statements - ENUM作成/変更（PostgreSQL）
/// 3. table_statements - CREATE/ALTER TABLE
/// 4. index_statements - CREATE INDEX
/// 5. constraint_statements - 制約追加
/// 6. cleanup_statements - DROP TABLE/TYPE
/// 7. finalize - SQL結合
pub struct MigrationPipeline<'a> {
    diff: &'a SchemaDiff,
    old_schema: Option<&'a Schema>,
    new_schema: Option<&'a Schema>,
    dialect: Dialect,
    allow_destructive: bool,
}

impl<'a> MigrationPipeline<'a> {
    /// 新しいパイプラインを作成
    ///
    /// # Arguments
    ///
    /// * `diff` - スキーマ差分
    /// * `dialect` - データベース方言
    pub fn new(diff: &'a SchemaDiff, dialect: Dialect) -> Self {
        Self {
            diff,
            old_schema: None,
            new_schema: None,
            dialect,
            allow_destructive: false,
        }
    }

    /// スキーマ情報を設定（型変更検証用）
    ///
    /// # Arguments
    ///
    /// * `old_schema` - 変更前のスキーマ
    /// * `new_schema` - 変更後のスキーマ
    pub fn with_schemas(mut self, old_schema: &'a Schema, new_schema: &'a Schema) -> Self {
        self.old_schema = Some(old_schema);
        self.new_schema = Some(new_schema);
        self
    }

    /// 破壊的変更を許可するか設定
    pub fn with_allow_destructive(mut self, allow_destructive: bool) -> Self {
        self.allow_destructive = allow_destructive;
        self
    }

    /// UP SQL を生成
    ///
    /// パイプラインステージを順に実行し、UP SQL を生成する。
    ///
    /// # Returns
    ///
    /// (SQL文字列, ValidationResult) またはエラー
    pub fn generate_up(&self) -> Result<(String, ValidationResult), PipelineStageError> {
        // ステージ1: prepare - 事前検証
        let validation_result = self.stage_prepare()?;
        if !validation_result.is_valid() {
            return Err(PipelineStageError {
                stage: "prepare".to_string(),
                message: format!(
                    "Type change validation failed:\n{}",
                    validation_result
                        .errors
                        .iter()
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            });
        }

        let generator = self.get_sql_generator();
        let mut statements = Vec::new();

        // ステージ2: enum_statements - ENUM作成/変更（PostgreSQL）
        if matches!(self.dialect, Dialect::PostgreSQL) {
            let enum_stmts = self.stage_enum_pre_table(&*generator)?;
            statements.extend(enum_stmts);
        }

        // ステージ3: table_statements - CREATE/ALTER TABLE
        let table_stmts = self.stage_table_statements(&*generator)?;
        statements.extend(table_stmts);

        // ステージ4: index_statements - CREATE INDEX
        let index_stmts = self.stage_index_statements(&*generator);
        statements.extend(index_stmts);

        // ステージ5: constraint_statements - 制約追加
        let constraint_stmts = self.stage_constraint_statements(&*generator);
        statements.extend(constraint_stmts);

        // ENUM post-table statements (PostgreSQL recreate)
        if matches!(self.dialect, Dialect::PostgreSQL) {
            let enum_post_stmts = self.stage_enum_post_table(&*generator)?;
            statements.extend(enum_post_stmts);
        }

        // ステージ6: cleanup_statements - DROP TABLE/TYPE
        let cleanup_stmts = self.stage_cleanup_statements(&*generator)?;
        statements.extend(cleanup_stmts);

        // ステージ7: finalize - SQL結合
        let sql = self.stage_finalize(statements);

        Ok((sql, validation_result))
    }

    /// DOWN SQL を生成
    ///
    /// パイプラインステージを逆順に実行し、DOWN SQL を生成する。
    ///
    /// # Returns
    ///
    /// (SQL文字列, ValidationResult) またはエラー
    pub fn generate_down(&self) -> Result<(String, ValidationResult), PipelineStageError> {
        let generator = self.get_sql_generator();
        let mut statements = Vec::new();

        // 追加されたテーブルを削除（依存関係の逆順）
        let sorted_tables =
            self.diff
                .sort_added_tables_by_dependency()
                .map_err(|e| PipelineStageError {
                    stage: "table_statements".to_string(),
                    message: e,
                })?;

        for table in sorted_tables.iter().rev() {
            statements.push(generator.generate_drop_table(&table.name));
        }

        // 変更されたテーブルの処理（逆操作）
        for table_diff in &self.diff.modified_tables {
            // 追加されたカラムを削除
            for column in &table_diff.added_columns {
                statements
                    .push(generator.generate_drop_column(&table_diff.table_name, &column.name));
            }

            // 型変更の逆処理（リネーム以外のカラム）
            for column_diff in &table_diff.modified_columns {
                if self.has_type_change(column_diff) {
                    if let Some(old_schema) = self.old_schema {
                        if let Some(table) = old_schema.tables.get(&table_diff.table_name) {
                            let other_table = self
                                .new_schema
                                .and_then(|s| s.tables.get(&table_diff.table_name));
                            let alter_statements = generator
                                .generate_alter_column_type_with_old_table(
                                    table,
                                    other_table,
                                    column_diff,
                                    MigrationDirection::Down,
                                );
                            statements.extend(alter_statements);
                        }
                    }
                }
            }

            // リネームカラムの逆処理（Down方向: 型変更の逆 → リネームの逆の順序）
            for renamed_column in &table_diff.renamed_columns {
                if let Some(old_schema) = self.old_schema {
                    if let Some(table) = old_schema.tables.get(&table_diff.table_name) {
                        // リネームと同時に型変更がある場合は、まず型変更を逆にする
                        // （新しいカラム名での操作なので、リネームの逆より先に実行）
                        if self.has_type_change_in_renamed(renamed_column) {
                            let column_diff = crate::core::schema_diff::ColumnDiff {
                                column_name: renamed_column.new_column.name.clone(),
                                old_column: renamed_column.old_column.clone(),
                                new_column: renamed_column.new_column.clone(),
                                changes: renamed_column.changes.clone(),
                            };
                            let other_table = self
                                .new_schema
                                .and_then(|s| s.tables.get(&table_diff.table_name));
                            let alter_statements = generator
                                .generate_alter_column_type_with_old_table(
                                    table,
                                    other_table,
                                    &column_diff,
                                    MigrationDirection::Down,
                                );
                            statements.extend(alter_statements);
                        }

                        // リネームの逆（new_name → old_name）
                        let rename_statements = generator.generate_rename_column(
                            table,
                            renamed_column,
                            MigrationDirection::Down,
                        );
                        statements.extend(rename_statements);
                    }
                }
            }

            // 追加されたインデックスを削除
            for index in &table_diff.added_indexes {
                statements.push(generator.generate_drop_index(&table_diff.table_name, index));
            }

            // 制約の逆操作（Down方向）
            if matches!(self.dialect, Dialect::SQLite) {
                // SQLite: 制約変更がある場合はテーブル再作成
                let has_constraint_changes = !table_diff.added_constraints.is_empty()
                    || !table_diff.removed_constraints.is_empty();

                if has_constraint_changes {
                    let has_type_change = table_diff
                        .modified_columns
                        .iter()
                        .any(|cd| self.has_type_change(cd));
                    let has_renamed_type_change = table_diff
                        .renamed_columns
                        .iter()
                        .any(|rc| self.has_type_change_in_renamed(rc));

                    if !has_type_change && !has_renamed_type_change {
                        // DOWN: old_schemaのテーブル定義をnew_table、new_schemaのテーブル定義をold_tableとして再作成
                        if let Some(old_schema) = self.old_schema {
                            if let Some(old_table) = old_schema.tables.get(&table_diff.table_name) {
                                let new_table_as_old = self
                                    .new_schema
                                    .and_then(|s| s.tables.get(&table_diff.table_name));
                                let recreator = crate::adapters::sql_generator::sqlite_table_recreator::SqliteTableRecreator::new();
                                let recreation_stmts = recreator
                                    .generate_table_recreation_with_old_table(
                                        old_table,
                                        new_table_as_old,
                                    );
                                statements.extend(recreation_stmts);
                            }
                        }
                    }
                }
            } else {
                // PostgreSQL・MySQL: ALTER TABLE で処理
                // 追加された制約を削除
                for constraint in &table_diff.added_constraints {
                    let sql = generator.generate_drop_constraint_for_existing_table(
                        &table_diff.table_name,
                        constraint,
                    );
                    if !sql.is_empty() {
                        statements.push(sql);
                    }
                }

                // 削除された制約を復元
                for constraint in &table_diff.removed_constraints {
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

        // 削除されたテーブルを再作成（手動対応が必要）
        for table_name in &self.diff.removed_tables {
            statements.push(generator.generate_missing_table_notice(table_name));
        }

        let sql = self.stage_finalize(statements);

        Ok((sql, ValidationResult::new()))
    }

    /// SqlGenerator を取得
    fn get_sql_generator(&self) -> Box<dyn SqlGenerator> {
        match self.dialect {
            Dialect::PostgreSQL => Box::new(PostgresSqlGenerator::new()),
            Dialect::MySQL => Box::new(MysqlSqlGenerator::new()),
            Dialect::SQLite => Box::new(SqliteSqlGenerator::new()),
        }
    }

    /// ステージ7: finalize - SQL結合
    fn stage_finalize(&self, statements: Vec<String>) -> String {
        statements.join(";\n\n") + if statements.is_empty() { "" } else { ";" }
    }

    /// カラム差分がTypeChangedまたはAutoIncrementChangedを含むかどうか
    ///
    /// PostgreSQLでは auto_increment の変更はSERIAL型への変換を伴うため、
    /// 型変更として扱う必要があります。
    fn has_type_change(&self, column_diff: &crate::core::schema_diff::ColumnDiff) -> bool {
        column_diff.changes.iter().any(|change| {
            matches!(
                change,
                ColumnChange::TypeChanged { .. } | ColumnChange::AutoIncrementChanged { .. }
            )
        })
    }

    /// リネームカラムがTypeChangedまたはAutoIncrementChangedを含むかどうか
    fn has_type_change_in_renamed(
        &self,
        renamed_column: &crate::core::schema_diff::RenamedColumn,
    ) -> bool {
        renamed_column.changes.iter().any(|change| {
            matches!(
                change,
                ColumnChange::TypeChanged { .. } | ColumnChange::AutoIncrementChanged { .. }
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::Table;

    // ==========================================
    // パイプライン基本テスト
    // ==========================================

    #[test]
    fn test_pipeline_new() {
        let diff = SchemaDiff::new();
        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);

        assert!(pipeline.old_schema.is_none());
        assert!(pipeline.new_schema.is_none());
    }

    #[test]
    fn test_pipeline_with_schemas() {
        let diff = SchemaDiff::new();
        let old_schema = Schema::new("1.0".to_string());
        let new_schema = Schema::new("1.0".to_string());

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);

        assert!(pipeline.old_schema.is_some());
        assert!(pipeline.new_schema.is_some());
    }

    #[test]
    fn test_pipeline_empty_diff() {
        let diff = SchemaDiff::new();
        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);

        let result = pipeline.generate_up();
        assert!(result.is_ok());
        let (sql, validation_result) = result.unwrap();
        assert!(sql.is_empty());
        assert!(validation_result.is_valid());
    }

    // ==========================================
    // エラーハンドリングテスト
    // ==========================================

    #[test]
    fn test_pipeline_stage_error_display() {
        let error = PipelineStageError {
            stage: "enum_statements".to_string(),
            message: "Enum recreation not allowed".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "[enum_statements] Enum recreation not allowed"
        );
    }

    #[test]
    fn test_pipeline_circular_dependency_error() {
        use crate::core::schema::Constraint;

        let mut diff = SchemaDiff::new();

        // 循環参照: A -> B -> A
        let mut table_a = Table::new("a".to_string());
        table_a.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["b_id".to_string()],
            referenced_table: "b".to_string(),
            referenced_columns: vec!["id".to_string()],
        });

        let mut table_b = Table::new("b".to_string());
        table_b.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["a_id".to_string()],
            referenced_table: "a".to_string(),
            referenced_columns: vec!["id".to_string()],
        });

        diff.added_tables.push(table_a);
        diff.added_tables.push(table_b);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.stage, "table_statements");
        assert!(err.message.contains("Circular reference"));
    }

    // ==========================================
    // DOWN SQL テスト (基本)
    // ==========================================

    #[test]
    fn test_pipeline_generate_down_drop_added_tables() {
        let mut diff = SchemaDiff::new();
        diff.added_tables.push(Table::new("users".to_string()));
        diff.added_tables.push(Table::new("posts".to_string()));

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains(r#"DROP TABLE "users""#));
        assert!(sql.contains(r#"DROP TABLE "posts""#));
    }
}
