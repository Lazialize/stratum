// データベースマイグレーターサービス
//
// マイグレーション履歴テーブルの管理とトランザクション制御を担当するサービス。
// データベース固有のSQL構文を抽象化し、マイグレーション適用とロールバックをサポートします。

use crate::core::config::Dialect;
use crate::core::error::DatabaseError;
use crate::core::migration::{Migration, MigrationRecord};
use chrono::{DateTime, Utc};
use sqlx::{any::AnyQueryResult, AnyPool, Row};

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
            Dialect::PostgreSQL => {
                r#"CREATE TABLE IF NOT EXISTS schema_migrations (
    version VARCHAR(255) PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    checksum VARCHAR(64) NOT NULL
)"#
                .to_string()
            }
            Dialect::MySQL => {
                r#"CREATE TABLE IF NOT EXISTS schema_migrations (
    version VARCHAR(255) PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    checksum VARCHAR(64) NOT NULL
)"#
                .to_string()
            }
            Dialect::SQLite => {
                r#"CREATE TABLE IF NOT EXISTS schema_migrations (
    version TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now')),
    checksum TEXT NOT NULL
)"#
                .to_string()
            }
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
                message: format!("マイグレーション履歴テーブルの作成に失敗しました: {}", e),
                sql: Some(sql),
            })?;

        Ok(())
    }

    /// マイグレーション記録のINSERT SQLを生成
    ///
    /// # Arguments
    ///
    /// * `migration` - 記録するマイグレーション
    ///
    /// # Returns
    ///
    /// INSERT文のSQL文字列
    pub fn generate_record_migration_sql(&self, migration: &Migration) -> String {
        format!(
            "INSERT INTO schema_migrations (version, description, applied_at, checksum) VALUES ('{}', '{}', '{}', '{}')",
            migration.version,
            migration.description.replace('\'', "''"),
            migration.timestamp.to_rfc3339(),
            migration.checksum
        )
    }

    /// マイグレーション記録をデータベースに保存
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    /// * `migration` - 記録するマイグレーション
    ///
    /// # Returns
    ///
    /// 保存に成功した場合はOk(())、失敗した場合はエラー
    pub async fn record_migration(
        &self,
        pool: &AnyPool,
        migration: &Migration,
    ) -> Result<(), DatabaseError> {
        let sql = self.generate_record_migration_sql(migration);

        sqlx::query(&sql)
            .execute(pool)
            .await
            .map_err(|e| DatabaseError::Query {
                message: format!("マイグレーション記録の保存に失敗しました: {}", e),
                sql: Some(sql),
            })?;

        Ok(())
    }

    /// マイグレーション削除のDELETE SQLを生成
    ///
    /// # Arguments
    ///
    /// * `version` - 削除するマイグレーションのバージョン
    ///
    /// # Returns
    ///
    /// DELETE文のSQL文字列
    pub fn generate_remove_migration_sql(&self, version: &str) -> String {
        format!(
            "DELETE FROM schema_migrations WHERE version = '{}'",
            version
        )
    }

    /// マイグレーション記録をデータベースから削除
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    /// * `version` - 削除するマイグレーションのバージョン
    ///
    /// # Returns
    ///
    /// 削除に成功した場合はOk(())、失敗した場合はエラー
    pub async fn remove_migration(
        &self,
        pool: &AnyPool,
        version: &str,
    ) -> Result<(), DatabaseError> {
        let sql = self.generate_remove_migration_sql(version);

        sqlx::query(&sql)
            .execute(pool)
            .await
            .map_err(|e| DatabaseError::Query {
                message: format!("マイグレーション記録の削除に失敗しました: {}", e),
                sql: Some(sql),
            })?;

        Ok(())
    }

    /// マイグレーション履歴取得のSELECT SQLを生成
    ///
    /// # Returns
    ///
    /// SELECT文のSQL文字列
    pub fn generate_get_migrations_sql(&self) -> String {
        "SELECT version, description, applied_at, checksum FROM schema_migrations ORDER BY version"
            .to_string()
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
    ) -> Result<Vec<MigrationRecord>, DatabaseError> {
        let sql = self.generate_get_migrations_sql();

        let rows = sqlx::query(&sql)
            .fetch_all(pool)
            .await
            .map_err(|e| DatabaseError::Query {
                message: format!("マイグレーション履歴の取得に失敗しました: {}", e),
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

    /// 特定バージョンのマイグレーション取得SQLを生成
    ///
    /// # Arguments
    ///
    /// * `version` - 取得するマイグレーションのバージョン
    ///
    /// # Returns
    ///
    /// SELECT文のSQL文字列
    pub fn generate_get_migration_by_version_sql(&self, version: &str) -> String {
        format!(
            "SELECT version, description, applied_at, checksum FROM schema_migrations WHERE version = '{}'",
            version
        )
    }

    /// 指定されたバージョンのマイグレーション記録を取得
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    /// * `version` - 取得するマイグレーションのバージョン
    ///
    /// # Returns
    ///
    /// マイグレーション記録（存在しない場合はNone）
    pub async fn get_migration_by_version(
        &self,
        pool: &AnyPool,
        version: &str,
    ) -> Result<Option<MigrationRecord>, DatabaseError> {
        let sql = self.generate_get_migration_by_version_sql(version);

        let row_result = sqlx::query(&sql).fetch_optional(pool).await.map_err(|e| {
            DatabaseError::Query {
                message: format!("マイグレーション記録の取得に失敗しました: {}", e),
                sql: Some(sql),
            }
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

        let row_result = sqlx::query(&sql).fetch_optional(pool).await.map_err(|e| {
            DatabaseError::Query {
                message: format!("マイグレーションテーブルの存在確認に失敗しました: {}", e),
                sql: Some(sql),
            }
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
                message: format!("マイグレーションSQLの実行に失敗しました: {}", e),
                sql: Some(sql.to_string()),
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
    fn test_generate_record_migration_sql() {
        let service = DatabaseMigratorService::new();
        let migration = Migration::new(
            "20240101120000".to_string(),
            "create_users_table".to_string(),
            "abc123def456".to_string(),
        );

        let sql = service.generate_record_migration_sql(&migration);

        assert!(sql.contains("INSERT INTO schema_migrations"));
        assert!(sql.contains("20240101120000"));
        assert!(sql.contains("create_users_table"));
        assert!(sql.contains("abc123def456"));
    }

    #[test]
    fn test_generate_remove_migration_sql() {
        let service = DatabaseMigratorService::new();
        let version = "20240101120000";

        let sql = service.generate_remove_migration_sql(version);

        assert!(sql.contains("DELETE FROM schema_migrations"));
        assert!(sql.contains("WHERE version ="));
        assert!(sql.contains("20240101120000"));
    }

    #[test]
    fn test_generate_get_migrations_sql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_get_migrations_sql();

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
    fn test_generate_get_migration_by_version_sql() {
        let service = DatabaseMigratorService::new();
        let version = "20240101120000";

        let sql = service.generate_get_migration_by_version_sql(version);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM schema_migrations"));
        assert!(sql.contains("WHERE version ="));
        assert!(sql.contains("20240101120000"));
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
}
