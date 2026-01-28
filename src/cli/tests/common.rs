// CLIテスト共通ヘルパー

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use strata::core::config::{Config, DatabaseConfig, Dialect};
use strata::services::config_serializer::ConfigSerializer;
use tempfile::TempDir;

/// テスト用のConfig作成ヘルパー
pub fn create_test_config(dialect: Dialect, database_path: Option<&str>) -> Config {
    let mut environments = HashMap::new();

    let db_config = DatabaseConfig {
        host: String::new(),
        port: None,
        database: database_path.unwrap_or(":memory:").to_string(),
        user: None,
        password: None,
        timeout: None,
    };

    environments.insert("development".to_string(), db_config);

    Config {
        version: "1.0".to_string(),
        dialect,
        schema_dir: PathBuf::from("schema"),
        migrations_dir: PathBuf::from("migrations"),
        environments,
    }
}

/// テスト用のプロジェクトディレクトリを作成
#[allow(dead_code)]
pub fn setup_test_project(
    dialect: Dialect,
    database_path: Option<&str>,
    create_migrations: bool,
) -> Result<(TempDir, PathBuf)> {
    let temp_dir = TempDir::new()?;
    let project_path = temp_dir.path().to_path_buf();

    // 設定ファイルを作成
    let config = create_test_config(dialect, database_path);
    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    let config_yaml = ConfigSerializer::to_yaml(&config)?;
    fs::write(&config_path, config_yaml)?;

    // スキーマディレクトリを作成
    fs::create_dir_all(project_path.join("schema"))?;

    // マイグレーションディレクトリを作成（必要な場合）
    if create_migrations {
        fs::create_dir_all(project_path.join("migrations"))?;
    }

    Ok((temp_dir, project_path))
}

/// テスト用のマイグレーションファイルを作成
#[allow(dead_code)]
pub fn create_test_migration(
    project_path: &Path,
    version: &str,
    description: &str,
    up_sql: &str,
    down_sql: &str,
    checksum: &str,
) -> Result<()> {
    let migration_dir = project_path
        .join("migrations")
        .join(format!("{}_{}", version, description));
    fs::create_dir_all(&migration_dir)?;

    fs::write(migration_dir.join("up.sql"), up_sql)?;
    fs::write(migration_dir.join("down.sql"), down_sql)?;

    let meta = format!(
        "version: \"{}\"\ndescription: \"{}\"\nchecksum: \"{}\"\n",
        version, description, checksum
    );
    fs::write(migration_dir.join(".meta.yaml"), meta)?;

    Ok(())
}
