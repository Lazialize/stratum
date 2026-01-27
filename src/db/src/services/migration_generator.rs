// マイグレーションファイル生成サービス
//
// スキーマ差分からマイグレーションファイル（up.sql, down.sql, .meta.yaml）を生成するサービス。
// 内部では MigrationPipeline を使用してSQL生成を行う。

use crate::core::config::Dialect;
use crate::core::destructive_change_report::DestructiveChangeReport;
use crate::core::error::ValidationResult;
use crate::core::migration::MigrationMetadata;
use crate::core::schema::Schema;
use crate::core::schema_diff::SchemaDiff;
use crate::services::migration_pipeline::MigrationPipeline;
use chrono::Utc;

/// マイグレーションファイル生成サービス
///
/// スキーマ差分からマイグレーションファイルを生成するサービス。
/// SQL生成は内部で MigrationPipeline に委譲する。
#[derive(Debug, Clone)]
pub struct MigrationGenerator {}

impl MigrationGenerator {
    /// 新しいMigrationGeneratorを作成
    pub fn new() -> Self {
        Self {}
    }

    /// タイムスタンプを生成
    ///
    /// YYYYMMDDHHmmss形式のタイムスタンプを生成します。
    ///
    /// # Returns
    ///
    /// タイムスタンプ文字列
    pub fn generate_timestamp(&self) -> String {
        let now = Utc::now();
        now.format("%Y%m%d%H%M%S").to_string()
    }

    /// マイグレーションファイル名を生成
    ///
    /// # Arguments
    ///
    /// * `timestamp` - タイムスタンプ
    /// * `description` - マイグレーションの説明
    ///
    /// # Returns
    ///
    /// ファイル名（例: 20260122120000_create_users_table）
    pub fn generate_migration_filename(&self, timestamp: &str, description: &str) -> String {
        format!("{}_{}", timestamp, description)
    }

    /// 説明文をファイル名用にサニタイズ
    ///
    /// # Arguments
    ///
    /// * `description` - サニタイズする説明文
    ///
    /// # Returns
    ///
    /// サニタイズされた説明文（小文字、スペースをアンダースコアに変換）
    pub fn sanitize_description(&self, description: &str) -> String {
        description
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .split('_')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_")
    }

    /// UP SQLを生成
    ///
    /// MigrationPipeline を使用してUP SQLを生成します。
    ///
    /// # Arguments
    ///
    /// * `diff` - スキーマ差分
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// UP SQL文字列（エラーの場合はエラーメッセージ）
    pub fn generate_up_sql(&self, diff: &SchemaDiff, dialect: Dialect) -> Result<String, String> {
        self.generate_up_sql_with_options(diff, dialect, false)
    }

    /// DOWN SQLを生成
    ///
    /// MigrationPipeline を使用してDOWN SQLを生成します。
    ///
    /// # Arguments
    ///
    /// * `diff` - スキーマ差分
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// DOWN SQL文字列（エラーの場合はエラーメッセージ）
    pub fn generate_down_sql(&self, diff: &SchemaDiff, dialect: Dialect) -> Result<String, String> {
        self.generate_down_sql_with_options(diff, dialect, false)
    }

    /// UP SQL文字列（破壊的変更許可付き）
    pub fn generate_up_sql_with_options(
        &self,
        diff: &SchemaDiff,
        dialect: Dialect,
        allow_destructive: bool,
    ) -> Result<String, String> {
        let pipeline =
            MigrationPipeline::new(diff, dialect).with_allow_destructive(allow_destructive);
        pipeline
            .generate_up()
            .map(|(sql, _)| sql)
            .map_err(|e| e.message)
    }

    /// DOWN SQL文字列（破壊的変更許可付き）
    pub fn generate_down_sql_with_options(
        &self,
        diff: &SchemaDiff,
        dialect: Dialect,
        allow_destructive: bool,
    ) -> Result<String, String> {
        let pipeline =
            MigrationPipeline::new(diff, dialect).with_allow_destructive(allow_destructive);
        pipeline
            .generate_down()
            .map(|(sql, _)| sql)
            .map_err(|e| e.message)
    }

    /// マイグレーションメタデータを生成
    ///
    /// # Arguments
    ///
    /// * `version` - マイグレーションバージョン
    /// * `description` - マイグレーションの説明
    /// * `dialect` - データベース方言
    /// * `checksum` - チェックサム
    ///
    /// # Returns
    ///
    /// YAML形式のメタデータ文字列
    pub fn generate_migration_metadata(
        &self,
        version: &str,
        description: &str,
        dialect: Dialect,
        checksum: &str,
        destructive_changes: DestructiveChangeReport,
    ) -> Result<String, String> {
        let metadata = MigrationMetadata {
            version: version.to_string(),
            description: description.to_string(),
            dialect,
            checksum: checksum.to_string(),
            destructive_changes,
        };

        serde_saphyr::to_string(&metadata)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))
    }

    /// UP SQLを生成（スキーマ付き、型変更対応）
    ///
    /// MigrationPipeline を使用してUP SQLを生成します。
    /// スキーマ情報を渡すことで、型変更の検証と適切なALTER文の生成が可能になります。
    ///
    /// # Arguments
    ///
    /// * `diff` - スキーマ差分
    /// * `old_schema` - 変更前のスキーマ
    /// * `new_schema` - 変更後のスキーマ
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// UP SQL文字列と検証結果のタプル、または検証エラー
    pub fn generate_up_sql_with_schemas(
        &self,
        diff: &SchemaDiff,
        old_schema: &Schema,
        new_schema: &Schema,
        dialect: Dialect,
    ) -> Result<(String, ValidationResult), String> {
        self.generate_up_sql_with_schemas_and_options(diff, old_schema, new_schema, dialect, false)
    }

    /// DOWN SQLを生成（スキーマ付き、型変更対応）
    ///
    /// MigrationPipeline を使用してDOWN SQLを生成します。
    /// スキーマ情報を渡すことで、型変更の逆操作が可能になります。
    ///
    /// # Arguments
    ///
    /// * `diff` - スキーマ差分
    /// * `old_schema` - 変更前のスキーマ
    /// * `new_schema` - 変更後のスキーマ
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// DOWN SQL文字列と検証結果のタプル、またはエラー
    pub fn generate_down_sql_with_schemas(
        &self,
        diff: &SchemaDiff,
        old_schema: &Schema,
        new_schema: &Schema,
        dialect: Dialect,
    ) -> Result<(String, ValidationResult), String> {
        self.generate_down_sql_with_schemas_and_options(
            diff, old_schema, new_schema, dialect, false,
        )
    }

    /// UP SQLを生成（スキーマ付き、破壊的変更許可付き）
    pub fn generate_up_sql_with_schemas_and_options(
        &self,
        diff: &SchemaDiff,
        old_schema: &Schema,
        new_schema: &Schema,
        dialect: Dialect,
        allow_destructive: bool,
    ) -> Result<(String, ValidationResult), String> {
        let pipeline = MigrationPipeline::new(diff, dialect)
            .with_schemas(old_schema, new_schema)
            .with_allow_destructive(allow_destructive);
        pipeline.generate_up().map_err(|e| e.to_string())
    }

    /// DOWN SQLを生成（スキーマ付き、破壊的変更許可付き）
    pub fn generate_down_sql_with_schemas_and_options(
        &self,
        diff: &SchemaDiff,
        old_schema: &Schema,
        new_schema: &Schema,
        dialect: Dialect,
        allow_destructive: bool,
    ) -> Result<(String, ValidationResult), String> {
        let pipeline = MigrationPipeline::new(diff, dialect)
            .with_schemas(old_schema, new_schema)
            .with_allow_destructive(allow_destructive);
        pipeline.generate_down().map_err(|e| e.to_string())
    }
}

impl Default for MigrationGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::EnumDefinition;
    use crate::core::schema_diff::{EnumChangeKind, EnumColumnRef, EnumDiff};

    #[test]
    fn test_new_service() {
        let generator = MigrationGenerator::new();
        assert!(format!("{:?}", generator).contains("MigrationGenerator"));
    }

    #[test]
    fn test_generate_timestamp() {
        let generator = MigrationGenerator::new();
        let timestamp = generator.generate_timestamp();

        assert_eq!(timestamp.len(), 14);
        assert!(timestamp.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_migration_filename() {
        let generator = MigrationGenerator::new();
        let filename = generator.generate_migration_filename("20260122120000", "create_users");

        assert_eq!(filename, "20260122120000_create_users");
    }

    #[test]
    fn test_sanitize_description() {
        let generator = MigrationGenerator::new();

        assert_eq!(
            generator.sanitize_description("Create Users Table"),
            "create_users_table"
        );
    }

    #[test]
    fn test_generate_migration_metadata() {
        let generator = MigrationGenerator::new();
        let metadata = generator
            .generate_migration_metadata(
                "20260122120000",
                "create_users",
                Dialect::PostgreSQL,
                "abc123",
                DestructiveChangeReport::new(),
            )
            .expect("Failed to generate metadata");

        assert!(
            metadata.contains("version: 20260122120000")
                || metadata.contains("version: \"20260122120000\"")
        );
        assert!(metadata.contains("description: create_users"));
        assert!(metadata.contains("destructive_changes"));
    }

    #[test]
    fn test_generate_up_sql_enum_create() {
        let generator = MigrationGenerator::new();
        let mut diff = SchemaDiff::new();
        diff.added_enums.push(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        let sql = generator
            .generate_up_sql(&diff, Dialect::PostgreSQL)
            .unwrap();

        assert!(sql.contains(r#"CREATE TYPE "status" AS ENUM ('active', 'inactive')"#));
    }

    #[test]
    fn test_generate_up_sql_enum_add_value() {
        let generator = MigrationGenerator::new();
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

        let sql = generator
            .generate_up_sql(&diff, Dialect::PostgreSQL)
            .unwrap();

        assert!(sql.contains(r#"ALTER TYPE "status" ADD VALUE 'inactive'"#));
    }

    #[test]
    fn test_generate_up_sql_enum_recreate_requires_opt_in() {
        let generator = MigrationGenerator::new();
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

        let result = generator.generate_up_sql(&diff, Dialect::PostgreSQL);

        assert!(result.is_err());
    }

    #[test]
    fn test_generate_up_sql_enum_recreate_with_opt_in() {
        let generator = MigrationGenerator::new();
        let mut diff = SchemaDiff::new();
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

        let sql = generator
            .generate_up_sql_with_options(&diff, Dialect::PostgreSQL, true)
            .unwrap();

        assert!(sql.contains(r#"ALTER TYPE "status" RENAME TO "status_old""#));
        assert!(sql.contains(r#"CREATE TYPE "status" AS ENUM ('inactive', 'active')"#));
        assert!(sql.contains(
            r#"ALTER TABLE "users" ALTER COLUMN "status" TYPE "status" USING "status"::text::"status""#
        ));
        assert!(sql.contains(r#"DROP TYPE "status_old""#));
    }

    #[test]
    fn test_generate_up_sql_enum_drop_requires_opt_in() {
        let generator = MigrationGenerator::new();
        let mut diff = SchemaDiff::new();
        diff.removed_enums.push("status".to_string());

        let result = generator.generate_up_sql(&diff, Dialect::PostgreSQL);

        assert!(result.is_err());
    }

    // ==========================================
    // 型変更SQL生成のテスト (with_schemas)
    // ==========================================

    use crate::core::schema::{Column, ColumnType, Constraint, Table};
    use crate::core::schema_diff::{ColumnChange, ColumnDiff, RenamedColumn, TableDiff};

    fn create_test_schemas_for_type_change() -> (Schema, Schema) {
        // 旧スキーマ: usersテーブル (age: INTEGER)
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

        // 新スキーマ: usersテーブル (age: VARCHAR)
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
    fn test_generate_up_sql_with_schemas_type_change_postgresql() {
        let generator = MigrationGenerator::new();
        let (old_schema, new_schema) = create_test_schemas_for_type_change();
        let diff = create_diff_with_type_change();

        let result = generator.generate_up_sql_with_schemas(
            &diff,
            &old_schema,
            &new_schema,
            Dialect::PostgreSQL,
        );

        assert!(result.is_ok());
        let (sql, validation_result) = result.unwrap();
        assert!(sql.contains(r#"ALTER TABLE "users" ALTER COLUMN "age" TYPE"#));
        assert!(validation_result.is_valid()); // Numeric → String は安全
    }

    #[test]
    fn test_generate_up_sql_with_schemas_type_change_mysql() {
        let generator = MigrationGenerator::new();
        let (old_schema, new_schema) = create_test_schemas_for_type_change();
        let diff = create_diff_with_type_change();

        let result =
            generator.generate_up_sql_with_schemas(&diff, &old_schema, &new_schema, Dialect::MySQL);

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains("ALTER TABLE `users` MODIFY COLUMN `age`"));
    }

    #[test]
    fn test_generate_up_sql_with_schemas_type_change_sqlite() {
        let generator = MigrationGenerator::new();
        let (old_schema, new_schema) = create_test_schemas_for_type_change();
        let diff = create_diff_with_type_change();

        let result = generator.generate_up_sql_with_schemas(
            &diff,
            &old_schema,
            &new_schema,
            Dialect::SQLite,
        );

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // SQLiteはテーブル再作成パターンを使用
        assert!(sql.contains("PRAGMA foreign_keys=off"));
        assert!(sql.contains("BEGIN TRANSACTION"));
        assert!(sql.contains(r#"CREATE TABLE "new_users""#));
    }

    #[test]
    fn test_generate_down_sql_with_schemas_type_change_postgresql() {
        let generator = MigrationGenerator::new();
        let (old_schema, new_schema) = create_test_schemas_for_type_change();
        let diff = create_diff_with_type_change();

        let result = generator.generate_down_sql_with_schemas(
            &diff,
            &old_schema,
            &new_schema,
            Dialect::PostgreSQL,
        );

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // Down方向では元の型(INTEGER)に戻す
        assert!(sql.contains(r#"ALTER TABLE "users" ALTER COLUMN "age" TYPE INTEGER"#));
    }

    #[test]
    fn test_generate_up_sql_with_schemas_validation_warning() {
        let generator = MigrationGenerator::new();

        // 逆方向の型変更（VARCHAR → INTEGER）で警告が出るケース
        let mut old_schema = Schema::new("1.0".to_string());
        let mut old_table = Table::new("products".to_string());
        old_table.columns.push(Column::new(
            "price".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        old_schema.tables.insert("products".to_string(), old_table);

        let mut new_schema = Schema::new("1.0".to_string());
        let mut new_table = Table::new("products".to_string());
        new_table.columns.push(Column::new(
            "price".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_schema.tables.insert("products".to_string(), new_table);

        let mut diff = SchemaDiff::new();
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
        let mut table_diff = TableDiff::new("products".to_string());
        table_diff.modified_columns.push(column_diff);
        diff.modified_tables.push(table_diff);

        let result = generator.generate_up_sql_with_schemas(
            &diff,
            &old_schema,
            &new_schema,
            Dialect::PostgreSQL,
        );

        assert!(result.is_ok());
        let (sql, validation_result) = result.unwrap();
        assert!(sql.contains("ALTER TABLE"));
        // String → Numeric は警告
        assert!(validation_result.warning_count() > 0);
    }

    #[test]
    fn test_generate_up_sql_with_schemas_validation_error() {
        let generator = MigrationGenerator::new();

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

        let result = generator.generate_up_sql_with_schemas(
            &diff,
            &old_schema,
            &new_schema,
            Dialect::PostgreSQL,
        );

        // エラーがある場合はErrが返される
        assert!(result.is_err());
    }

    // ==========================================
    // カラムリネームSQL生成のテスト
    // ==========================================

    fn create_test_schemas_for_rename() -> (Schema, Schema) {
        // 旧スキーマ: usersテーブル (name: VARCHAR)
        let mut old_schema = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
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
        old_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        old_schema.tables.insert("users".to_string(), old_table);

        // 新スキーマ: usersテーブル (user_name: VARCHAR) - nameをuser_nameにリネーム
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

        (old_schema, new_schema)
    }

    fn create_diff_with_rename() -> SchemaDiff {
        let mut diff = SchemaDiff::new();

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
        let renamed_column = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(renamed_column);
        diff.modified_tables.push(table_diff);

        diff
    }

    #[test]
    fn test_generate_up_sql_with_schemas_rename_column_postgresql() {
        let generator = MigrationGenerator::new();
        let (old_schema, new_schema) = create_test_schemas_for_rename();
        let diff = create_diff_with_rename();

        let result = generator.generate_up_sql_with_schemas(
            &diff,
            &old_schema,
            &new_schema,
            Dialect::PostgreSQL,
        );

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ALTER TABLE "users" RENAME COLUMN "name" TO "user_name""#),
            "SQL should contain RENAME COLUMN statement, got: {}",
            sql
        );
    }

    #[test]
    fn test_generate_up_sql_with_schemas_rename_column_mysql() {
        let generator = MigrationGenerator::new();
        let (old_schema, new_schema) = create_test_schemas_for_rename();
        let diff = create_diff_with_rename();

        let result =
            generator.generate_up_sql_with_schemas(&diff, &old_schema, &new_schema, Dialect::MySQL);

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // MySQLではCHANGE COLUMN構文を使用（完全なカラム定義が必要）
        assert!(
            sql.contains("ALTER TABLE `users` CHANGE COLUMN `name` `user_name`"),
            "SQL should contain CHANGE COLUMN statement, got: {}",
            sql
        );
    }

    #[test]
    fn test_generate_up_sql_with_schemas_rename_column_sqlite() {
        let generator = MigrationGenerator::new();
        let (old_schema, new_schema) = create_test_schemas_for_rename();
        let diff = create_diff_with_rename();

        let result = generator.generate_up_sql_with_schemas(
            &diff,
            &old_schema,
            &new_schema,
            Dialect::SQLite,
        );

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(
            sql.contains(r#"ALTER TABLE "users" RENAME COLUMN "name" TO "user_name""#),
            "SQL should contain RENAME COLUMN statement, got: {}",
            sql
        );
    }

    #[test]
    fn test_generate_down_sql_with_schemas_rename_column_postgresql() {
        let generator = MigrationGenerator::new();
        let (old_schema, new_schema) = create_test_schemas_for_rename();
        let diff = create_diff_with_rename();

        let result = generator.generate_down_sql_with_schemas(
            &diff,
            &old_schema,
            &new_schema,
            Dialect::PostgreSQL,
        );

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        // Down方向ではuser_nameをnameに戻す
        assert!(
            sql.contains(r#"ALTER TABLE "users" RENAME COLUMN "user_name" TO "name""#),
            "SQL should contain reverse RENAME COLUMN statement, got: {}",
            sql
        );
    }

    // ==========================================
    // カラムリネーム + 型変更のテスト
    // ==========================================

    fn create_test_schemas_for_rename_and_type_change() -> (Schema, Schema) {
        // 旧スキーマ: usersテーブル (age: INTEGER)
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

        // 新スキーマ: usersテーブル (age_years: VARCHAR) - リネーム + 型変更
        let mut new_schema = Schema::new("1.0".to_string());
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.columns.push(Column::new(
            "age_years".to_string(),
            ColumnType::VARCHAR { length: 50 },
            true,
        ));
        new_table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        new_schema.tables.insert("users".to_string(), new_table);

        (old_schema, new_schema)
    }

    fn create_diff_with_rename_and_type_change() -> SchemaDiff {
        let mut diff = SchemaDiff::new();

        let old_column = Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            true,
        );
        let new_column = Column::new(
            "age_years".to_string(),
            ColumnType::VARCHAR { length: 50 },
            true,
        );
        let renamed_column = RenamedColumn {
            old_name: "age".to_string(),
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![ColumnChange::TypeChanged {
                old_type: format!("{:?}", old_column.column_type),
                new_type: format!("{:?}", new_column.column_type),
            }],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(renamed_column);
        diff.modified_tables.push(table_diff);

        diff
    }

    #[test]
    fn test_generate_up_sql_with_schemas_rename_and_type_change_postgresql() {
        let generator = MigrationGenerator::new();
        let (old_schema, new_schema) = create_test_schemas_for_rename_and_type_change();
        let diff = create_diff_with_rename_and_type_change();

        let result = generator.generate_up_sql_with_schemas(
            &diff,
            &old_schema,
            &new_schema,
            Dialect::PostgreSQL,
        );

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // Up方向: まずリネーム、次に型変更
        let rename_pos = sql.find(r#"ALTER TABLE "users" RENAME COLUMN "age" TO "age_years""#);
        let type_pos = sql.find(r#"ALTER TABLE "users" ALTER COLUMN "age_years" TYPE"#);

        assert!(
            rename_pos.is_some(),
            "SQL should contain RENAME COLUMN, got: {}",
            sql
        );
        assert!(
            type_pos.is_some(),
            "SQL should contain ALTER COLUMN TYPE, got: {}",
            sql
        );

        // リネームが型変更より先に来ることを確認
        assert!(
            rename_pos.unwrap() < type_pos.unwrap(),
            "RENAME should come before TYPE change in Up direction"
        );
    }

    #[test]
    fn test_generate_down_sql_with_schemas_rename_and_type_change_postgresql() {
        let generator = MigrationGenerator::new();
        let (old_schema, new_schema) = create_test_schemas_for_rename_and_type_change();
        let diff = create_diff_with_rename_and_type_change();

        let result = generator.generate_down_sql_with_schemas(
            &diff,
            &old_schema,
            &new_schema,
            Dialect::PostgreSQL,
        );

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();

        // Down方向: まず型変更の逆操作、次にリネームの逆操作
        let type_pos = sql.find(r#"ALTER TABLE "users" ALTER COLUMN "age_years" TYPE INTEGER"#);
        let rename_pos = sql.find(r#"ALTER TABLE "users" RENAME COLUMN "age_years" TO "age""#);

        assert!(
            type_pos.is_some(),
            "SQL should contain TYPE change reversal, got: {}",
            sql
        );
        assert!(
            rename_pos.is_some(),
            "SQL should contain RENAME reversal, got: {}",
            sql
        );

        // Down方向では型変更がリネームより先に来ることを確認
        assert!(
            type_pos.unwrap() < rename_pos.unwrap(),
            "TYPE change should come before RENAME in Down direction"
        );
    }
}
