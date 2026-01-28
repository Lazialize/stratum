// 接続文字列ビルダー
//
// DatabaseConfig と Dialect から接続文字列を生成する。

use crate::core::config::{DatabaseConfig, Dialect};
use urlencoding::encode;

/// 接続文字列を生成
///
/// ユーザー名とパスワードはパーセントエンコードされます。
/// これにより、`@`, `:`, `/`, `#`, `?` などの特殊文字を含むパスワードでも
/// 正しく接続文字列が構築されます。
pub fn build_connection_string(dialect: Dialect, config: &DatabaseConfig) -> String {
    match dialect {
        Dialect::PostgreSQL => {
            let user = config.user.as_deref().unwrap_or("postgres");
            let encoded_user = encode(user);
            let auth = match config.password.as_deref() {
                Some(password) if !password.is_empty() => {
                    let encoded_password = encode(password);
                    format!("{}:{}", encoded_user, encoded_password)
                }
                _ => encoded_user.to_string(),
            };
            let port = config.resolved_port(dialect);
            format!(
                "postgresql://{}@{}:{}/{}",
                auth, config.host, port, config.database
            )
        }
        Dialect::MySQL => {
            let user = config.user.as_deref().unwrap_or("root");
            let encoded_user = encode(user);
            let auth = match config.password.as_deref() {
                Some(password) if !password.is_empty() => {
                    let encoded_password = encode(password);
                    format!("{}:{}", encoded_user, encoded_password)
                }
                _ => encoded_user.to_string(),
            };
            let port = config.resolved_port(dialect);
            format!(
                "mysql://{}@{}:{}/{}",
                auth, config.host, port, config.database
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
            port: Some(5432),
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
            port: Some(3306),
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
            port: None,
            database: "/path/to/test.db".to_string(),
            user: None,
            password: None,
            timeout: None,
        };

        let conn_str = build_connection_string(Dialect::SQLite, &config);

        assert!(conn_str.contains("sqlite://"));
        assert!(conn_str.contains("/path/to/test.db"));
    }

    #[test]
    fn test_build_connection_string_postgres_special_chars_in_password() {
        // パスワードに @, :, /, #, ? などの特殊文字を含むケース
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: Some(5432),
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("p@ss:word/test#query?".to_string()),
            timeout: None,
        };

        let conn_str = build_connection_string(Dialect::PostgreSQL, &config);

        // 特殊文字がエンコードされていることを確認
        assert!(conn_str.contains("postgresql://"));
        assert!(conn_str.contains("testuser"));
        // @ は %40, : は %3A, / は %2F, # は %23, ? は %3F にエンコードされる
        assert!(conn_str.contains("p%40ss%3Aword%2Ftest%23query%3F"));
        assert!(conn_str.contains("localhost:5432/testdb"));
    }

    #[test]
    fn test_build_connection_string_mysql_special_chars_in_password() {
        // パスワードに @, :, /, #, ? などの特殊文字を含むケース
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: Some(3306),
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("p@ss:word".to_string()),
            timeout: None,
        };

        let conn_str = build_connection_string(Dialect::MySQL, &config);

        assert!(conn_str.contains("mysql://"));
        assert!(conn_str.contains("testuser"));
        assert!(conn_str.contains("p%40ss%3Aword"));
    }

    #[test]
    fn test_build_connection_string_special_chars_in_username() {
        // ユーザー名に特殊文字を含むケース
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: Some(5432),
            database: "testdb".to_string(),
            user: Some("user@domain".to_string()),
            password: Some("password".to_string()),
            timeout: None,
        };

        let conn_str = build_connection_string(Dialect::PostgreSQL, &config);

        assert!(conn_str.contains("user%40domain:password@localhost"));
    }
}
