// データベース設定の解決サービス
//
// 環境変数による上書きをCLI/サービス層で扱い、coreは純粋な構造体に保つ。

use crate::core::config::DatabaseConfig;

/// データベース設定の解決ユーティリティ
#[derive(Debug, Clone, Default)]
pub struct DatabaseConfigResolver;

impl DatabaseConfigResolver {
    /// 環境変数による上書きを適用
    pub fn apply_env_overrides(base: &DatabaseConfig) -> DatabaseConfig {
        let mut config = base.clone();

        if let Ok(host) = std::env::var("DB_HOST") {
            config.host = host;
        }
        if let Ok(port) = std::env::var("DB_PORT") {
            match port.parse::<u16>() {
                Ok(port_num) => config.port = Some(port_num),
                Err(_) => {
                    eprintln!(
                        "Warning: DB_PORT value '{}' is not a valid port number, ignoring",
                        port
                    );
                }
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
        if let Ok(timeout) = std::env::var("DB_TIMEOUT") {
            match timeout.parse::<u64>() {
                Ok(t) => config.timeout = Some(t),
                Err(_) => {
                    eprintln!(
                        "Warning: DB_TIMEOUT value '{}' is not a valid number, ignoring",
                        timeout
                    );
                }
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn base_config() -> DatabaseConfig {
        DatabaseConfig {
            port: Some(5432),
            database: "testdb".to_string(),
            user: Some("user".to_string()),
            password: Some("pass".to_string()),
            timeout: Some(30),
            ..Default::default()
        }
    }

    #[test]
    #[serial]
    fn test_apply_env_overrides_timeout() {
        let config = base_config();
        std::env::set_var("DB_TIMEOUT", "60");
        let resolved = DatabaseConfigResolver::apply_env_overrides(&config);
        assert_eq!(resolved.timeout, Some(60));
        std::env::remove_var("DB_TIMEOUT");
    }

    #[test]
    #[serial]
    fn test_apply_env_overrides_invalid_port_keeps_original() {
        let config = base_config();
        std::env::set_var("DB_PORT", "not_a_number");
        let resolved = DatabaseConfigResolver::apply_env_overrides(&config);
        // パース失敗時は元の値を維持
        assert_eq!(resolved.port, Some(5432));
        std::env::remove_var("DB_PORT");
    }

    #[test]
    #[serial]
    fn test_apply_env_overrides_invalid_timeout_keeps_original() {
        let config = base_config();
        std::env::set_var("DB_TIMEOUT", "invalid");
        let resolved = DatabaseConfigResolver::apply_env_overrides(&config);
        // パース失敗時は元の値を維持
        assert_eq!(resolved.timeout, Some(30));
        std::env::remove_var("DB_TIMEOUT");
    }

    #[test]
    #[serial]
    fn test_apply_env_overrides_host() {
        let config = base_config();
        std::env::set_var("DB_HOST", "newhost");
        let resolved = DatabaseConfigResolver::apply_env_overrides(&config);
        assert_eq!(resolved.host, "newhost");
        std::env::remove_var("DB_HOST");
    }

    #[test]
    #[serial]
    fn test_apply_env_overrides_database() {
        let config = base_config();
        std::env::set_var("DB_DATABASE", "newdb");
        let resolved = DatabaseConfigResolver::apply_env_overrides(&config);
        assert_eq!(resolved.database, "newdb");
        std::env::remove_var("DB_DATABASE");
    }

    #[test]
    #[serial]
    fn test_apply_env_overrides_user() {
        let config = base_config();
        std::env::set_var("DB_USER", "admin");
        let resolved = DatabaseConfigResolver::apply_env_overrides(&config);
        assert_eq!(resolved.user, Some("admin".to_string()));
        std::env::remove_var("DB_USER");
    }

    #[test]
    #[serial]
    fn test_apply_env_overrides_password() {
        let config = base_config();
        std::env::set_var("DB_PASSWORD", "secret");
        let resolved = DatabaseConfigResolver::apply_env_overrides(&config);
        assert_eq!(resolved.password, Some("secret".to_string()));
        std::env::remove_var("DB_PASSWORD");
    }

    #[test]
    #[serial]
    fn test_apply_env_overrides_valid_port() {
        let config = base_config();
        std::env::set_var("DB_PORT", "3306");
        let resolved = DatabaseConfigResolver::apply_env_overrides(&config);
        assert_eq!(resolved.port, Some(3306));
        std::env::remove_var("DB_PORT");
    }

    #[test]
    #[serial]
    fn test_apply_env_overrides_no_env_keeps_base() {
        // 全ての環境変数が未設定の場合、ベース値がそのまま保持される
        std::env::remove_var("DB_HOST");
        std::env::remove_var("DB_PORT");
        std::env::remove_var("DB_DATABASE");
        std::env::remove_var("DB_USER");
        std::env::remove_var("DB_PASSWORD");
        std::env::remove_var("DB_TIMEOUT");

        let config = base_config();
        let resolved = DatabaseConfigResolver::apply_env_overrides(&config);
        assert_eq!(resolved.port, Some(5432));
        assert_eq!(resolved.database, "testdb");
        assert_eq!(resolved.user, Some("user".to_string()));
        assert_eq!(resolved.password, Some("pass".to_string()));
        assert_eq!(resolved.timeout, Some(30));
    }

    #[test]
    #[serial]
    fn test_apply_env_overrides_multiple() {
        let config = base_config();
        std::env::set_var("DB_HOST", "remotehost");
        std::env::set_var("DB_PORT", "5433");
        std::env::set_var("DB_DATABASE", "proddb");
        let resolved = DatabaseConfigResolver::apply_env_overrides(&config);
        assert_eq!(resolved.host, "remotehost");
        assert_eq!(resolved.port, Some(5433));
        assert_eq!(resolved.database, "proddb");
        std::env::remove_var("DB_HOST");
        std::env::remove_var("DB_PORT");
        std::env::remove_var("DB_DATABASE");
    }
}
