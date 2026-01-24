// マイグレーションパイプラインサービス
//
// スキーマ差分からマイグレーションSQLを生成する共通パイプライン。
// with/without schemas の分岐を統一したパイプライン方式で処理する。

use crate::adapters::sql_generator::mysql::MysqlSqlGenerator;
use crate::adapters::sql_generator::postgres::PostgresSqlGenerator;
use crate::adapters::sql_generator::sqlite::SqliteSqlGenerator;
use crate::adapters::sql_generator::{MigrationDirection, SqlGenerator};
use crate::core::config::Dialect;
use crate::core::error::ValidationResult;
use crate::core::schema::Schema;
use crate::core::schema_diff::{ColumnChange, EnumChangeKind, EnumDiff, SchemaDiff};
use crate::services::type_change_validator::TypeChangeValidator;

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
            let enum_stmts = self.stage_enum_pre_table()?;
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
            let enum_post_stmts = self.stage_enum_post_table()?;
            statements.extend(enum_post_stmts);
        }

        // ステージ6: cleanup_statements - DROP TABLE/TYPE
        let cleanup_stmts = self.stage_cleanup_statements()?;
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
            statements.push(format!("DROP TABLE {}", table.name));
        }

        // 変更されたテーブルの処理（逆操作）
        for table_diff in &self.diff.modified_tables {
            // 追加されたカラムを削除
            for column in &table_diff.added_columns {
                statements.push(format!(
                    "ALTER TABLE {} DROP COLUMN {}",
                    table_diff.table_name, column.name
                ));
            }

            // 型変更の逆処理
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

            // 追加されたインデックスを削除
            for index in &table_diff.added_indexes {
                statements.push(format!("DROP INDEX {}", index.name));
            }
        }

        // 削除されたテーブルを再作成（手動対応が必要）
        for table_name in &self.diff.removed_tables {
            statements.push(format!(
                "-- NOTE: Manually add CREATE TABLE statement for '{}' if rollback is needed",
                table_name
            ));
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

    /// ステージ1: prepare - 事前検証
    fn stage_prepare(&self) -> Result<ValidationResult, PipelineStageError> {
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

    /// ステージ2: enum_statements (pre-table) - ENUM作成/変更
    fn stage_enum_pre_table(&self) -> Result<Vec<String>, PipelineStageError> {
        let mut statements = Vec::new();

        // ENUM再作成の許可チェック
        if (!self.diff.removed_enums.is_empty()
            || self
                .diff
                .modified_enums
                .iter()
                .any(|e| matches!(e.change_kind, EnumChangeKind::Recreate)))
            && !self.diff.enum_recreate_allowed
        {
            return Err(PipelineStageError {
                stage: "enum_statements".to_string(),
                message: "Enum recreation is required but not allowed. Enable enum_recreate_allowed to proceed.".to_string(),
            });
        }

        // 新規ENUM作成
        for enum_def in &self.diff.added_enums {
            let values = self.format_enum_values(&enum_def.values);
            statements.push(format!(
                "CREATE TYPE {} AS ENUM ({})",
                enum_def.name, values
            ));
        }

        // ENUM値追加（AddOnlyの場合）
        for enum_diff in &self.diff.modified_enums {
            if matches!(enum_diff.change_kind, EnumChangeKind::AddOnly) {
                for value in &enum_diff.added_values {
                    statements.push(format!(
                        "ALTER TYPE {} ADD VALUE '{}'",
                        enum_diff.enum_name,
                        self.escape_enum_value(value)
                    ));
                }
            }
        }

        Ok(statements)
    }

    /// ステージ: enum_statements (post-table) - ENUM再作成
    fn stage_enum_post_table(&self) -> Result<Vec<String>, PipelineStageError> {
        let mut statements = Vec::new();

        for enum_diff in &self.diff.modified_enums {
            if matches!(enum_diff.change_kind, EnumChangeKind::Recreate) {
                statements.extend(self.generate_enum_recreate_statements(enum_diff));
            }
        }

        Ok(statements)
    }

    /// ステージ3: table_statements - CREATE/ALTER TABLE
    fn stage_table_statements(
        &self,
        generator: &dyn SqlGenerator,
    ) -> Result<Vec<String>, PipelineStageError> {
        let mut statements = Vec::new();

        // 外部キー依存関係を考慮してテーブルをソート
        let sorted_tables =
            self.diff
                .sort_added_tables_by_dependency()
                .map_err(|e| PipelineStageError {
                    stage: "table_statements".to_string(),
                    message: e,
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
                statements.push(format!(
                    "ALTER TABLE {} ADD COLUMN {} {} {}",
                    table_diff.table_name,
                    column.name,
                    column.column_type.to_sql_type(&self.dialect),
                    if column.nullable { "" } else { "NOT NULL" }
                ));
            }

            // 型変更の処理
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
        }

        Ok(statements)
    }

    /// ステージ4: index_statements - CREATE INDEX
    fn stage_index_statements(&self, generator: &dyn SqlGenerator) -> Vec<String> {
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
    fn stage_constraint_statements(&self, _generator: &dyn SqlGenerator) -> Vec<String> {
        // 現時点では追加制約は table_statements で処理されている
        Vec::new()
    }

    /// ステージ6: cleanup_statements - DROP TABLE/TYPE
    fn stage_cleanup_statements(&self) -> Result<Vec<String>, PipelineStageError> {
        let mut statements = Vec::new();

        // 削除されたテーブルのDROP TABLE文を生成
        for table_name in &self.diff.removed_tables {
            statements.push(format!("DROP TABLE {}", table_name));
        }

        // ENUM削除（PostgreSQL）
        if matches!(self.dialect, Dialect::PostgreSQL) {
            for enum_name in &self.diff.removed_enums {
                statements.push(format!("DROP TYPE {}", enum_name));
            }
        }

        Ok(statements)
    }

    /// ステージ7: finalize - SQL結合
    fn stage_finalize(&self, statements: Vec<String>) -> String {
        statements.join(";\n\n") + if statements.is_empty() { "" } else { ";" }
    }

    /// カラム差分がTypeChangedを含むかどうか
    fn has_type_change(&self, column_diff: &crate::core::schema_diff::ColumnDiff) -> bool {
        column_diff
            .changes
            .iter()
            .any(|change| matches!(change, ColumnChange::TypeChanged { .. }))
    }

    /// ENUM値をフォーマット
    fn format_enum_values(&self, values: &[String]) -> String {
        values
            .iter()
            .map(|value| format!("'{}'", self.escape_enum_value(value)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// ENUM値をエスケープ
    fn escape_enum_value(&self, value: &str) -> String {
        value.replace('\'', "''")
    }

    /// ENUM再作成ステートメントを生成
    fn generate_enum_recreate_statements(&self, enum_diff: &EnumDiff) -> Vec<String> {
        let old_name = format!("{}_old", enum_diff.enum_name);
        let values = self.format_enum_values(&enum_diff.new_values);
        let mut statements = Vec::new();

        statements.push(format!(
            "ALTER TYPE {} RENAME TO {}",
            enum_diff.enum_name, old_name
        ));
        statements.push(format!(
            "CREATE TYPE {} AS ENUM ({})",
            enum_diff.enum_name, values
        ));

        for column in &enum_diff.columns {
            statements.push(format!(
                "ALTER TABLE {} ALTER COLUMN {} TYPE {} USING {}::text::{}",
                column.table_name,
                column.column_name,
                enum_diff.enum_name,
                column.column_name,
                enum_diff.enum_name
            ));
        }

        statements.push(format!("DROP TYPE {}", old_name));

        statements
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{Column, ColumnType, Constraint, EnumDefinition, Table};
    use crate::core::schema_diff::{
        ColumnDiff, EnumChangeKind, EnumColumnRef, EnumDiff, TableDiff,
    };

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
    // ENUM関連テスト
    // ==========================================

    #[test]
    fn test_pipeline_enum_create() {
        let mut diff = SchemaDiff::new();
        diff.added_enums.push(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("CREATE TYPE status AS ENUM ('active', 'inactive')"));
    }

    #[test]
    fn test_pipeline_enum_add_value() {
        let mut diff = SchemaDiff::new();
        diff.modified_enums.push(EnumDiff {
            enum_name: "status".to_string(),
            old_values: vec!["active".to_string()],
            new_values: vec!["active".to_string(), "inactive".to_string()],
            added_values: vec!["inactive".to_string()],
            removed_values: Vec::new(),
            change_kind: EnumChangeKind::AddOnly,
            columns: Vec::new(),
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("ALTER TYPE status ADD VALUE 'inactive'"));
    }

    #[test]
    fn test_pipeline_enum_recreate_requires_opt_in() {
        let mut diff = SchemaDiff::new();
        diff.modified_enums.push(EnumDiff {
            enum_name: "status".to_string(),
            old_values: vec!["active".to_string(), "inactive".to_string()],
            new_values: vec!["inactive".to_string(), "active".to_string()],
            added_values: Vec::new(),
            removed_values: Vec::new(),
            change_kind: EnumChangeKind::Recreate,
            columns: Vec::new(),
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.stage, "enum_statements");
    }

    #[test]
    fn test_pipeline_enum_recreate_with_opt_in() {
        let mut diff = SchemaDiff::new();
        diff.enum_recreate_allowed = true;
        diff.modified_enums.push(EnumDiff {
            enum_name: "status".to_string(),
            old_values: vec!["active".to_string(), "inactive".to_string()],
            new_values: vec!["inactive".to_string(), "active".to_string()],
            added_values: Vec::new(),
            removed_values: Vec::new(),
            change_kind: EnumChangeKind::Recreate,
            columns: vec![EnumColumnRef {
                table_name: "users".to_string(),
                column_name: "status".to_string(),
            }],
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("ALTER TYPE status RENAME TO status_old"));
        assert!(sql.contains("CREATE TYPE status AS ENUM ('inactive', 'active')"));
        assert!(sql.contains("DROP TYPE status_old"));
    }

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
        assert!(sql.contains("CREATE TABLE users"));
    }

    #[test]
    fn test_pipeline_drop_table() {
        let mut diff = SchemaDiff::new();
        diff.removed_tables.push("users".to_string());
        diff.enum_recreate_allowed = true; // 削除を許可

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("DROP TABLE users"));
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
        assert!(sql.contains("ALTER TABLE users ALTER COLUMN age TYPE"));
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
        assert!(sql.contains("ALTER TABLE users MODIFY COLUMN age"));
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
        assert!(sql.contains("CREATE TABLE new_users"));
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
        assert_eq!(err.stage, "prepare");
    }

    // ==========================================
    // DOWN SQL テスト
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
        assert!(sql.contains("DROP TABLE users"));
        assert!(sql.contains("DROP TABLE posts"));
    }

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
        assert!(sql.contains("ALTER TABLE users ALTER COLUMN age TYPE INTEGER"));
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
}
