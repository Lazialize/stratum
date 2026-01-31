// 設定ファイル管理
//
// プロジェクトの設定ファイル（YAML形式）の読み込み、検証、
// 環境別のデータベース接続設定の管理を行います。

use crate::core::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// SSL接続モード
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SslMode {
    Disable,
    #[serde(rename = "prefer")]
    Prefer,
    Require,
    #[serde(rename = "verify-ca")]
    VerifyCa,
    #[serde(rename = "verify-full")]
    VerifyFull,
}

impl std::fmt::Display for SslMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SslMode::Disable => write!(f, "disable"),
            SslMode::Prefer => write!(f, "prefer"),
            SslMode::Require => write!(f, "require"),
            SslMode::VerifyCa => write!(f, "verify-ca"),
            SslMode::VerifyFull => write!(f, "verify-full"),
        }
    }
}

/// データベース方言
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dialect {
    #[serde(rename = "postgresql")]
    PostgreSQL,
    #[serde(rename = "mysql")]
    MySQL,
    #[serde(rename = "sqlite")]
    SQLite,
}

impl std::fmt::Display for Dialect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dialect::PostgreSQL => write!(f, "postgresql"),
            Dialect::MySQL => write!(f, "mysql"),
            Dialect::SQLite => write!(f, "sqlite"),
        }
    }
}

impl Dialect {
    /// Dialectに応じたデフォルトポートを返す
    ///
    /// - PostgreSQL: 5432
    /// - MySQL: 3306
    /// - SQLite: None（ファイルベースのためポート不要）
    pub fn default_port(&self) -> Option<u16> {
        match self {
            Dialect::PostgreSQL => Some(5432),
            Dialect::MySQL => Some(3306),
            Dialect::SQLite => None,
        }
    }
}

/// プロジェクト設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 設定ファイルのバージョン
    pub version: String,

    /// データベース方言
    pub dialect: Dialect,

    /// スキーマ定義ディレクトリ
    #[serde(default = "default_schema_dir")]
    pub schema_dir: PathBuf,

    /// マイグレーションディレクトリ
    #[serde(default = "default_migrations_dir")]
    pub migrations_dir: PathBuf,

    /// 環境別のデータベース設定
    pub environments: HashMap<String, DatabaseConfig>,
}

fn default_schema_dir() -> PathBuf {
    PathBuf::from("schema")
}

fn default_migrations_dir() -> PathBuf {
    PathBuf::from("migrations")
}

impl Config {
    /// デフォルトの設定ファイルパス
    pub const DEFAULT_CONFIG_PATH: &'static str = crate::core::naming::CONFIG_FILE;

    /// 指定された環境のデータベース設定を取得
    pub fn get_database_config(&self, environment: &str) -> Result<DatabaseConfig, ConfigError> {
        self.environments.get(environment).cloned().ok_or_else(|| {
            ConfigError::EnvironmentNotFound {
                name: environment.to_string(),
                available: self.environments.keys().cloned().collect(),
            }
        })
    }

    /// 設定の妥当性を検証
    pub fn validate(&self) -> Result<(), ConfigError> {
        // バージョンチェック
        if self.version.is_empty() {
            return Err(ConfigError::MissingVersion);
        }

        // 環境設定チェック
        if self.environments.is_empty() {
            return Err(ConfigError::NoEnvironments);
        }

        // 各環境のデータベース設定を検証
        for (env_name, db_config) in &self.environments {
            db_config
                .validate()
                .map_err(|source| ConfigError::InvalidEnvironment {
                    environment: env_name.clone(),
                    source: Box::new(source),
                })?;
        }

        Ok(())
    }
}

/// データベース接続設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// ホスト名（SQLiteの場合は不要）
    #[serde(default = "default_host", skip_serializing_if = "String::is_empty")]
    pub host: String,

    /// ポート番号（Noneの場合はDialectのデフォルトポートを使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// データベース名
    pub database: String,

    /// ユーザー名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// パスワード
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// 接続タイムアウト（秒）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,

    /// SSL接続モード
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssl_mode: Option<SslMode>,

    /// 最大コネクション数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<u32>,

    /// 最小コネクション数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_connections: Option<u32>,

    /// アイドルタイムアウト（秒）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<u64>,

    /// 追加接続オプション（クエリパラメータとして付与）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, String>>,
}

fn default_host() -> String {
    "localhost".to_string()
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: None,
            database: String::new(),
            user: None,
            password: None,
            timeout: None,
            ssl_mode: None,
            max_connections: None,
            min_connections: None,
            idle_timeout: None,
            options: None,
        }
    }
}

impl DatabaseConfig {
    /// Dialectに応じた解決済みポート番号を取得
    ///
    /// portがSomeの場合はその値を返し、Noneの場合はDialectのデフォルトポートを返します。
    /// SQLiteなどデフォルトポートがないDialectの場合は0を返します。
    pub fn resolved_port(&self, dialect: Dialect) -> u16 {
        self.port
            .unwrap_or_else(|| dialect.default_port().unwrap_or(0))
    }

    /// Validate database configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.database.is_empty() {
            return Err(ConfigError::MissingDatabaseName);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialect_display() {
        assert_eq!(Dialect::PostgreSQL.to_string(), "postgresql");
        assert_eq!(Dialect::MySQL.to_string(), "mysql");
        assert_eq!(Dialect::SQLite.to_string(), "sqlite");
    }

    #[test]
    fn test_dialect_default_port() {
        assert_eq!(Dialect::PostgreSQL.default_port(), Some(5432));
        assert_eq!(Dialect::MySQL.default_port(), Some(3306));
        assert_eq!(Dialect::SQLite.default_port(), None);
    }

    #[test]
    fn test_resolved_port_with_explicit_port() {
        let config = DatabaseConfig {
            port: Some(5433),
            database: "test".to_string(),
            ..Default::default()
        };

        // 明示的に設定したポートは常にその値を返す
        assert_eq!(config.resolved_port(Dialect::PostgreSQL), 5433);
        assert_eq!(config.resolved_port(Dialect::MySQL), 5433);
    }

    #[test]
    fn test_resolved_port_without_explicit_port() {
        let config = DatabaseConfig {
            database: "test".to_string(),
            ..Default::default()
        };

        // Noneの場合はDialectのデフォルトポートを返す
        assert_eq!(config.resolved_port(Dialect::PostgreSQL), 5432);
        assert_eq!(config.resolved_port(Dialect::MySQL), 3306);
        assert_eq!(config.resolved_port(Dialect::SQLite), 0);
    }

    #[test]
    fn test_explicit_port_5432_for_mysql_not_overwritten() {
        // ユーザーが意図的にMySQLにポート5432を設定した場合、上書きされない
        let config = DatabaseConfig {
            port: Some(5432),
            database: "test".to_string(),
            ..Default::default()
        };

        assert_eq!(config.resolved_port(Dialect::MySQL), 5432);
    }
}
