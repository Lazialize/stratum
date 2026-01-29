// 接続文字列ビルダー
//
// DatabaseConfig と Dialect から接続文字列を生成する。

use crate::core::config::{DatabaseConfig, Dialect};
use std::collections::BTreeMap;
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
            let base = format!(
                "postgresql://{}@{}:{}/{}",
                auth, config.host, port, config.database
            );
            append_query_params(base, config)
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
            let base = format!(
                "mysql://{}@{}:{}/{}",
                auth, config.host, port, config.database
            );
            append_query_params(base, config)
        }
        Dialect::SQLite => format!("sqlite://{}", config.database),
    }
}

/// 接続文字列にクエリパラメータ（ssl_mode, options）を付与
fn append_query_params(base: String, config: &DatabaseConfig) -> String {
    // BTreeMapで順序を安定させる
    let mut params = BTreeMap::new();

    if let Some(ref ssl_mode) = config.ssl_mode {
        params.insert("sslmode".to_string(), ssl_mode.to_string());
    }

    if let Some(ref options) = config.options {
        for (key, value) in options {
            // ssl_modeフィールドが優先されるため、optionsのsslmodeは無視
            if key != "sslmode" || config.ssl_mode.is_none() {
                params.insert(key.clone(), value.clone());
            }
        }
    }

    if params.is_empty() {
        return base;
    }

    let query: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect();

    format!("{}?{}", base, query.join("&"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_connection_string_postgres() {
        let config = DatabaseConfig {
            port: Some(5432),
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            ..Default::default()
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
            port: Some(3306),
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            ..Default::default()
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
            database: "/path/to/test.db".to_string(),
            ..Default::default()
        };

        let conn_str = build_connection_string(Dialect::SQLite, &config);

        assert!(conn_str.contains("sqlite://"));
        assert!(conn_str.contains("/path/to/test.db"));
    }

    #[test]
    fn test_build_connection_string_postgres_special_chars_in_password() {
        // パスワードに @, :, /, #, ? などの特殊文字を含むケース
        let config = DatabaseConfig {
            port: Some(5432),
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("p@ss:word/test#query?".to_string()),
            ..Default::default()
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
            port: Some(3306),
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("p@ss:word".to_string()),
            ..Default::default()
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
            port: Some(5432),
            database: "testdb".to_string(),
            user: Some("user@domain".to_string()),
            password: Some("password".to_string()),
            ..Default::default()
        };

        let conn_str = build_connection_string(Dialect::PostgreSQL, &config);

        assert!(conn_str.contains("user%40domain:password@localhost"));
    }

    #[test]
    fn test_build_connection_string_with_ssl_mode() {
        use crate::core::config::SslMode;

        let config = DatabaseConfig {
            port: Some(5432),
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            ssl_mode: Some(SslMode::Require),
            ..Default::default()
        };

        let conn_str = build_connection_string(Dialect::PostgreSQL, &config);

        assert!(conn_str.contains("?sslmode=require"));
    }

    #[test]
    fn test_build_connection_string_with_options() {
        let mut opts = std::collections::HashMap::new();
        opts.insert("application_name".to_string(), "stratum".to_string());
        opts.insert("connect_timeout".to_string(), "10".to_string());

        let config = DatabaseConfig {
            port: Some(5432),
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            options: Some(opts),
            ..Default::default()
        };

        let conn_str = build_connection_string(Dialect::PostgreSQL, &config);

        assert!(conn_str.contains("application_name=stratum"));
        assert!(conn_str.contains("connect_timeout=10"));
        assert!(conn_str.contains('?'));
        assert!(conn_str.contains('&'));
    }

    #[test]
    fn test_build_connection_string_ssl_mode_overrides_options_sslmode() {
        use crate::core::config::SslMode;

        let mut opts = std::collections::HashMap::new();
        opts.insert("sslmode".to_string(), "disable".to_string());

        let config = DatabaseConfig {
            port: Some(5432),
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            ssl_mode: Some(SslMode::Require),
            options: Some(opts),
            ..Default::default()
        };

        let conn_str = build_connection_string(Dialect::PostgreSQL, &config);

        // ssl_modeフィールドが優先される
        assert!(conn_str.contains("sslmode=require"));
        assert!(!conn_str.contains("sslmode=disable"));
    }

    #[test]
    fn test_build_connection_string_sqlite_ignores_options() {
        use crate::core::config::SslMode;

        let config = DatabaseConfig {
            host: "".to_string(),
            database: "/path/to/test.db".to_string(),
            ssl_mode: Some(SslMode::Require),
            ..Default::default()
        };

        let conn_str = build_connection_string(Dialect::SQLite, &config);

        // SQLiteはクエリパラメータを付与しない
        assert_eq!(conn_str, "sqlite:///path/to/test.db");
    }
}
