// マイグレーションファイル生成サービス
//
// スキーマ差分からマイグレーションファイル（up.sql, down.sql, .meta.yaml）を生成するサービス。

use crate::adapters::sql_generator::mysql::MysqlSqlGenerator;
use crate::adapters::sql_generator::postgres::PostgresSqlGenerator;
use crate::adapters::sql_generator::sqlite::SqliteSqlGenerator;
use crate::adapters::sql_generator::{MigrationDirection, SqlGenerator};
use crate::core::config::Dialect;
use crate::core::error::ValidationResult;
use crate::core::schema::Schema;
use crate::core::schema_diff::{ColumnChange, EnumChangeKind, EnumDiff, SchemaDiff};
use crate::services::type_change_validator::TypeChangeValidator;
use chrono::Utc;

/// マイグレーションファイル生成サービス
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
    /// # Arguments
    ///
    /// * `diff` - スキーマ差分
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// UP SQL文字列（エラーの場合はエラーメッセージ）
    pub fn generate_up_sql(&self, diff: &SchemaDiff, dialect: Dialect) -> Result<String, String> {
        let mut statements = Vec::new();

        // データベース方言に応じたSQLジェネレーターを取得
        let generator: Box<dyn SqlGenerator> = match dialect {
            Dialect::PostgreSQL => Box::new(PostgresSqlGenerator::new()),
            Dialect::MySQL => Box::new(MysqlSqlGenerator::new()),
            Dialect::SQLite => Box::new(SqliteSqlGenerator::new()),
        };

        if matches!(dialect, Dialect::PostgreSQL) {
            self.append_enum_statements_pre_table(diff, &mut statements)?;
        }

        // 外部キー依存関係を考慮してテーブルをソート
        let sorted_tables = diff.sort_added_tables_by_dependency()?;

        // 追加されたテーブルのCREATE TABLE文を生成（ソート済み）
        for table in &sorted_tables {
            statements.push(generator.generate_create_table(table));

            // インデックスの作成
            for index in &table.indexes {
                statements.push(generator.generate_create_index(table, index));
            }

            // FOREIGN KEY制約の追加（SQLite以外）
            if !matches!(dialect, Dialect::SQLite) {
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
        for table_diff in &diff.modified_tables {
            // カラムの追加
            for column in &table_diff.added_columns {
                statements.push(format!(
                    "ALTER TABLE {} ADD COLUMN {} {} {}",
                    table_diff.table_name,
                    column.name,
                    self.get_column_type_string(column, dialect),
                    if column.nullable { "" } else { "NOT NULL" }
                ));
            }

            // インデックスの追加
            for index in &table_diff.added_indexes {
                let table = crate::core::schema::Table::new(table_diff.table_name.clone());
                statements.push(generator.generate_create_index(&table, index));
            }
        }

        if matches!(dialect, Dialect::PostgreSQL) {
            self.append_enum_statements_post_table(diff, &mut statements)?;
        }

        // 削除されたテーブルのDROP TABLE文を生成
        for table_name in &diff.removed_tables {
            statements.push(format!("DROP TABLE {}", table_name));
        }

        if matches!(dialect, Dialect::PostgreSQL) {
            self.append_enum_drop_statements(diff, &mut statements)?;
        }

        Ok(statements.join(";\n\n") + if statements.is_empty() { "" } else { ";" })
    }

    /// DOWN SQLを生成
    ///
    /// # Arguments
    ///
    /// * `diff` - スキーマ差分
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// DOWN SQL文字列（エラーの場合はエラーメッセージ）
    pub fn generate_down_sql(
        &self,
        diff: &SchemaDiff,
        _dialect: Dialect,
    ) -> Result<String, String> {
        let mut statements = Vec::new();

        // 外部キー依存関係を考慮してテーブルをソート
        let sorted_tables = diff.sort_added_tables_by_dependency()?;

        // UP SQLと逆の操作を生成
        // 追加されたテーブルを削除（依存関係の逆順 = 参照元を先に削除）
        for table in sorted_tables.iter().rev() {
            statements.push(format!("DROP TABLE {}", table.name));
        }

        // 変更されたテーブルの処理（逆操作）
        for table_diff in &diff.modified_tables {
            // 追加されたカラムを削除
            for column in &table_diff.added_columns {
                statements.push(format!(
                    "ALTER TABLE {} DROP COLUMN {}",
                    table_diff.table_name, column.name
                ));
            }

            // 追加されたインデックスを削除
            for index in &table_diff.added_indexes {
                statements.push(format!("DROP INDEX {}", index.name));
            }
        }

        // 削除されたテーブルを再作成
        // 注: ロールバック時に削除されたテーブルを復元するには、
        // 元のスキーマ定義が必要です。手動で CREATE TABLE 文を追加してください。
        for table_name in &diff.removed_tables {
            statements.push(format!(
                "-- NOTE: Manually add CREATE TABLE statement for '{}' if rollback is needed",
                table_name
            ));
        }

        Ok(statements.join(";\n\n") + if statements.is_empty() { "" } else { ";" })
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
    ) -> String {
        format!(
            "version: {}\ndescription: {}\ndialect: {:?}\nchecksum: {}\n",
            version, description, dialect, checksum
        )
    }

    /// カラム型を文字列に変換
    fn get_column_type_string(
        &self,
        column: &crate::core::schema::Column,
        dialect: Dialect,
    ) -> String {
        // ColumnType の to_sql_type メソッドを使用
        column.column_type.to_sql_type(&dialect)
    }

    fn append_enum_statements_pre_table(
        &self,
        diff: &SchemaDiff,
        statements: &mut Vec<String>,
    ) -> Result<(), String> {
        if (!diff.removed_enums.is_empty()
            || diff
                .modified_enums
                .iter()
                .any(|e| matches!(e.change_kind, EnumChangeKind::Recreate)))
            && !diff.enum_recreate_allowed
        {
            return Err(
                "Enum recreation is required but not allowed. Enable enum_recreate_allowed to proceed."
                    .to_string(),
            );
        }

        for enum_def in &diff.added_enums {
            let values = self.format_enum_values(&enum_def.values);
            statements.push(format!(
                "CREATE TYPE {} AS ENUM ({})",
                enum_def.name, values
            ));
        }

        for enum_diff in &diff.modified_enums {
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

        Ok(())
    }

    fn append_enum_statements_post_table(
        &self,
        diff: &SchemaDiff,
        statements: &mut Vec<String>,
    ) -> Result<(), String> {
        for enum_diff in &diff.modified_enums {
            if matches!(enum_diff.change_kind, EnumChangeKind::Recreate) {
                statements.extend(self.generate_enum_recreate_statements(enum_diff));
            }
        }

        Ok(())
    }

    fn append_enum_drop_statements(
        &self,
        diff: &SchemaDiff,
        statements: &mut Vec<String>,
    ) -> Result<(), String> {
        for enum_name in &diff.removed_enums {
            statements.push(format!("DROP TYPE {}", enum_name));
        }

        Ok(())
    }

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

    fn format_enum_values(&self, values: &[String]) -> String {
        values
            .iter()
            .map(|value| format!("'{}'", self.escape_enum_value(value)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn escape_enum_value(&self, value: &str) -> String {
        value.replace('\'', "''")
    }

    /// UP SQLを生成（スキーマ付き、型変更対応）
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
        _old_schema: &Schema,
        new_schema: &Schema,
        dialect: Dialect,
    ) -> Result<(String, ValidationResult), String> {
        // 型変更の検証
        let validator = TypeChangeValidator::new();
        let mut total_validation_result = ValidationResult::new();

        for table_diff in &diff.modified_tables {
            let validation = validator.validate_type_changes(
                &table_diff.table_name,
                &table_diff.modified_columns,
                &dialect,
            );
            total_validation_result.merge(validation);
        }

        // エラーがある場合は早期リターン
        if !total_validation_result.is_valid() {
            let error_messages: Vec<String> = total_validation_result
                .errors
                .iter()
                .map(|e| e.to_string())
                .collect();
            return Err(format!(
                "Type change validation failed:\n{}",
                error_messages.join("\n")
            ));
        }

        let mut statements = Vec::new();

        // データベース方言に応じたSQLジェネレーターを取得
        let generator: Box<dyn SqlGenerator> = match dialect {
            Dialect::PostgreSQL => Box::new(PostgresSqlGenerator::new()),
            Dialect::MySQL => Box::new(MysqlSqlGenerator::new()),
            Dialect::SQLite => Box::new(SqliteSqlGenerator::new()),
        };

        if matches!(dialect, Dialect::PostgreSQL) {
            self.append_enum_statements_pre_table(diff, &mut statements)?;
        }

        // 外部キー依存関係を考慮してテーブルをソート
        let sorted_tables = diff.sort_added_tables_by_dependency()?;

        // 追加されたテーブルのCREATE TABLE文を生成（ソート済み）
        for table in &sorted_tables {
            statements.push(generator.generate_create_table(table));

            // インデックスの作成
            for index in &table.indexes {
                statements.push(generator.generate_create_index(table, index));
            }

            // FOREIGN KEY制約の追加（SQLite以外）
            if !matches!(dialect, Dialect::SQLite) {
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
        for table_diff in &diff.modified_tables {
            // カラムの追加
            for column in &table_diff.added_columns {
                statements.push(format!(
                    "ALTER TABLE {} ADD COLUMN {} {} {}",
                    table_diff.table_name,
                    column.name,
                    self.get_column_type_string(column, dialect),
                    if column.nullable { "" } else { "NOT NULL" }
                ));
            }

            // 型変更の処理
            for column_diff in &table_diff.modified_columns {
                if self.has_type_change(column_diff) {
                    // Up方向ではnew_schemaからテーブル定義を取得
                    if let Some(table) = new_schema.tables.get(&table_diff.table_name) {
                        // 旧テーブル情報を渡して列交差ロジックを有効にする（SQLite用）
                        let old_table = _old_schema.tables.get(&table_diff.table_name);
                        let alter_statements = generator.generate_alter_column_type_with_old_table(
                            table,
                            old_table,
                            column_diff,
                            MigrationDirection::Up,
                        );
                        statements.extend(alter_statements);
                    }
                }
            }

            // インデックスの追加
            for index in &table_diff.added_indexes {
                let table = crate::core::schema::Table::new(table_diff.table_name.clone());
                statements.push(generator.generate_create_index(&table, index));
            }
        }

        if matches!(dialect, Dialect::PostgreSQL) {
            self.append_enum_statements_post_table(diff, &mut statements)?;
        }

        // 削除されたテーブルのDROP TABLE文を生成
        for table_name in &diff.removed_tables {
            statements.push(format!("DROP TABLE {}", table_name));
        }

        if matches!(dialect, Dialect::PostgreSQL) {
            self.append_enum_drop_statements(diff, &mut statements)?;
        }

        let sql = statements.join(";\n\n") + if statements.is_empty() { "" } else { ";" };
        Ok((sql, total_validation_result))
    }

    /// DOWN SQLを生成（スキーマ付き、型変更対応）
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
        _new_schema: &Schema,
        dialect: Dialect,
    ) -> Result<(String, ValidationResult), String> {
        let mut statements = Vec::new();

        // データベース方言に応じたSQLジェネレーターを取得
        let generator: Box<dyn SqlGenerator> = match dialect {
            Dialect::PostgreSQL => Box::new(PostgresSqlGenerator::new()),
            Dialect::MySQL => Box::new(MysqlSqlGenerator::new()),
            Dialect::SQLite => Box::new(SqliteSqlGenerator::new()),
        };

        // 外部キー依存関係を考慮してテーブルをソート
        let sorted_tables = diff.sort_added_tables_by_dependency()?;

        // UP SQLと逆の操作を生成
        // 追加されたテーブルを削除（依存関係の逆順 = 参照元を先に削除）
        for table in sorted_tables.iter().rev() {
            statements.push(format!("DROP TABLE {}", table.name));
        }

        // 変更されたテーブルの処理（逆操作）
        for table_diff in &diff.modified_tables {
            // 追加されたカラムを削除
            for column in &table_diff.added_columns {
                statements.push(format!(
                    "ALTER TABLE {} DROP COLUMN {}",
                    table_diff.table_name, column.name
                ));
            }

            // 型変更の逆処理（元の型に戻す）
            for column_diff in &table_diff.modified_columns {
                if self.has_type_change(column_diff) {
                    // Down方向ではold_schemaからテーブル定義を取得
                    if let Some(table) = old_schema.tables.get(&table_diff.table_name) {
                        // 旧テーブル情報を渡して列交差ロジックを有効にする（SQLite用）
                        // Down方向では新スキーマが「旧」として扱われる
                        let other_table = _new_schema.tables.get(&table_diff.table_name);
                        let alter_statements = generator.generate_alter_column_type_with_old_table(
                            table,
                            other_table,
                            column_diff,
                            MigrationDirection::Down,
                        );
                        statements.extend(alter_statements);
                    }
                }
            }

            // 追加されたインデックスを削除
            for index in &table_diff.added_indexes {
                statements.push(format!("DROP INDEX {}", index.name));
            }
        }

        // 削除されたテーブルを再作成
        for table_name in &diff.removed_tables {
            statements.push(format!(
                "-- NOTE: Manually add CREATE TABLE statement for '{}' if rollback is needed",
                table_name
            ));
        }

        let sql = statements.join(";\n\n") + if statements.is_empty() { "" } else { ";" };
        Ok((sql, ValidationResult::new()))
    }

    /// カラム差分がTypeChangedを含むかどうか
    fn has_type_change(&self, column_diff: &crate::core::schema_diff::ColumnDiff) -> bool {
        column_diff
            .changes
            .iter()
            .any(|change| matches!(change, ColumnChange::TypeChanged { .. }))
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
        let metadata = generator.generate_migration_metadata(
            "20260122120000",
            "create_users",
            Dialect::PostgreSQL,
            "abc123",
        );

        assert!(metadata.contains("version: 20260122120000"));
        assert!(metadata.contains("description: create_users"));
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

        assert!(sql.contains("CREATE TYPE status AS ENUM ('active', 'inactive')"));
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

        assert!(sql.contains("ALTER TYPE status ADD VALUE 'inactive'"));
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

        let sql = generator
            .generate_up_sql(&diff, Dialect::PostgreSQL)
            .unwrap();

        assert!(sql.contains("ALTER TYPE status RENAME TO status_old"));
        assert!(sql.contains("CREATE TYPE status AS ENUM ('inactive', 'active')"));
        assert!(sql.contains(
            "ALTER TABLE users ALTER COLUMN status TYPE status USING status::text::status"
        ));
        assert!(sql.contains("DROP TYPE status_old"));
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
    use crate::core::schema_diff::{ColumnDiff, TableDiff};

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
        assert!(sql.contains("ALTER TABLE users ALTER COLUMN age TYPE"));
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
        assert!(sql.contains("ALTER TABLE users MODIFY COLUMN age"));
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
        assert!(sql.contains("CREATE TABLE new_users"));
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
        assert!(sql.contains("ALTER TABLE users ALTER COLUMN age TYPE INTEGER"));
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
}
