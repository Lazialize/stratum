// 接続文字列ビルダー
//
// DatabaseConfig と Dialect から接続文字列を生成する。

use crate::core::config::{DatabaseConfig, Dialect};

/// 接続文字列を生成
pub fn build_connection_string(dialect: Dialect, config: &DatabaseConfig) -> String {
    match dialect {
        Dialect::PostgreSQL => {
            let user = config.user.as_deref().unwrap_or("postgres");
            let auth = match config.password.as_deref() {
                Some(password) if !password.is_empty() => format!("{}:{}", user, password),
                _ => user.to_string(),
            };
            format!(
                "postgresql://{}@{}:{}/{}",
                auth, config.host, config.port, config.database
            )
        }
        Dialect::MySQL => {
            let user = config.user.as_deref().unwrap_or("root");
            let auth = match config.password.as_deref() {
                Some(password) if !password.is_empty() => format!("{}:{}", user, password),
                _ => user.to_string(),
            };
            format!(
                "mysql://{}@{}:{}/{}",
                auth, config.host, config.port, config.database
            )
        }
        Dialect::SQLite => format!("sqlite://{}", config.database),
    }
}
