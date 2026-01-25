// 設定ファイル管理
//
// プロジェクトの設定ファイル（YAML形式）の読み込み、検証、
// 環境別のデータベース接続設定の管理を行います。

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

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
    pub fn get_database_config(&self, environment: &str) -> Result<DatabaseConfig> {
        self.environments.get(environment).cloned().ok_or_else(|| {
            anyhow!(
                "Environment '{}' not found. Available environments: {:?}",
                environment,
                self.environments.keys().collect::<Vec<_>>()
            )
        })
    }

    /// 設定の妥当性を検証
    pub fn validate(&self) -> Result<()> {
        // バージョンチェック
        if self.version.is_empty() {
            return Err(anyhow!("Config file version is not specified"));
        }

        // 環境設定チェック
        if self.environments.is_empty() {
            return Err(anyhow!(
                "At least one environment configuration is required"
            ));
        }

        // 各環境のデータベース設定を検証
        for (env_name, db_config) in &self.environments {
            db_config
                .validate()
                .with_context(|| format!("Invalid config for environment '{}'", env_name))?;
        }

        Ok(())
    }
}

/// std::str::FromStrトレイトの実装
impl FromStr for Config {
    type Err = anyhow::Error;

    fn from_str(yaml: &str) -> Result<Self, Self::Err> {
        serde_saphyr::from_str(yaml).with_context(|| "Failed to parse config file")
    }
}

/// データベース接続設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// ホスト名（SQLiteの場合は不要）
    #[serde(default = "default_host")]
    pub host: String,

    /// ポート番号
    #[serde(default = "default_port")]
    pub port: u16,

    /// データベース名
    pub database: String,

    /// ユーザー名
    pub user: Option<String>,

    /// パスワード
    pub password: Option<String>,

    /// 接続タイムアウト（秒）
    pub timeout: Option<u64>,
}

fn default_host() -> String {
    "localhost".to_string()
}

fn default_port() -> u16 {
    5432 // PostgreSQLのデフォルトポート
}

impl DatabaseConfig {
    /// 環境変数から設定を上書き
    pub fn merge_with_env(&self) -> Self {
        let mut config = self.clone();

        // 環境変数から値を取得して上書き
        if let Ok(host) = std::env::var("DB_HOST") {
            config.host = host;
        }
        if let Ok(port) = std::env::var("DB_PORT") {
            if let Ok(port_num) = port.parse::<u16>() {
                config.port = port_num;
            }
        }
        if let Ok(database) = std::env::var("DB_DATABASE") {
            config.database = database;
        }
        if let Ok(user) = std::env::var("DB_USER") {
            config.user = Some(user);
        }
        if let Ok(password) = std::env::var("DB_PASSWORD") {
            config.password = Some(password);
        }

        config
    }

    /// Validate database configuration
    pub fn validate(&self) -> Result<()> {
        if self.database.is_empty() {
            return Err(anyhow!("Database name is not specified"));
        }

        Ok(())
    }

    /// Generate connection string (for PostgreSQL)
    pub fn to_connection_string(&self, dialect: Dialect) -> String {
        match dialect {
            Dialect::PostgreSQL => {
                let user = self.user.as_deref().unwrap_or("postgres");
                let auth = match self.password.as_deref() {
                    Some(password) if !password.is_empty() => format!("{}:{}", user, password),
                    _ => user.to_string(),
                };
                format!(
                    "postgresql://{}@{}:{}/{}",
                    auth, self.host, self.port, self.database
                )
            }
            Dialect::MySQL => {
                let user = self.user.as_deref().unwrap_or("root");
                let auth = match self.password.as_deref() {
                    Some(password) if !password.is_empty() => format!("{}:{}", user, password),
                    _ => user.to_string(),
                };
                format!(
                    "mysql://{}@{}:{}/{}",
                    auth, self.host, self.port, self.database
                )
            }
            Dialect::SQLite => {
                format!("sqlite://{}", self.database)
            }
        }
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
    fn test_connection_string_postgresql() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            timeout: None,
        };

        let conn_str = config.to_connection_string(Dialect::PostgreSQL);
        assert_eq!(
            conn_str,
            "postgresql://testuser:testpass@localhost:5432/testdb"
        );
    }

    #[test]
    fn test_connection_string_mysql() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 3306,
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            timeout: None,
        };

        let conn_str = config.to_connection_string(Dialect::MySQL);
        assert_eq!(conn_str, "mysql://testuser:testpass@localhost:3306/testdb");
    }

    #[test]
    fn test_connection_string_sqlite() {
        let config = DatabaseConfig {
            host: "".to_string(),
            port: 0,
            database: "/path/to/db.sqlite".to_string(),
            user: None,
            password: None,
            timeout: None,
        };

        let conn_str = config.to_connection_string(Dialect::SQLite);
        assert_eq!(conn_str, "sqlite:///path/to/db.sqlite");
    }
}
