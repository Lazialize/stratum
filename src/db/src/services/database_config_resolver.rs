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
            if let Ok(port_num) = port.parse::<u16>() {
                config.port = Some(port_num);
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
}
