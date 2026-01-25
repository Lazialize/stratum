// applyコマンドハンドラーのテスト
//
// applyコマンドの動作を検証するテストスイート
// - データベース接続の確立
// - 未適用マイグレーションの検出
// - マイグレーションの順次実行
// - 実行結果の記録とチェックサムの保存

use sqlx::any::install_default_drivers;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use strata::cli::commands::apply::{ApplyCommand, ApplyCommandHandler};
use strata::core::config::{Config, DatabaseConfig, Dialect};

// テスト用のConfig作成ヘルパー
fn create_test_config(dialect: Dialect, database_path: Option<&str>) -> Config {
    let mut environments = HashMap::new();

    let db_config = match dialect {
        Dialect::SQLite => DatabaseConfig {
            host: String::new(),
            port: 0,
            database: database_path.unwrap_or(":memory:").to_string(),
            user: None,
            password: None,
            timeout: None,
        },
        Dialect::PostgreSQL => DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: database_path.unwrap_or("test_db").to_string(),
            user: Some("postgres".to_string()),
            password: Some("password".to_string()),
            timeout: None,
        },
        Dialect::MySQL => DatabaseConfig {
            host: "localhost".to_string(),
            port: 3306,
            database: database_path.unwrap_or("test_db").to_string(),
            user: Some("root".to_string()),
            password: Some("password".to_string()),
            timeout: None,
        },
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

#[tokio::test]
async fn test_apply_command_handler_new() {
    let handler = ApplyCommandHandler::new();
    assert!(format!("{:?}", handler).contains("ApplyCommandHandler"));
}

#[tokio::test]
async fn test_apply_command_no_config_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    let handler = ApplyCommandHandler::new();
    let command = ApplyCommand {
        project_path: project_path.clone(),
        dry_run: false,
        env: "development".to_string(),
        timeout: None,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Config file not found"));
}

#[tokio::test]
async fn test_apply_command_no_pending_migrations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    // 設定ファイルを作成
    let config = create_test_config(Dialect::SQLite, None);
    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    let config_yaml = serde_saphyr::to_string(&config).unwrap();
    fs::write(&config_path, config_yaml).unwrap();

    // マイグレーションディレクトリを作成（空）
    let migrations_dir = project_path.join(&config.migrations_dir);
    fs::create_dir_all(&migrations_dir).unwrap();

    let handler = ApplyCommandHandler::new();
    let command = ApplyCommand {
        project_path: project_path.clone(),
        dry_run: false,
        env: "development".to_string(),
        timeout: None,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    println!("Error: {}", error_msg);
    assert!(error_msg.contains("No migration files found"));
}

#[tokio::test]
async fn test_apply_command_dry_run_mode() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    // 設定ファイルを作成
    let config = create_test_config(Dialect::SQLite, None);
    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    let config_yaml = serde_saphyr::to_string(&config).unwrap();
    fs::write(&config_path, config_yaml).unwrap();

    // マイグレーションディレクトリを作成
    let migrations_dir = project_path.join(&config.migrations_dir);
    fs::create_dir_all(&migrations_dir).unwrap();

    // テスト用のマイグレーションファイルを作成
    let migration_dir = migrations_dir.join("20260121120000_create_users");
    fs::create_dir_all(&migration_dir).unwrap();

    let up_sql = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);";
    fs::write(migration_dir.join("up.sql"), up_sql).unwrap();

    let down_sql = "DROP TABLE users;";
    fs::write(migration_dir.join("down.sql"), down_sql).unwrap();

    let metadata = r#"version: "20260121120000"
description: "create_users"
dialect: SQLite
checksum: "test_checksum"
"#;
    fs::write(migration_dir.join(".meta.yaml"), metadata).unwrap();

    let handler = ApplyCommandHandler::new();
    let command = ApplyCommand {
        project_path: project_path.clone(),
        dry_run: true,
        env: "development".to_string(),
        timeout: None,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_ok());

    let summary = result.unwrap();
    assert!(summary.contains("DRY RUN"));
    assert!(summary.contains("20260121120000"));
}

#[tokio::test]
#[ignore] // Requires SQLx Any driver linkage - run as integration test
async fn test_apply_command_success_with_sqlite() {
    install_default_drivers();
    let temp_dir = tempfile::tempdir().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    // 設定ファイルを作成（SQLiteデータベース）
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();
    let config = create_test_config(Dialect::SQLite, Some(&db_path.to_string_lossy()));

    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    let config_yaml = serde_saphyr::to_string(&config).unwrap();
    fs::write(&config_path, config_yaml).unwrap();

    // マイグレーションディレクトリを作成
    let migrations_dir = project_path.join(&config.migrations_dir);
    fs::create_dir_all(&migrations_dir).unwrap();

    // テスト用のマイグレーションファイルを作成
    let migration_dir = migrations_dir.join("20260121120000_create_users");
    fs::create_dir_all(&migration_dir).unwrap();

    let up_sql = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);";
    fs::write(migration_dir.join("up.sql"), up_sql).unwrap();

    let down_sql = "DROP TABLE users;";
    fs::write(migration_dir.join("down.sql"), down_sql).unwrap();

    let metadata = r#"version: "20260121120000"
description: "create_users"
dialect: SQLite
checksum: "test_checksum"
"#;
    fs::write(migration_dir.join(".meta.yaml"), metadata).unwrap();

    let handler = ApplyCommandHandler::new();
    let command = ApplyCommand {
        project_path: project_path.clone(),
        dry_run: false,
        env: "development".to_string(),
        timeout: None,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_ok());

    let summary = result.unwrap();
    assert!(summary.contains("1"));
    assert!(summary.contains("20260121120000"));
    assert!(summary.contains("create_users"));
}

#[tokio::test]
#[ignore] // Requires SQLx Any driver linkage - run as integration test
async fn test_apply_command_migration_already_applied() {
    install_default_drivers();
    let temp_dir = tempfile::tempdir().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    // 設定ファイルを作成（SQLiteデータベース）
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();
    let config = create_test_config(Dialect::SQLite, Some(&db_path.to_string_lossy()));

    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    let config_yaml = serde_saphyr::to_string(&config).unwrap();
    fs::write(&config_path, config_yaml).unwrap();

    // マイグレーションディレクトリを作成
    let migrations_dir = project_path.join(&config.migrations_dir);
    fs::create_dir_all(&migrations_dir).unwrap();

    // テスト用のマイグレーションファイルを作成
    let migration_dir = migrations_dir.join("20260121120000_create_users");
    fs::create_dir_all(&migration_dir).unwrap();

    let up_sql = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);";
    fs::write(migration_dir.join("up.sql"), up_sql).unwrap();

    let down_sql = "DROP TABLE users;";
    fs::write(migration_dir.join("down.sql"), down_sql).unwrap();

    let metadata = r#"version: "20260121120000"
description: "create_users"
dialect: SQLite
checksum: "test_checksum"
"#;
    fs::write(migration_dir.join(".meta.yaml"), metadata).unwrap();

    let handler = ApplyCommandHandler::new();
    let command = ApplyCommand {
        project_path: project_path.clone(),
        dry_run: false,
        env: "development".to_string(),
        timeout: None,
    };

    // 1回目の適用
    let result1 = handler.execute(&command).await;
    assert!(result1.is_ok());

    // 2回目の適用（すでに適用済み）
    let result2 = handler.execute(&command).await;
    assert!(result2.is_err());
    assert!(result2
        .unwrap_err()
        .to_string()
        .contains("No pending migrations"));
}

#[tokio::test]
#[ignore] // Requires SQLx Any driver linkage - run as integration test
async fn test_apply_command_multiple_migrations() {
    install_default_drivers();
    let temp_dir = tempfile::tempdir().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    // 設定ファイルを作成（SQLiteデータベース）
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();
    let config = create_test_config(Dialect::SQLite, Some(&db_path.to_string_lossy()));

    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    let config_yaml = serde_saphyr::to_string(&config).unwrap();
    fs::write(&config_path, config_yaml).unwrap();

    // マイグレーションディレクトリを作成
    let migrations_dir = project_path.join(&config.migrations_dir);
    fs::create_dir_all(&migrations_dir).unwrap();

    // 1つ目のマイグレーション
    let migration_dir1 = migrations_dir.join("20260121120000_create_users");
    fs::create_dir_all(&migration_dir1).unwrap();
    fs::write(
        migration_dir1.join("up.sql"),
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
    )
    .unwrap();
    fs::write(migration_dir1.join("down.sql"), "DROP TABLE users;").unwrap();
    fs::write(
        migration_dir1.join(".meta.yaml"),
        r#"version: "20260121120000"
description: "create_users"
dialect: SQLite
checksum: "checksum1"
"#,
    )
    .unwrap();

    // 2つ目のマイグレーション
    let migration_dir2 = migrations_dir.join("20260121120001_create_posts");
    fs::create_dir_all(&migration_dir2).unwrap();
    fs::write(
        migration_dir2.join("up.sql"),
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT NOT NULL);",
    )
    .unwrap();
    fs::write(migration_dir2.join("down.sql"), "DROP TABLE posts;").unwrap();
    fs::write(
        migration_dir2.join(".meta.yaml"),
        r#"version: "20260121120001"
description: "create_posts"
dialect: SQLite
checksum: "checksum2"
"#,
    )
    .unwrap();

    let handler = ApplyCommandHandler::new();
    let command = ApplyCommand {
        project_path: project_path.clone(),
        dry_run: false,
        env: "development".to_string(),
        timeout: None,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_ok());

    let summary = result.unwrap();
    assert!(summary.contains("2")); // 2つのマイグレーション
    assert!(summary.contains("20260121120000"));
    assert!(summary.contains("20260121120001"));
}

#[tokio::test]
#[ignore] // Requires SQLx Any driver linkage - run as integration test
async fn test_apply_command_sql_error() {
    install_default_drivers();
    let temp_dir = tempfile::tempdir().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    // 設定ファイルを作成（SQLiteデータベース）
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();
    let config = create_test_config(Dialect::SQLite, Some(&db_path.to_string_lossy()));

    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    let config_yaml = serde_saphyr::to_string(&config).unwrap();
    fs::write(&config_path, config_yaml).unwrap();

    // マイグレーションディレクトリを作成
    let migrations_dir = project_path.join(&config.migrations_dir);
    fs::create_dir_all(&migrations_dir).unwrap();

    // 不正なSQLを含むマイグレーション
    let migration_dir = migrations_dir.join("20260121120000_invalid_sql");
    fs::create_dir_all(&migration_dir).unwrap();
    fs::write(
        migration_dir.join("up.sql"),
        "INVALID SQL STATEMENT THAT WILL FAIL;",
    )
    .unwrap();
    fs::write(migration_dir.join("down.sql"), "DROP TABLE users;").unwrap();
    fs::write(
        migration_dir.join(".meta.yaml"),
        r#"version: "20260121120000"
description: "invalid_sql"
dialect: SQLite
checksum: "test_checksum"
"#,
    )
    .unwrap();

    let handler = ApplyCommandHandler::new();
    let command = ApplyCommand {
        project_path: project_path.clone(),
        dry_run: false,
        env: "development".to_string(),
        timeout: None,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_err());
}
