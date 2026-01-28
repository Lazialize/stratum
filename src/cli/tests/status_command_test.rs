// statusコマンドハンドラーのテスト

use sqlx::any::install_default_drivers;
use std::fs;
use std::path::PathBuf;
use strata::cli::commands::status::{StatusCommand, StatusCommandHandler};
use strata::core::config::Dialect;
use strata::services::config_loader::ConfigLoader;
use strata::services::config_serializer::ConfigSerializer;
use tempfile::TempDir;

mod common;

#[test]
fn test_new_handler() {
    let handler = StatusCommandHandler::new();
    assert!(format!("{:?}", handler).contains("StatusCommandHandler"));
}

#[test]
fn test_status_command_struct() {
    let command = StatusCommand {
        project_path: PathBuf::from("/test/path"),
        config_path: None,
        env: "development".to_string(),
    };

    assert_eq!(command.project_path, PathBuf::from("/test/path"));
    assert_eq!(command.env, "development");
}

#[tokio::test]
async fn test_status_no_config_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    let handler = StatusCommandHandler::new();
    let command = StatusCommand {
        project_path,
        config_path: None,
        env: "development".to_string(),
    };

    let result = handler.execute(&command).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Config file not found"));
}

#[tokio::test]
async fn test_status_no_migrations_dir() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // マイグレーションディレクトリを削除
    fs::remove_dir_all(project_path.join("migrations")).unwrap();

    let handler = StatusCommandHandler::new();
    let command = StatusCommand {
        project_path,
        config_path: None,
        env: "development".to_string(),
    };

    let result = handler.execute(&command).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Migrations directory not found"));
}

#[tokio::test]
async fn test_status_no_migrations() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    let handler = StatusCommandHandler::new();
    let command = StatusCommand {
        project_path,
        config_path: None,
        env: "development".to_string(),
    };

    let result = handler.execute(&command).await;
    assert!(result.is_ok());

    let summary = result.unwrap();
    assert!(summary.contains("Migration Status"));
    assert!(summary.contains("No migrations found"));
}

#[tokio::test]
#[ignore] // 統合テスト - 実際のデータベースが必要
async fn test_status_with_pending_migrations() {
    install_default_drivers();
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // データベースファイルのパス
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();

    // 設定ファイルにデータベース接続情報を追加
    let config = common::create_test_config(Dialect::SQLite, Some(&db_path.to_string_lossy()));
    let config_path = project_path.join(strata::core::config::Config::DEFAULT_CONFIG_PATH);
    let config_yaml = ConfigSerializer::to_yaml(&config).unwrap();
    fs::write(&config_path, config_yaml).unwrap();

    // マイグレーションファイルを作成
    common::create_test_migration(
        &project_path,
        "20260121120000",
        "create_users",
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);",
        "DROP TABLE users;",
        "test_checksum_20260121120000",
    )
    .unwrap();

    common::create_test_migration(
        &project_path,
        "20260121120001",
        "create_posts",
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT);",
        "DROP TABLE posts;",
        "test_checksum_20260121120001",
    )
    .unwrap();

    let handler = StatusCommandHandler::new();
    let command = StatusCommand {
        project_path,
        config_path: None,
        env: "development".to_string(),
    };

    let result = handler.execute(&command).await;
    assert!(result.is_ok(), "Status failed: {:?}", result);

    let summary = result.unwrap();
    assert!(summary.contains("Migration Status"));
    assert!(summary.contains("20260121120000"));
    assert!(summary.contains("20260121120001"));
    assert!(summary.contains("Pending"));
}

#[tokio::test]
#[ignore] // 統合テスト - 実際のデータベースが必要
async fn test_status_with_applied_migrations() {
    install_default_drivers();
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // データベースファイルのパス
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();

    // 設定ファイルにデータベース接続情報を追加
    let config = common::create_test_config(Dialect::SQLite, Some(&db_path.to_string_lossy()));
    let config_path = project_path.join(strata::core::config::Config::DEFAULT_CONFIG_PATH);
    let config_yaml = ConfigSerializer::to_yaml(&config).unwrap();
    fs::write(&config_path, config_yaml).unwrap();

    // マイグレーションファイルを作成
    common::create_test_migration(
        &project_path,
        "20260121120000",
        "create_users",
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);",
        "DROP TABLE users;",
        "test_checksum_20260121120000",
    )
    .unwrap();

    // データベースとマイグレーション履歴を準備
    use strata::adapters::database::DatabaseConnectionService;
    use strata::adapters::database_migrator::DatabaseMigratorService;

    let config_loaded = ConfigLoader::from_file(
        &project_path.join(strata::core::config::Config::DEFAULT_CONFIG_PATH),
    )
    .unwrap();
    let db_config = config_loaded.get_database_config("development").unwrap();

    let db_service = DatabaseConnectionService::new();
    let pool = db_service
        .create_pool(Dialect::SQLite, &db_config)
        .await
        .unwrap();

    let migrator = DatabaseMigratorService::new();
    migrator
        .create_migration_table(&pool, Dialect::SQLite)
        .await
        .unwrap();

    // マイグレーションを手動で適用
    sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);")
        .execute(&pool)
        .await
        .unwrap();

    // マイグレーション履歴を記録
    let migration = strata::core::migration::Migration::new(
        "20260121120000".to_string(),
        "create_users".to_string(),
        "test_checksum_20260121120000".to_string(),
    );
    migrator
        .record_migration_with_dialect(&pool, &migration, Dialect::SQLite)
        .await
        .unwrap();

    let handler = StatusCommandHandler::new();
    let command = StatusCommand {
        project_path,
        config_path: None,
        env: "development".to_string(),
    };

    let result = handler.execute(&command).await;
    assert!(result.is_ok(), "Status failed: {:?}", result);

    let summary = result.unwrap();
    assert!(summary.contains("Migration Status"));
    assert!(summary.contains("20260121120000"));
    assert!(summary.contains("Applied"));
}

#[test]
fn test_format_migration_status() {
    let handler = StatusCommandHandler::new();

    let summary =
        handler.format_migration_status(&[("20260121120000", "create_users", "Applied")], 1, 0);

    assert!(summary.contains("Migration Status"));
    assert!(summary.contains("20260121120000"));
    assert!(summary.contains("create_users"));
    assert!(summary.contains("Applied"));
    assert!(summary.contains("Total: 1"));
}
