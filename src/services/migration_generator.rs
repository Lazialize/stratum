// マイグレーションファイル生成サービス
//
// スキーマ差分からマイグレーションファイル（up.sql, down.sql, .meta.yaml）を生成するサービス。

use crate::adapters::sql_generator::mysql::MysqlSqlGenerator;
use crate::adapters::sql_generator::postgres::PostgresSqlGenerator;
use crate::adapters::sql_generator::sqlite::SqliteSqlGenerator;
use crate::adapters::sql_generator::SqlGenerator;
use crate::core::config::Dialect;
use crate::core::schema_diff::SchemaDiff;
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

        // 削除されたテーブルのDROP TABLE文を生成
        for table_name in &diff.removed_tables {
            statements.push(format!("DROP TABLE {}", table_name));
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
    pub fn generate_down_sql(&self, diff: &SchemaDiff, dialect: Dialect) -> Result<String, String> {
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

        // 削除されたテーブルを再作成（UPで生成した分）
        let _generator: Box<dyn SqlGenerator> = match dialect {
            Dialect::PostgreSQL => Box::new(PostgresSqlGenerator::new()),
            Dialect::MySQL => Box::new(MysqlSqlGenerator::new()),
            Dialect::SQLite => Box::new(SqliteSqlGenerator::new()),
        };

        for table_name in &diff.removed_tables {
            // 注: 実際のテーブル定義がないため、プレースホルダー
            statements.push(format!("-- TODO: Recreate table {}", table_name));
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
}

impl Default for MigrationGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
