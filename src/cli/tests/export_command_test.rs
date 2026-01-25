// exportコマンドハンドラーのテスト

use sqlx::any::install_default_drivers;
use std::fs;
use std::path::PathBuf;
use strata::cli::commands::export::{ExportCommand, ExportCommandHandler};
use strata::core::config::Dialect;
use strata::services::config_serializer::ConfigSerializer;
use tempfile::TempDir;

mod common;

#[test]
fn test_new_handler() {
    let handler = ExportCommandHandler::new();
    assert!(format!("{:?}", handler).contains("ExportCommandHandler"));
}

#[test]
fn test_export_command_struct() {
    let command = ExportCommand {
        project_path: PathBuf::from("/test/path"),
        env: "development".to_string(),
        output_dir: Some(PathBuf::from("/test/output")),
    };

    assert_eq!(command.project_path, PathBuf::from("/test/path"));
    assert_eq!(command.env, "development");
    assert_eq!(command.output_dir, Some(PathBuf::from("/test/output")));
}

#[tokio::test]
async fn test_export_no_config_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    let handler = ExportCommandHandler::new();
    let command = ExportCommand {
        project_path,
        env: "development".to_string(),
        output_dir: None,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Config file not found"));
}

#[tokio::test]
async fn test_export_invalid_environment() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, false).unwrap();

    let handler = ExportCommandHandler::new();
    let command = ExportCommand {
        project_path,
        env: "invalid_env".to_string(),
        output_dir: None,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Environment") || error_msg.contains("not found"));
}

#[tokio::test]
#[ignore] // 統合テスト - 実際のデータベースが必要
async fn test_export_from_sqlite_database() {
    install_default_drivers();
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, false).unwrap();

    // データベースファイルのパス
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();

    // 設定ファイルにデータベース接続情報を追加
    let config =
        common::create_test_config(Dialect::SQLite, Some(&db_path.to_string_lossy()));
    let config_path = project_path.join(strata::core::config::Config::DEFAULT_CONFIG_PATH);
    let config_yaml = ConfigSerializer::to_yaml(&config).unwrap();
    fs::write(&config_path, config_yaml).unwrap();

    // テスト用のテーブルを作成
    use strata::adapters::database::DatabaseConnectionService;

    let db_service = DatabaseConnectionService::new();
    let db_config = config.get_database_config("development").unwrap();
    let pool = db_service
        .create_pool(Dialect::SQLite, &db_config)
        .await
        .unwrap();

    // テーブルを作成
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT UNIQUE
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            user_id INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // インデックスを作成
    sqlx::query("CREATE INDEX idx_users_email ON users(email)")
        .execute(&pool)
        .await
        .unwrap();

    // エクスポートディレクトリを指定
    let export_dir = project_path.join("exported_schema");

    let handler = ExportCommandHandler::new();
    let command = ExportCommand {
        project_path: project_path.clone(),
        env: "development".to_string(),
        output_dir: Some(export_dir.clone()),
    };

    let result = handler.execute(&command).await;
    assert!(result.is_ok(), "Export failed: {:?}", result);

    let summary = result.unwrap();
    assert!(summary.contains("Export Complete"));
    assert!(summary.contains("users"));
    assert!(summary.contains("posts"));

    // エクスポートされたファイルが存在することを確認
    assert!(export_dir.exists());
    assert!(export_dir.join("users.yaml").exists() || export_dir.join("schema.yaml").exists());
}

#[tokio::test]
#[ignore] // 統合テスト - 実際のデータベースが必要
async fn test_export_to_stdout() {
    install_default_drivers();
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, false).unwrap();

    // データベースファイルのパス
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();

    // 設定ファイルにデータベース接続情報を追加
    let config =
        common::create_test_config(Dialect::SQLite, Some(&db_path.to_string_lossy()));
    let config_path = project_path.join(strata::core::config::Config::DEFAULT_CONFIG_PATH);
    let config_yaml = ConfigSerializer::to_yaml(&config).unwrap();
    fs::write(&config_path, config_yaml).unwrap();

    // テスト用のテーブルを作成
    use strata::adapters::database::DatabaseConnectionService;

    let db_service = DatabaseConnectionService::new();
    let db_config = config.get_database_config("development").unwrap();
    let pool = db_service
        .create_pool(Dialect::SQLite, &db_config)
        .await
        .unwrap();

    sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(&pool)
        .await
        .unwrap();

    // 出力ディレクトリを指定しない（標準出力）
    let handler = ExportCommandHandler::new();
    let command = ExportCommand {
        project_path,
        env: "development".to_string(),
        output_dir: None,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_ok(), "Export to stdout failed: {:?}", result);

    let output = result.unwrap();
    assert!(output.contains("version:"));
    assert!(output.contains("tables:"));
    assert!(output.contains("users:"));
}

#[test]
fn test_format_export_summary() {
    let handler = ExportCommandHandler::new();

    let table_names = vec!["users".to_string(), "posts".to_string()];
    let output_path = Some(PathBuf::from("/test/output"));

    let summary = handler.format_export_summary(&table_names, output_path.as_ref());

    assert!(summary.contains("Export Complete"));
    assert!(summary.contains("Exported tables: 2"));
    assert!(summary.contains("users"));
    assert!(summary.contains("posts"));
    assert!(summary.contains("/test/output"));
}

#[test]
fn test_format_export_summary_stdout() {
    let handler = ExportCommandHandler::new();

    let table_names = vec!["users".to_string()];

    let summary = handler.format_export_summary(&table_names, None);

    assert!(summary.contains("Export Complete"));
    assert!(summary.contains("Exported tables: 1"));
    assert!(summary.contains("stdout"));
}
