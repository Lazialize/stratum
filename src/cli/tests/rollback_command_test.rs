// rollbackコマンドハンドラーのテスト

use sqlx::any::install_default_drivers;
use std::fs;
use std::path::PathBuf;
use strata::cli::commands::rollback::{RollbackCommand, RollbackCommandHandler};
use strata::core::config::Dialect;
use strata::services::config_loader::ConfigLoader;
use strata::services::config_serializer::ConfigSerializer;
use tempfile::TempDir;

mod common;

#[test]
fn test_new_handler() {
    let handler = RollbackCommandHandler::new();
    assert!(format!("{:?}", handler).contains("RollbackCommandHandler"));
}

#[test]
fn test_rollback_command_struct() {
    let command = RollbackCommand {
        project_path: PathBuf::from("/test/path"),
        steps: Some(1),
        env: "development".to_string(),
    };

    assert_eq!(command.project_path, PathBuf::from("/test/path"));
    assert_eq!(command.steps, Some(1));
    assert_eq!(command.env, "development");
}

#[tokio::test]
async fn test_rollback_no_config_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    let handler = RollbackCommandHandler::new();
    let command = RollbackCommand {
        project_path,
        steps: None,
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
async fn test_rollback_no_migrations_dir() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // マイグレーションディレクトリを削除
    fs::remove_dir_all(project_path.join("migrations")).unwrap();

    let handler = RollbackCommandHandler::new();
    let command = RollbackCommand {
        project_path,
        steps: None,
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
async fn test_load_available_migrations() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // テストマイグレーションを作成
    common::create_test_migration(
        &project_path,
        "20260121120000",
        "create_users",
        "CREATE TABLE users (id INTEGER PRIMARY KEY);",
        "DROP TABLE users;",
        "test_checksum_20260121120000",
    )
    .unwrap();

    common::create_test_migration(
        &project_path,
        "20260121120001",
        "create_posts",
        "CREATE TABLE posts (id INTEGER PRIMARY KEY);",
        "DROP TABLE posts;",
        "test_checksum_20260121120001",
    )
    .unwrap();

    let handler = RollbackCommandHandler::new();
    let migrations_dir = project_path.join("migrations");

    let migrations = handler.load_available_migrations(&migrations_dir).unwrap();

    assert_eq!(migrations.len(), 2);
    assert_eq!(migrations[0].0, "20260121120000");
    assert_eq!(migrations[0].1, "create_users");
    assert_eq!(migrations[1].0, "20260121120001");
    assert_eq!(migrations[1].1, "create_posts");
}

#[tokio::test]
#[ignore] // 統合テスト - 実際のデータベースが必要
async fn test_rollback_single_migration_sqlite() {
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

    // マイグレーションを作成
    common::create_test_migration(
        &project_path,
        "20260121120000",
        "create_users",
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);",
        "DROP TABLE users;",
        "test_checksum_20260121120000",
    )
    .unwrap();

    // マイグレーションを適用（データベースとマイグレーション履歴を準備）
    use strata::adapters::database::DatabaseConnectionService;
    use strata::adapters::database_migrator::DatabaseMigratorService;

    let config = ConfigLoader::from_file(
        &project_path.join(strata::core::config::Config::DEFAULT_CONFIG_PATH),
    )
    .unwrap();
    let db_config = config.get_database_config("development").unwrap();

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
        "test_checksum".to_string(),
    );
    migrator
        .record_migration_with_dialect(&pool, &migration, Dialect::SQLite)
        .await
        .unwrap();

    // ロールバックコマンドを実行
    let handler = RollbackCommandHandler::new();
    let command = RollbackCommand {
        project_path: project_path.clone(),
        steps: None, // デフォルトは1件
        env: "development".to_string(),
    };

    let result = handler.execute(&command).await;
    assert!(result.is_ok(), "Rollback failed: {:?}", result);

    let summary = result.unwrap();
    assert!(summary.contains("Migration Rollback Complete"));
    assert!(summary.contains("20260121120000"));

    // マイグレーション履歴が削除されたことを確認
    let records = migrator
        .get_migrations(&pool, Dialect::SQLite)
        .await
        .unwrap();
    assert_eq!(records.len(), 0);

    // テーブルが削除されたことを確認
    let table_exists =
        sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='users'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(table_exists.is_none());
}

#[tokio::test]
async fn test_generate_summary() {
    use chrono::Duration;
    use strata::core::migration::AppliedMigration;

    let handler = RollbackCommandHandler::new();

    let rolled_back = vec![
        AppliedMigration::new(
            "20260121120001".to_string(),
            "create_posts".to_string(),
            chrono::Utc::now(),
            Duration::milliseconds(50),
        ),
        AppliedMigration::new(
            "20260121120000".to_string(),
            "create_users".to_string(),
            chrono::Utc::now(),
            Duration::milliseconds(30),
        ),
    ];

    let summary = handler.generate_summary(&rolled_back);
    assert!(summary.contains("2 migration(s) rolled back"));
    assert!(summary.contains("20260121120001"));
    assert!(summary.contains("20260121120000"));
    assert!(summary.contains("80ms")); // 50 + 30
}
