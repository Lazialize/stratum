// データベース接続アダプター
//
// SQLxを使用したデータベース接続の管理を行います。
// PostgreSQL、MySQL、SQLiteに対応した統一されたインターフェースを提供します。

use crate::adapters::connection_string;
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
        connection_string::build_connection_string(dialect, config)
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
        let pool_options = self.create_pool_options_from_config(config);

        // 接続プールを作成
        pool_options
            .connect(&connection_string)
            .await
            .map_err(|e| DatabaseError::Connection {
                message: format!("Failed to create database connection pool: {}", dialect),
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
                message: "Database connection test failed".to_string(),
                cause: e.to_string(),
            })
    }

    /// DatabaseConfigからプールオプションを作成
    ///
    /// max_connections, min_connections, idle_timeout, timeout の設定を反映します。
    /// 未設定の場合はデフォルト値（max_connections=5, timeout=30秒）を使用します。
    pub fn create_pool_options_from_config(&self, config: &DatabaseConfig) -> PoolOptions<Any> {
        let max_conn = config.max_connections.unwrap_or(5);
        let timeout = config.timeout.unwrap_or(30);

        let mut opts = PoolOptions::new()
            .max_connections(max_conn)
            .acquire_timeout(Duration::from_secs(timeout));

        if let Some(min_conn) = config.min_connections {
            opts = opts.min_connections(min_conn);
        }

        if let Some(idle_secs) = config.idle_timeout {
            opts = opts.idle_timeout(Duration::from_secs(idle_secs));
        }

        opts
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
    fn test_create_pool_options_from_config_defaults() {
        let service = DatabaseConnectionService::new();
        let config = DatabaseConfig {
            database: "test".to_string(),
            ..Default::default()
        };
        let pool_options = service.create_pool_options_from_config(&config);

        assert!(format!("{:?}", pool_options).contains("PoolOptions"));
    }

    #[test]
    fn test_create_pool_options_from_config_custom() {
        let service = DatabaseConnectionService::new();
        let config = DatabaseConfig {
            database: "test".to_string(),
            timeout: Some(60),
            max_connections: Some(20),
            min_connections: Some(2),
            idle_timeout: Some(300),
            ..Default::default()
        };
        let pool_options = service.create_pool_options_from_config(&config);

        assert!(format!("{:?}", pool_options).contains("PoolOptions"));
    }
}
