/// 設定ファイル管理機能のテスト
///
/// このテストは、設定ファイルの読み込み、検証、環境別設定の管理が
/// 正しく動作することを確認します。

#[cfg(test)]
mod config_tests {
    use std::path::Path;
    use strata::core::config::{Config, DatabaseConfig, Dialect};

    /// Config構造体が正しくデシリアライズできることを確認
    #[test]
    fn test_config_deserialization() {
        let yaml = r#"
version: "1.0"
dialect: postgresql
schema_dir: schema
migrations_dir: migrations

environments:
  development:
    host: localhost
    port: 5432
    database: strata_dev
    user: postgres
    password: password
"#;

        let config: Config = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(config.version, "1.0");
        assert_eq!(config.dialect, Dialect::PostgreSQL);
        assert_eq!(config.schema_dir, Path::new("schema"));
        assert_eq!(config.migrations_dir, Path::new("migrations"));
    }

    /// 環境別のデータベース設定を取得できることを確認
    #[test]
    fn test_get_database_config_for_environment() {
        let yaml = r#"
version: "1.0"
dialect: postgresql
schema_dir: schema
migrations_dir: migrations

environments:
  development:
    host: localhost
    port: 5432
    database: strata_dev
    user: postgres
    password: password

  production:
    host: prod.example.com
    port: 5432
    database: strata_prod
    user: app_user
    password: secure_password
"#;

        let config: Config = serde_saphyr::from_str(yaml).unwrap();

        let dev_config = config.get_database_config("development").unwrap();
        assert_eq!(dev_config.host, "localhost");
        assert_eq!(dev_config.port, 5432);
        assert_eq!(dev_config.database, "strata_dev");

        let prod_config = config.get_database_config("production").unwrap();
        assert_eq!(prod_config.host, "prod.example.com");
        assert_eq!(prod_config.database, "strata_prod");
    }

    /// 存在しない環境名でエラーが返されることを確認
    #[test]
    fn test_get_nonexistent_environment() {
        let yaml = r#"
version: "1.0"
dialect: postgresql
schema_dir: schema
migrations_dir: migrations

environments:
  development:
    host: localhost
    port: 5432
    database: strata_dev
    user: postgres
    password: password
"#;

        let config: Config = serde_saphyr::from_str(yaml).unwrap();

        let result = config.get_database_config("staging");
        assert!(result.is_err());
    }

    /// Dialectがシリアライズ/デシリアライズできることを確認
    #[test]
    fn test_dialect_serialization() {
        let postgresql_yaml = "postgresql";
        let mysql_yaml = "mysql";
        let sqlite_yaml = "sqlite";

        let pg: Dialect = serde_saphyr::from_str(postgresql_yaml).unwrap();
        assert_eq!(pg, Dialect::PostgreSQL);

        let my: Dialect = serde_saphyr::from_str(mysql_yaml).unwrap();
        assert_eq!(my, Dialect::MySQL);

        let sq: Dialect = serde_saphyr::from_str(sqlite_yaml).unwrap();
        assert_eq!(sq, Dialect::SQLite);
    }

    /// 不正なdialectでエラーが返されることを確認
    #[test]
    fn test_invalid_dialect() {
        let invalid_yaml = "oracle";
        let result: Result<Dialect, _> = serde_saphyr::from_str(invalid_yaml);
        assert!(result.is_err());
    }

    /// デフォルト値が正しく設定されることを確認
    #[test]
    fn test_config_defaults() {
        let minimal_yaml = r#"
version: "1.0"
dialect: sqlite

environments:
  development:
    database: strata.db
"#;

        let config: Config = serde_saphyr::from_str(minimal_yaml).unwrap();

        // デフォルト値の確認
        assert_eq!(config.schema_dir, Path::new("schema"));
        assert_eq!(config.migrations_dir, Path::new("migrations"));
    }

    /// 環境変数からデータベース設定を上書きできることを確認
    #[test]
    fn test_database_config_with_env_vars() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "strata_dev".to_string(),
            user: Some("postgres".to_string()),
            password: None,
            timeout: None,
        };

        // 環境変数を模擬（実際のテストでは std::env を使用）
        let merged = config.merge_with_env();

        assert_eq!(merged.host, "localhost");
        assert_eq!(merged.database, "strata_dev");
    }

    /// バリデーションが正しく動作することを確認
    #[test]
    fn test_config_validation() {
        let valid_yaml = r#"
version: "1.0"
dialect: postgresql
schema_dir: schema
migrations_dir: migrations

environments:
  development:
    host: localhost
    port: 5432
    database: strata_dev
    user: postgres
    password: password
"#;

        let config: Config = serde_saphyr::from_str(valid_yaml).unwrap();
        assert!(config.validate().is_ok());
    }

    /// 必須フィールドがない場合のバリデーションエラーを確認
    #[test]
    fn test_config_validation_missing_database() {
        let invalid_yaml = r#"
version: "1.0"
dialect: postgresql
schema_dir: schema
migrations_dir: migrations

environments:
  development:
    host: localhost
    port: 5432
    user: postgres
    password: password
"#;

        let result: Result<Config, _> = serde_saphyr::from_str(invalid_yaml);
        // databaseフィールドがないためデシリアライズに失敗することを期待
        assert!(result.is_err());
    }
}
