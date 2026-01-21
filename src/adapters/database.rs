// データベース接続アダプター
//
// SQLxを使用したデータベース接続の管理を行います。
// PostgreSQL、MySQL、SQLiteに対応した統一されたインターフェースを提供します。

use crate::core::config::{DatabaseConfig, Dialect};
use crate::core::error::DatabaseError;
use sqlx::pool::PoolOptions;
use sqlx::{Any, AnyPool};
use std::time::Duration;

/// データベース接続サービス
///
/// データベース接続プールの初期化と管理を行います。
#[derive(Debug, Clone)]
pub struct DatabaseConnectionService {
    // 将来的な拡張のためのフィールドを予約
}

impl DatabaseConnectionService {
    /// 新しいDatabaseConnectionServiceを作成
    pub fn new() -> Self {
        Self {}
    }

    /// データベース接続文字列を構築
    ///
    /// # Arguments
    ///
    /// * `dialect` - データベース方言
    /// * `config` - データベース設定
    ///
    /// # Returns
    ///
    /// 接続文字列
    pub fn build_connection_string(&self, dialect: Dialect, config: &DatabaseConfig) -> String {
        config.to_connection_string(dialect)
    }

    /// データベース接続プールを作成
    ///
    /// # Arguments
    ///
    /// * `dialect` - データベース方言
    /// * `config` - データベース設定
    ///
    /// # Returns
    ///
    /// 接続プールまたはエラー
    pub async fn create_pool(
        &self,
        dialect: Dialect,
        config: &DatabaseConfig,
    ) -> Result<AnyPool, DatabaseError> {
        let connection_string = self.build_connection_string(dialect, config);

        // プールオプションを作成
        let pool_options = if let Some(timeout_secs) = config.timeout {
            self.create_pool_options_with_timeout(Some(timeout_secs))
        } else {
            self.create_pool_options()
        };

        // 接続プールを作成
        pool_options
            .connect(&connection_string)
            .await
            .map_err(|e| DatabaseError::Connection {
                message: format!("データベース接続プールの作成に失敗しました: {}", dialect),
                cause: e.to_string(),
            })
    }

    /// 接続テストを実行
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    ///
    /// # Returns
    ///
    /// 成功した場合はOk、失敗した場合はErr
    pub async fn test_connection(&self, pool: &AnyPool) -> Result<(), DatabaseError> {
        // シンプルなクエリで接続をテスト
        sqlx::query("SELECT 1")
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| DatabaseError::Connection {
                message: "データベース接続テストに失敗しました".to_string(),
                cause: e.to_string(),
            })
    }

    /// デフォルトのプールオプションを作成
    ///
    /// # Returns
    ///
    /// プールオプション
    pub fn create_pool_options(&self) -> PoolOptions<Any> {
        PoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(30))
    }

    /// タイムアウト付きのプールオプションを作成
    ///
    /// # Arguments
    ///
    /// * `timeout_secs` - タイムアウト秒数
    ///
    /// # Returns
    ///
    /// プールオプション
    pub fn create_pool_options_with_timeout(
        &self,
        timeout_secs: Option<u64>,
    ) -> PoolOptions<Any> {
        let timeout = timeout_secs.unwrap_or(30);
        PoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(timeout))
    }

    /// 接続プールを閉じる
    ///
    /// # Arguments
    ///
    /// * `pool` - データベース接続プール
    pub async fn close_pool(&self, pool: AnyPool) {
        pool.close().await;
    }
}

impl Default for DatabaseConnectionService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_service() {
        let service = DatabaseConnectionService::new();
        assert!(format!("{:?}", service).contains("DatabaseConnectionService"));
    }

    #[test]
    fn test_build_connection_string_postgres() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            timeout: None,
        };

        let service = DatabaseConnectionService::new();
        let conn_str = service.build_connection_string(Dialect::PostgreSQL, &config);

        assert!(conn_str.contains("postgresql://"));
        assert!(conn_str.contains("testuser"));
        assert!(conn_str.contains("testpass"));
        assert!(conn_str.contains("localhost"));
        assert!(conn_str.contains("5432"));
        assert!(conn_str.contains("testdb"));
    }

    #[test]
    fn test_build_connection_string_mysql() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 3306,
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            timeout: None,
        };

        let service = DatabaseConnectionService::new();
        let conn_str = service.build_connection_string(Dialect::MySQL, &config);

        assert!(conn_str.contains("mysql://"));
        assert!(conn_str.contains("testuser"));
        assert!(conn_str.contains("localhost"));
        assert!(conn_str.contains("3306"));
        assert!(conn_str.contains("testdb"));
    }

    #[test]
    fn test_build_connection_string_sqlite() {
        let config = DatabaseConfig {
            host: "".to_string(),
            port: 0,
            database: "/path/to/test.db".to_string(),
            user: None,
            password: None,
            timeout: None,
        };

        let service = DatabaseConnectionService::new();
        let conn_str = service.build_connection_string(Dialect::SQLite, &config);

        assert!(conn_str.contains("sqlite://"));
        assert!(conn_str.contains("/path/to/test.db"));
    }

    #[test]
    fn test_create_pool_options() {
        let service = DatabaseConnectionService::new();
        let pool_options = service.create_pool_options();

        assert!(format!("{:?}", pool_options).contains("PoolOptions"));
    }

    #[test]
    fn test_create_pool_options_with_timeout() {
        let service = DatabaseConnectionService::new();
        let pool_options = service.create_pool_options_with_timeout(Some(60));

        assert!(format!("{:?}", pool_options).contains("PoolOptions"));
    }
}
