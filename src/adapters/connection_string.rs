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

#[cfg(test)]
mod tests {
    use super::*;

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

        let conn_str = build_connection_string(Dialect::PostgreSQL, &config);

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

        let conn_str = build_connection_string(Dialect::MySQL, &config);

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

        let conn_str = build_connection_string(Dialect::SQLite, &config);

        assert!(conn_str.contains("sqlite://"));
        assert!(conn_str.contains("/path/to/test.db"));
    }
}
