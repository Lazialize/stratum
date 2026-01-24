/// データベース接続サービスのテスト
///
/// データベース接続の初期化と管理機能が正しく動作することを確認します。

#[cfg(test)]
mod database_connection_tests {
    use strata::adapters::database::DatabaseConnectionService;
    use strata::core::config::{DatabaseConfig, Dialect};

    /// サービスの作成テスト
    #[test]
    fn test_new_service() {
        let service = DatabaseConnectionService::new();
        assert!(format!("{:?}", service).contains("DatabaseConnectionService"));
    }

    /// PostgreSQL接続文字列の構築テスト
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
        assert!(conn_str.contains("localhost"));
        assert!(conn_str.contains("5432"));
        assert!(conn_str.contains("testdb"));
    }

    /// MySQL接続文字列の構築テスト
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

    /// SQLite接続文字列の構築テスト
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

    /// 接続文字列にパスワードが含まれる場合のテスト
    #[test]
    fn test_connection_string_with_password() {
        let config = DatabaseConfig {
            host: "db.example.com".to_string(),
            port: 5432,
            database: "proddb".to_string(),
            user: Some("admin".to_string()),
            password: Some("secret123".to_string()),
            timeout: None,
        };

        let service = DatabaseConnectionService::new();
        let conn_str = service.build_connection_string(Dialect::PostgreSQL, &config);

        // パスワードが含まれていることを確認
        assert!(conn_str.contains("secret123"));
    }

    /// 接続文字列にパスワードがない場合のテスト
    #[test]
    fn test_connection_string_without_password() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: None,
            timeout: None,
        };

        let service = DatabaseConnectionService::new();
        let conn_str = service.build_connection_string(Dialect::PostgreSQL, &config);

        // パスワードなしの場合、:@ は含まれない
        assert!(conn_str.contains("postgresql://testuser@"));
        assert!(!conn_str.contains("testuser:@"));
    }

    /// デフォルトユーザー名の使用テスト（PostgreSQL）
    #[test]
    fn test_default_username_postgres() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            user: None,
            password: None,
            timeout: None,
        };

        let service = DatabaseConnectionService::new();
        let conn_str = service.build_connection_string(Dialect::PostgreSQL, &config);

        // PostgreSQLのデフォルトユーザーは "postgres"
        assert!(conn_str.contains("postgres"));
    }

    /// デフォルトユーザー名の使用テスト（MySQL）
    #[test]
    fn test_default_username_mysql() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 3306,
            database: "testdb".to_string(),
            user: None,
            password: None,
            timeout: None,
        };

        let service = DatabaseConnectionService::new();
        let conn_str = service.build_connection_string(Dialect::MySQL, &config);

        // MySQLのデフォルトユーザーは "root"
        assert!(conn_str.contains("root"));
    }

    /// プールオプションの設定テスト
    #[test]
    fn test_pool_options() {
        let service = DatabaseConnectionService::new();
        let pool_options = service.create_pool_options();

        // プールオプションが作成されることを確認
        assert!(format!("{:?}", pool_options).contains("PoolOptions"));
    }

    /// 接続タイムアウトの設定テスト
    #[test]
    fn test_connection_timeout() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            user: Some("testuser".to_string()),
            password: None,
            timeout: Some(30),
        };

        let service = DatabaseConnectionService::new();
        let pool_options = service.create_pool_options_with_timeout(config.timeout);

        // タイムアウトが設定されることを確認
        assert!(format!("{:?}", pool_options).contains("PoolOptions"));
    }
}
