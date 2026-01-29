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
use thiserror::Error;

/// パイプラインステージでのエラー
#[derive(Debug, Clone, Error)]
pub enum PipelineStageError {
    /// 事前検証（型変更バリデーション）の失敗
    #[error("[prepare] Type change validation failed:\n{message}")]
    Prepare {
        /// バリデーションエラーメッセージ
        message: String,
    },

    /// テーブル依存関係の循環参照
    #[error("[table_statements] {message}")]
    CircularDependency {
        /// エラーメッセージ
        message: String,
    },

    /// ENUM再作成が許可されていない
    #[error("[enum_statements] Enum recreation is required but not allowed. Use --allow-destructive to proceed.")]
    EnumRecreationNotAllowed,
}

impl PipelineStageError {
    /// エラーが発生したステージ名を取得
    pub fn stage(&self) -> &str {
        match self {
            PipelineStageError::Prepare { .. } => "prepare",
            PipelineStageError::CircularDependency { .. } => "table_statements",
            PipelineStageError::EnumRecreationNotAllowed => "enum_statements",
        }
    }
}

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
            return Err(PipelineStageError::Prepare {
                message: validation_result.errors_to_string(),
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
        let sql = self.add_transaction_header(sql);

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

        // ENUM操作の逆処理（PostgreSQL）
        if matches!(self.dialect, Dialect::PostgreSQL) {
            // 追加されたENUMを削除
            for enum_def in &self.diff.added_enums {
                statements.extend(generator.generate_drop_enum_type(&enum_def.name));
            }

            // 変更されたENUMの逆処理（手動対応が必要）
            for enum_diff in &self.diff.modified_enums {
                statements.push(format!(
                    "-- TODO: Reverse ENUM modification for '{}' (manual intervention required)",
                    enum_diff.enum_name
                ));
            }

            // 削除されたENUMを再作成
            for enum_name in &self.diff.removed_enums {
                if let Some(old_schema) = self.old_schema {
                    if let Some(enum_def) = old_schema.enums.get(enum_name) {
                        statements.extend(generator.generate_create_enum_type(enum_def));
                    } else {
                        statements.push(format!(
                            "-- TODO: Recreate ENUM type '{}' (definition not available)",
                            enum_name
                        ));
                    }
                } else {
                    statements.push(format!(
                        "-- TODO: Recreate ENUM type '{}' (old schema not available)",
                        enum_name
                    ));
                }
            }
        }

        // 追加されたテーブルを削除（依存関係の逆順）
        let sorted_tables = self.diff.sort_added_tables_by_dependency().map_err(|e| {
            PipelineStageError::CircularDependency {
                message: e.to_string(),
            }
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

            // nullable/default変更の逆処理（型変更がないカラム、SQLite以外）
            if !matches!(self.dialect, Dialect::SQLite) {
                for column_diff in &table_diff.modified_columns {
                    if !self.has_type_change(column_diff)
                        && self.has_nullable_or_default_change(column_diff)
                    {
                        // DOWN: old_columnの値を使って逆操作を生成
                        let target_column = &column_diff.old_column;
                        for change in &column_diff.changes {
                            match change {
                                ColumnChange::NullableChanged { old_nullable, .. } => {
                                    statements.extend(generator.generate_alter_column_nullable(
                                        &table_diff.table_name,
                                        target_column,
                                        *old_nullable,
                                    ));
                                }
                                ColumnChange::DefaultValueChanged { old_default, .. } => {
                                    statements.extend(generator.generate_alter_column_default(
                                        &table_diff.table_name,
                                        target_column,
                                        old_default.as_deref(),
                                    ));
                                }
                                _ => {}
                            }
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
                statements.push(generator.generate_drop_index(&table_diff.table_name, &index.name));
            }

            // 制約の逆操作（Down方向）
            if matches!(self.dialect, Dialect::SQLite) {
                // SQLite: 制約変更またはnullable/default変更がある場合はテーブル再作成
                let has_constraint_changes = !table_diff.added_constraints.is_empty()
                    || !table_diff.removed_constraints.is_empty();
                let has_nullable_or_default_changes = table_diff
                    .modified_columns
                    .iter()
                    .any(|cd| self.has_nullable_or_default_change(cd));

                if has_constraint_changes || has_nullable_or_default_changes {
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

        // リネームされたテーブルの逆処理（new_name → old_name）
        for renamed_table in &self.diff.renamed_tables {
            statements.push(
                generator
                    .generate_rename_table(&renamed_table.new_table.name, &renamed_table.old_name),
            );
        }

        // 削除されたテーブルを再作成
        for table_name in &self.diff.removed_tables {
            if let Some(old_schema) = self.old_schema {
                if let Some(old_table) = old_schema.tables.get(table_name) {
                    // old_schemaからCREATE TABLE文を生成
                    statements.push(generator.generate_create_table(old_table));

                    // インデックスも再作成
                    for index in &old_table.indexes {
                        statements.push(generator.generate_create_index(old_table, index));
                    }

                    // FOREIGN KEY制約も再作成（SQLite以外）
                    if !matches!(self.dialect, Dialect::SQLite) {
                        for (i, constraint) in old_table.constraints.iter().enumerate() {
                            if matches!(
                                constraint,
                                crate::core::schema::Constraint::FOREIGN_KEY { .. }
                            ) {
                                let alter_sql =
                                    generator.generate_alter_table_add_constraint(old_table, i);
                                if !alter_sql.is_empty() {
                                    statements.push(alter_sql);
                                }
                            }
                        }
                    }
                } else {
                    statements.push(generator.generate_missing_table_notice(table_name));
                }
            } else {
                statements.push(generator.generate_missing_table_notice(table_name));
            }
        }

        let sql = self.stage_finalize(statements);
        let sql = self.add_transaction_header(sql);

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

    /// トランザクションヘッダーコメントを追加
    ///
    /// apply コマンドが既にトランザクション内で SQL を実行するため、
    /// SQL ファイルに BEGIN/COMMIT を直接含めない。
    /// 代わりに手動実行時のガイダンスとしてコメントを追加する。
    fn add_transaction_header(&self, sql: String) -> String {
        if sql.is_empty() {
            return sql;
        }

        match self.dialect {
            Dialect::PostgreSQL => {
                format!(
                    "-- Transaction: strata apply wraps this in a transaction automatically.\n-- For manual execution: BEGIN; ... COMMIT;\n\n{}",
                    sql
                )
            }
            Dialect::MySQL => {
                format!(
                    "-- Transaction: strata apply wraps this in a transaction automatically.\n-- NOTE: MySQL DDL statements cause implicit commits.\n\n{}",
                    sql
                )
            }
            Dialect::SQLite => {
                format!(
                    "-- Transaction: strata apply wraps this in a transaction automatically.\n\n{}",
                    sql
                )
            }
        }
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

    /// カラム差分がNullableChangedまたはDefaultValueChangedを含むかどうか
    fn has_nullable_or_default_change(
        &self,
        column_diff: &crate::core::schema_diff::ColumnDiff,
    ) -> bool {
        column_diff.changes.iter().any(|change| {
            matches!(
                change,
                ColumnChange::NullableChanged { .. } | ColumnChange::DefaultValueChanged { .. }
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
        let error = PipelineStageError::EnumRecreationNotAllowed;

        assert_eq!(error.stage(), "enum_statements");
        assert!(error.to_string().contains("enum_statements"));
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
            on_delete: None,
            on_update: None,
        });

        let mut table_b = Table::new("b".to_string());
        table_b.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["a_id".to_string()],
            referenced_table: "a".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });

        diff.added_tables.push(table_a);
        diff.added_tables.push(table_b);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.stage(), "table_statements");
        assert!(err.to_string().contains("Circular reference"));
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

    #[test]
    fn test_pipeline_with_allow_destructive() {
        let diff = SchemaDiff::new();
        let pipeline =
            MigrationPipeline::new(&diff, Dialect::PostgreSQL).with_allow_destructive(true);
        assert!(pipeline.allow_destructive);
    }

    #[test]
    fn test_pipeline_stage_error_prepare() {
        let error = PipelineStageError::Prepare {
            message: "validation failed".to_string(),
        };
        assert_eq!(error.stage(), "prepare");
        assert!(error.to_string().contains("validation failed"));
    }

    #[test]
    fn test_pipeline_stage_error_circular_dependency() {
        let error = PipelineStageError::CircularDependency {
            message: "cycle detected".to_string(),
        };
        assert_eq!(error.stage(), "table_statements");
        assert!(error.to_string().contains("cycle detected"));
    }

    #[test]
    fn test_pipeline_generate_down_removed_table_with_old_schema() {
        use crate::core::schema::Column;
        use crate::core::schema::ColumnType;

        let mut diff = SchemaDiff::new();
        diff.removed_tables.push("users".to_string());

        let mut old_schema = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
        old_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        old_schema.add_table(old_table);
        let new_schema = Schema::new("1.0".to_string());

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_down();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("users"));
    }

    #[test]
    fn test_pipeline_generate_down_removed_table_without_old_schema() {
        let mut diff = SchemaDiff::new();
        diff.removed_tables.push("users".to_string());

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("NOTE"));
        assert!(sql.contains("users"));
    }

    #[test]
    fn test_pipeline_generate_down_added_columns_dropped() {
        use crate::core::schema::{Column, ColumnType};
        use crate::core::schema_diff::TableDiff;

        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.added_columns.push(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            true,
        ));
        diff.modified_tables.push(table_diff);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("DROP COLUMN"));
        assert!(sql.contains("email"));
    }

    #[test]
    fn test_pipeline_generate_down_renamed_table() {
        use crate::core::schema_diff::RenamedTable;

        let mut diff = SchemaDiff::new();
        let new_table = Table::new("accounts".to_string());
        diff.renamed_tables.push(RenamedTable {
            old_name: "users".to_string(),
            new_table,
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("RENAME TO"));
        assert!(sql.contains("accounts"));
        assert!(sql.contains("users"));
    }

    #[test]
    fn test_pipeline_transaction_header_mysql() {
        let mut diff = SchemaDiff::new();
        diff.added_tables.push(Table::new("test".to_string()));

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL);
        let result = pipeline.generate_up();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("implicit commits"));
    }

    #[test]
    fn test_pipeline_transaction_header_sqlite() {
        let mut diff = SchemaDiff::new();
        diff.added_tables.push(Table::new("test".to_string()));

        let pipeline = MigrationPipeline::new(&diff, Dialect::SQLite);
        let result = pipeline.generate_up();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("Transaction:"));
    }

    #[test]
    fn test_pipeline_empty_sql_no_header() {
        let diff = SchemaDiff::new();
        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.is_empty());
    }

    #[test]
    fn test_pipeline_generate_down_enum_added() {
        use crate::core::schema::EnumDefinition;

        let mut diff = SchemaDiff::new();
        diff.added_enums.push(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("DROP TYPE") || sql.contains("status"));
    }

    #[test]
    fn test_pipeline_generate_down_enum_removed_with_old_schema() {
        use crate::core::schema::EnumDefinition;

        let mut diff = SchemaDiff::new();
        diff.removed_enums.push("status".to_string());

        let mut old_schema = Schema::new("1.0".to_string());
        old_schema.enums.insert(
            "status".to_string(),
            EnumDefinition {
                name: "status".to_string(),
                values: vec!["active".to_string(), "inactive".to_string()],
            },
        );
        let new_schema = Schema::new("1.0".to_string());

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);
        let result = pipeline.generate_down();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("CREATE TYPE") || sql.contains("status"));
    }

    #[test]
    fn test_pipeline_generate_down_enum_removed_without_old_schema() {
        let mut diff = SchemaDiff::new();
        diff.removed_enums.push("status".to_string());

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("TODO"));
        assert!(sql.contains("status"));
    }

    #[test]
    fn test_pipeline_generate_down_enum_modified() {
        use crate::core::schema_diff::{EnumChangeKind, EnumDiff};

        let mut diff = SchemaDiff::new();
        diff.modified_enums.push(EnumDiff {
            enum_name: "status".to_string(),
            old_values: vec!["active".to_string()],
            new_values: vec!["active".to_string(), "pending".to_string()],
            added_values: vec!["pending".to_string()],
            removed_values: vec![],
            change_kind: EnumChangeKind::AddOnly,
            columns: vec![],
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_down();
        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("TODO"));
        assert!(sql.contains("status"));
    }
}
