// データベースマイグレーターサービス
//
// マイグレーション履歴テーブルの管理とトランザクション制御を担当するサービス。
// データベース固有のSQL構文を抽象化し、マイグレーション適用とロールバックをサポートします。

use crate::core::config::Dialect;
use crate::core::error::DatabaseError;
use crate::core::migration::{Migration, MigrationRecord};
use chrono::{DateTime, Utc};
use regex::Regex;
use sqlx::{any::AnyQueryResult, AnyPool, Row};

/// 許可されるマイグレーションテーブル名のパターン
///
/// # Security
/// - 英字またはアンダースコアで開始
/// - 英数字とアンダースコアのみで構成
/// - 最大63文字（PostgreSQL識別子制限）
const ALLOWED_TABLE_NAME_PATTERN: &str = r"^[a-zA-Z_][a-zA-Z0-9_]{0,62}$";

/// デフォルトのマイグレーションテーブル名
pub const DEFAULT_MIGRATION_TABLE: &str = "schema_migrations";

/// データベースマイグレーターサービス
///
/// マイグレーション履歴の管理とトランザクション制御を提供します。
#[derive(Debug, Clone)]
pub struct DatabaseMigratorService {}

impl DatabaseMigratorService {
    /// 新しいDatabaseMigratorServiceを作成
    pub fn new() -> Self {
        Self {}
    }

    /// マイグレーションテーブル名を検証
    ///
    /// # Security
    /// SQLインジェクション防止のため、テーブル名は以下を満たす必要がある:
    /// - 英字またはアンダースコアで開始
    /// - 英数字とアンダースコアのみで構成
    /// - 最大63文字（PostgreSQL識別子制限）
    ///
    /// # Arguments
    /// * `name` - 検証するテーブル名
    ///
    /// # Returns
    /// 検証成功時はOk(()), 失敗時はDatabaseError::InvalidTableName
    pub fn validate_table_name(name: &str) -> Result<(), DatabaseError> {
        if name.is_empty() {
            return Err(DatabaseError::InvalidTableName {
                name: name.to_string(),
                reason: "Table name cannot be empty".to_string(),
            });
        }

        let re = Regex::new(ALLOWED_TABLE_NAME_PATTERN).expect("Invalid regex pattern");
        if !re.is_match(name) {
            return Err(DatabaseError::InvalidTableName {
                name: name.to_string(),
                reason: "Table name must start with letter or underscore, contain only alphanumeric characters and underscores, and be at most 63 characters".to_string(),
            });
        }
        Ok(())
    }

    /// マイグレーション履歴テーブル作成SQLを生成
    ///
    /// # Arguments
    ///
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// CREATE TABLE文のSQL文字列
    pub fn generate_create_migration_table_sql(&self, dialect: Dialect) -> String {
        match dialect {
            Dialect::PostgreSQL => r#"CREATE TABLE IF NOT EXISTS schema_migrations (
    version VARCHAR(255) PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    checksum VARCHAR(64) NOT NULL
)"#
            .to_string(),
            Dialect::MySQL => r#"CREATE TABLE IF NOT EXISTS schema_migrations (
    version VARCHAR(255) PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    checksum VARCHAR(64) NOT NULL
)"#
            .to_string(),
            Dialect::SQLite => r#"CREATE TABLE IF NOT EXISTS schema_migrations (
    version TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now')),
    checksum TEXT NOT NULL
)"#
            .to_string(),
        }
    }

    /// マイグレーション履歴テーブルを作成
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// 作成に成功した場合はOk(())、失敗した場合はエラー
    pub async fn create_migration_table(
        &self,
        pool: &AnyPool,
        dialect: Dialect,
    ) -> Result<(), DatabaseError> {
        let sql = self.generate_create_migration_table_sql(dialect);

        sqlx::query(&sql)
            .execute(pool)
            .await
            .map_err(|e| DatabaseError::Query {
                message: format!("Failed to create migration history table: {}", e),
                sql: Some(sql),
            })?;

        Ok(())
    }

    /// マイグレーション記録クエリを生成（パラメータバインド対応）
    ///
    /// # Security
    /// - テーブル名: `schema_migrations`固定（許可リスト検証済み）
    /// - パラメータ値: bind() でエスケープ
    ///
    /// # Arguments
    ///
    /// * `migration` - 記録するマイグレーション
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// (SQL文字列, バインドパラメータのベクタ)
    pub fn generate_record_migration_query(
        &self,
        migration: &Migration,
        dialect: Dialect,
    ) -> (String, Vec<String>) {
        let sql = match dialect {
            Dialect::PostgreSQL => {
                "INSERT INTO schema_migrations (version, description, applied_at, checksum) VALUES ($1, $2, $3, $4)".to_string()
            }
            Dialect::MySQL | Dialect::SQLite => {
                "INSERT INTO schema_migrations (version, description, applied_at, checksum) VALUES (?, ?, ?, ?)".to_string()
            }
        };

        let params = vec![
            migration.version.clone(),
            migration.description.clone(),
            migration.timestamp.to_rfc3339(),
            migration.checksum.clone(),
        ];

        (sql, params)
    }

    /// マイグレーション記録をデータベースに保存（パラメータバインド対応）
    ///
    /// # Security
    /// パラメータバインディングを使用してSQLインジェクションを防止
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    /// * `migration` - 記録するマイグレーション
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// 保存に成功した場合はOk(())、失敗した場合はエラー
    pub async fn record_migration_with_dialect(
        &self,
        pool: &AnyPool,
        migration: &Migration,
        dialect: Dialect,
    ) -> Result<(), DatabaseError> {
        let (sql, params) = self.generate_record_migration_query(migration, dialect);

        let mut query = sqlx::query(&sql);
        for param in &params {
            query = query.bind(param);
        }

        query
            .execute(pool)
            .await
            .map_err(|e| DatabaseError::Query {
                message: format!("Failed to save migration record: {}", e),
                sql: Some(sql),
            })?;

        Ok(())
    }

    /// マイグレーション削除クエリを生成（パラメータバインド対応）
    ///
    /// # Security
    /// - テーブル名: `schema_migrations`固定（許可リスト検証済み）
    /// - パラメータ値: bind() でエスケープ
    ///
    /// # Arguments
    ///
    /// * `version` - 削除するマイグレーションのバージョン
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// (SQL文字列, バインドパラメータのベクタ)
    pub fn generate_remove_migration_query(
        &self,
        version: &str,
        dialect: Dialect,
    ) -> (String, Vec<String>) {
        let sql = match dialect {
            Dialect::PostgreSQL => "DELETE FROM schema_migrations WHERE version = $1".to_string(),
            Dialect::MySQL | Dialect::SQLite => {
                "DELETE FROM schema_migrations WHERE version = ?".to_string()
            }
        };

        let params = vec![version.to_string()];

        (sql, params)
    }

    /// マイグレーション記録をデータベースから削除（パラメータバインド対応）
    ///
    /// # Security
    /// パラメータバインディングを使用してSQLインジェクションを防止
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    /// * `version` - 削除するマイグレーションのバージョン
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// 削除に成功した場合はOk(())、失敗した場合はエラー
    pub async fn remove_migration_with_dialect(
        &self,
        pool: &AnyPool,
        version: &str,
        dialect: Dialect,
    ) -> Result<(), DatabaseError> {
        let (sql, params) = self.generate_remove_migration_query(version, dialect);

        let mut query = sqlx::query(&sql);
        for param in &params {
            query = query.bind(param);
        }

        query
            .execute(pool)
            .await
            .map_err(|e| DatabaseError::Query {
                message: format!("Failed to delete migration record: {}", e),
                sql: Some(sql),
            })?;

        Ok(())
    }

    /// マイグレーション履歴取得のSELECT SQLを生成
    ///
    /// # Returns
    ///
    /// SELECT文のSQL文字列
    pub fn generate_get_migrations_sql(&self, dialect: Dialect) -> String {
        match dialect {
            Dialect::PostgreSQL => {
                "SELECT version, description, applied_at::text AS applied_at, checksum FROM schema_migrations ORDER BY version"
                    .to_string()
            }
            Dialect::MySQL => {
                "SELECT version, description, CAST(applied_at AS CHAR) AS applied_at, checksum FROM schema_migrations ORDER BY version"
                    .to_string()
            }
            Dialect::SQLite => {
                "SELECT version, description, applied_at, checksum FROM schema_migrations ORDER BY version"
                    .to_string()
            }
        }
    }

    /// データベースからすべてのマイグレーション記録を取得
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    ///
    /// # Returns
    ///
    /// マイグレーション記録のリスト
    pub async fn get_migrations(
        &self,
        pool: &AnyPool,
        dialect: Dialect,
    ) -> Result<Vec<MigrationRecord>, DatabaseError> {
        let sql = self.generate_get_migrations_sql(dialect);

        let rows = sqlx::query(&sql)
            .fetch_all(pool)
            .await
            .map_err(|e| DatabaseError::Query {
                message: format!("Failed to get migration history: {}", e),
                sql: Some(sql),
            })?;

        let records: Vec<MigrationRecord> = rows
            .iter()
            .map(|row| {
                let version: String = row.get(0);
                let description: String = row.get(1);
                let applied_at_str: String = row.get(2);
                let checksum: String = row.get(3);

                // RFC3339形式またはISO 8601形式の日時文字列をパース
                let applied_at = DateTime::parse_from_rfc3339(&applied_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                MigrationRecord {
                    version,
                    description,
                    applied_at,
                    checksum,
                }
            })
            .collect();

        Ok(records)
    }

    /// 特定バージョンのマイグレーション取得クエリを生成（パラメータバインド対応）
    ///
    /// # Security
    /// - テーブル名: `schema_migrations`固定（許可リスト検証済み）
    /// - パラメータ値: bind() でエスケープ
    ///
    /// # Arguments
    ///
    /// * `dialect` - データベース方言
    /// * `version` - 取得するマイグレーションのバージョン
    ///
    /// # Returns
    ///
    /// (SQL文字列, バインドパラメータのベクタ)
    pub fn generate_get_migration_by_version_query(
        &self,
        dialect: Dialect,
        version: &str,
    ) -> (String, Vec<String>) {
        let sql = match dialect {
            Dialect::PostgreSQL => {
                "SELECT version, description, applied_at::text AS applied_at, checksum FROM schema_migrations WHERE version = $1".to_string()
            }
            Dialect::MySQL => {
                "SELECT version, description, CAST(applied_at AS CHAR) AS applied_at, checksum FROM schema_migrations WHERE version = ?".to_string()
            }
            Dialect::SQLite => {
                "SELECT version, description, applied_at, checksum FROM schema_migrations WHERE version = ?".to_string()
            }
        };

        let params = vec![version.to_string()];

        (sql, params)
    }

    /// 指定されたバージョンのマイグレーション記録を取得（パラメータバインド対応）
    ///
    /// # Security
    /// パラメータバインディングを使用してSQLインジェクションを防止
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    /// * `dialect` - データベース方言
    /// * `version` - 取得するマイグレーションのバージョン
    ///
    /// # Returns
    ///
    /// マイグレーション記録（存在しない場合はNone）
    pub async fn get_migration_by_version_safe(
        &self,
        pool: &AnyPool,
        dialect: Dialect,
        version: &str,
    ) -> Result<Option<MigrationRecord>, DatabaseError> {
        let (sql, params) = self.generate_get_migration_by_version_query(dialect, version);

        let mut query = sqlx::query(&sql);
        for param in &params {
            query = query.bind(param);
        }

        let row_result = query
            .fetch_optional(pool)
            .await
            .map_err(|e| DatabaseError::Query {
                message: format!("Failed to get migration record: {}", e),
                sql: Some(sql),
            })?;

        if let Some(row) = row_result {
            let version: String = row.get(0);
            let description: String = row.get(1);
            let applied_at_str: String = row.get(2);
            let checksum: String = row.get(3);

            let applied_at = DateTime::parse_from_rfc3339(&applied_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Some(MigrationRecord {
                version,
                description,
                applied_at,
                checksum,
            }))
        } else {
            Ok(None)
        }
    }

    /// トランザクション開始SQLを生成
    ///
    /// # Returns
    ///
    /// BEGIN文のSQL文字列
    pub fn generate_begin_transaction_sql(&self) -> String {
        "BEGIN".to_string()
    }

    /// トランザクションコミットSQLを生成
    ///
    /// # Returns
    ///
    /// COMMIT文のSQL文字列
    pub fn generate_commit_transaction_sql(&self) -> String {
        "COMMIT".to_string()
    }

    /// トランザクションロールバックSQLを生成
    ///
    /// # Returns
    ///
    /// ROLLBACK文のSQL文字列
    pub fn generate_rollback_transaction_sql(&self) -> String {
        "ROLLBACK".to_string()
    }

    /// マイグレーションテーブル存在確認SQLを生成
    ///
    /// # Arguments
    ///
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// テーブル存在確認のSQL文字列
    pub fn generate_check_migration_table_exists_sql(&self, dialect: Dialect) -> String {
        match dialect {
            Dialect::PostgreSQL | Dialect::MySQL => {
                "SELECT table_name FROM information_schema.tables WHERE table_name = 'schema_migrations'"
                    .to_string()
            }
            Dialect::SQLite => {
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'"
                    .to_string()
            }
        }
    }

    /// マイグレーションテーブルが存在するか確認
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    /// * `dialect` - データベース方言
    ///
    /// # Returns
    ///
    /// テーブルが存在する場合はtrue、存在しない場合はfalse
    pub async fn migration_table_exists(
        &self,
        pool: &AnyPool,
        dialect: Dialect,
    ) -> Result<bool, DatabaseError> {
        let sql = self.generate_check_migration_table_exists_sql(dialect);

        let row_result =
            sqlx::query(&sql)
                .fetch_optional(pool)
                .await
                .map_err(|e| DatabaseError::Query {
                    message: format!("Failed to check migration table existence: {}", e),
                    sql: Some(sql),
                })?;

        Ok(row_result.is_some())
    }

    /// マイグレーションSQLを実行
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    /// * `sql` - 実行するSQL
    ///
    /// # Returns
    ///
    /// 実行に成功した場合はOk(())、失敗した場合はエラー
    pub async fn execute_migration_sql(
        &self,
        pool: &AnyPool,
        sql: &str,
    ) -> Result<AnyQueryResult, DatabaseError> {
        sqlx::query(sql)
            .execute(pool)
            .await
            .map_err(|e| DatabaseError::Query {
                message: format!("Failed to execute migration SQL: {}", e),
                sql: Some(sql.to_string()),
            })
    }

    /// カラムリネームSQLを実行（詳細エラー解析付き）
    ///
    /// リネームSQLの実行に失敗した場合、エラーメッセージを解析して
    /// 具体的な原因（カラム不存在、権限不足等）を特定します。
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    /// * `sql` - 実行するリネームSQL
    /// * `table_name` - 対象テーブル名
    /// * `old_name` - リネーム前のカラム名
    /// * `new_name` - リネーム後のカラム名
    ///
    /// # Returns
    ///
    /// 実行に成功した場合はOk(())、失敗した場合は詳細なエラー
    pub async fn execute_rename_column_sql(
        &self,
        pool: &AnyPool,
        sql: &str,
        table_name: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<AnyQueryResult, DatabaseError> {
        sqlx::query(sql).execute(pool).await.map_err(|e| {
            // エラーメッセージを解析して詳細なエラーを生成
            DatabaseError::parse_rename_error(&e.to_string(), table_name, old_name, new_name)
        })
    }
}

impl Default for DatabaseMigratorService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_service() {
        let service = DatabaseMigratorService::new();
        assert!(format!("{:?}", service).contains("DatabaseMigratorService"));
    }

    #[test]
    fn test_generate_create_migration_table_sql_postgres() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_create_migration_table_sql(Dialect::PostgreSQL);

        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("schema_migrations"));
        assert!(sql.contains("version"));
        assert!(sql.contains("applied_at"));
        assert!(sql.contains("checksum"));
    }

    #[test]
    fn test_generate_create_migration_table_sql_mysql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_create_migration_table_sql(Dialect::MySQL);

        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("schema_migrations"));
    }

    #[test]
    fn test_generate_create_migration_table_sql_sqlite() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_create_migration_table_sql(Dialect::SQLite);

        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("schema_migrations"));
    }

    #[test]
    fn test_generate_get_migrations_sql_postgres() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_get_migrations_sql(Dialect::PostgreSQL);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM schema_migrations"));
        assert!(sql.contains("ORDER BY"));
        assert!(sql.contains("version"));
        assert!(sql.contains("applied_at::text"));
    }

    #[test]
    fn test_generate_get_migrations_sql_mysql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_get_migrations_sql(Dialect::MySQL);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM schema_migrations"));
        assert!(sql.contains("ORDER BY"));
        assert!(sql.contains("version"));
        assert!(sql.contains("CAST(applied_at AS CHAR)"));
    }

    #[test]
    fn test_generate_get_migrations_sql_sqlite() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_get_migrations_sql(Dialect::SQLite);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM schema_migrations"));
        assert!(sql.contains("ORDER BY"));
        assert!(sql.contains("version"));
    }

    #[test]
    fn test_generate_begin_transaction_sql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_begin_transaction_sql();

        assert_eq!(sql, "BEGIN");
    }

    #[test]
    fn test_generate_commit_transaction_sql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_commit_transaction_sql();

        assert_eq!(sql, "COMMIT");
    }

    #[test]
    fn test_generate_rollback_transaction_sql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_rollback_transaction_sql();

        assert_eq!(sql, "ROLLBACK");
    }

    #[test]
    fn test_generate_check_migration_table_exists_sql_postgres() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_check_migration_table_exists_sql(Dialect::PostgreSQL);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("information_schema.tables"));
        assert!(sql.contains("table_name"));
        assert!(sql.contains("schema_migrations"));
    }

    #[test]
    fn test_generate_check_migration_table_exists_sql_mysql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_check_migration_table_exists_sql(Dialect::MySQL);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("information_schema.tables"));
        assert!(sql.contains("table_name"));
        assert!(sql.contains("schema_migrations"));
    }

    #[test]
    fn test_generate_check_migration_table_exists_sql_sqlite() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_check_migration_table_exists_sql(Dialect::SQLite);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("sqlite_master"));
        assert!(sql.contains("type = 'table'"));
        assert!(sql.contains("name"));
        assert!(sql.contains("schema_migrations"));
    }

    // Task 6.1: テーブル名検証のテスト
    #[test]
    fn test_validate_table_name_valid() {
        // 有効なテーブル名
        assert!(DatabaseMigratorService::validate_table_name("schema_migrations").is_ok());
        assert!(DatabaseMigratorService::validate_table_name("_migrations").is_ok());
        assert!(DatabaseMigratorService::validate_table_name("MyTable123").is_ok());
        assert!(DatabaseMigratorService::validate_table_name("a").is_ok());
        assert!(DatabaseMigratorService::validate_table_name(
            "a23456789012345678901234567890123456789012345678901234567890123"
        )
        .is_ok()); // 63文字
    }

    #[test]
    fn test_validate_table_name_invalid_start_with_number() {
        let result = DatabaseMigratorService::validate_table_name("123table");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_invalid_table_name());
    }

    #[test]
    fn test_validate_table_name_invalid_special_chars() {
        let result = DatabaseMigratorService::validate_table_name("table-name");
        assert!(result.is_err());

        let result = DatabaseMigratorService::validate_table_name("table.name");
        assert!(result.is_err());

        let result = DatabaseMigratorService::validate_table_name("table name");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_table_name_too_long() {
        // 64文字は長すぎる
        let long_name = "a".repeat(64);
        let result = DatabaseMigratorService::validate_table_name(&long_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_table_name_empty() {
        let result = DatabaseMigratorService::validate_table_name("");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_table_name_sql_injection_attempt() {
        // SQLインジェクション攻撃を試みるテーブル名
        let result = DatabaseMigratorService::validate_table_name("users; DROP TABLE users--");
        assert!(result.is_err());

        let result = DatabaseMigratorService::validate_table_name("users' OR '1'='1");
        assert!(result.is_err());
    }

    // Task 6.2: パラメータバインディングのテスト
    #[test]
    fn test_generate_record_migration_query_postgres() {
        let service = DatabaseMigratorService::new();
        let migration = Migration::new(
            "20240101120000".to_string(),
            "create_users_table".to_string(),
            "abc123def456".to_string(),
        );

        let (sql, params) =
            service.generate_record_migration_query(&migration, Dialect::PostgreSQL);

        // PostgreSQLはプレースホルダとして $1, $2, ... を使用
        assert!(sql.contains("$1"));
        assert!(sql.contains("$2"));
        assert!(sql.contains("$3"));
        assert!(sql.contains("$4"));
        assert!(!sql.contains("20240101120000")); // 値が直接埋め込まれていないことを確認
        assert_eq!(params.len(), 4);
        assert_eq!(params[0], "20240101120000");
        assert_eq!(params[1], "create_users_table");
    }

    #[test]
    fn test_generate_record_migration_query_mysql() {
        let service = DatabaseMigratorService::new();
        let migration = Migration::new(
            "20240101120000".to_string(),
            "create_users_table".to_string(),
            "abc123def456".to_string(),
        );

        let (sql, params) = service.generate_record_migration_query(&migration, Dialect::MySQL);

        // MySQLはプレースホルダとして ? を使用
        assert!(sql.contains("?"));
        assert!(!sql.contains("$1")); // PostgreSQL形式ではない
        assert!(!sql.contains("20240101120000")); // 値が直接埋め込まれていないことを確認
        assert_eq!(params.len(), 4);
    }

    #[test]
    fn test_generate_record_migration_query_sqlite() {
        let service = DatabaseMigratorService::new();
        let migration = Migration::new(
            "20240101120000".to_string(),
            "create_users_table".to_string(),
            "abc123def456".to_string(),
        );

        let (sql, params) = service.generate_record_migration_query(&migration, Dialect::SQLite);

        // SQLiteはプレースホルダとして ? を使用
        assert!(sql.contains("?"));
        assert!(!sql.contains("20240101120000")); // 値が直接埋め込まれていないことを確認
        assert_eq!(params.len(), 4);
    }

    #[test]
    fn test_generate_remove_migration_query_postgres() {
        let service = DatabaseMigratorService::new();
        let version = "20240101120000";

        let (sql, params) = service.generate_remove_migration_query(version, Dialect::PostgreSQL);

        assert!(sql.contains("$1"));
        assert!(!sql.contains("20240101120000"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "20240101120000");
    }

    #[test]
    fn test_generate_remove_migration_query_mysql() {
        let service = DatabaseMigratorService::new();
        let version = "20240101120000";

        let (sql, params) = service.generate_remove_migration_query(version, Dialect::MySQL);

        assert!(sql.contains("?"));
        assert!(!sql.contains("20240101120000"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_generate_get_migration_by_version_query_postgres() {
        let service = DatabaseMigratorService::new();
        let version = "20240101120000";

        let (sql, params) =
            service.generate_get_migration_by_version_query(Dialect::PostgreSQL, version);

        assert!(sql.contains("$1"));
        assert!(!sql.contains("20240101120000"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "20240101120000");
    }

    #[test]
    fn test_default_migration_table_name() {
        assert_eq!(DEFAULT_MIGRATION_TABLE, "schema_migrations");
    }
}
