use std::fs;

use sqlx::any::install_default_drivers;
use sqlx::Executor;
use strata::cli::commands::apply::{ApplyCommand, ApplyCommandHandler};
use strata::cli::commands::generate::{GenerateCommand, GenerateCommandHandler};
use strata::core::config::Dialect;
use tempfile::TempDir;

mod common;

#[test]
fn test_generate_rejects_destructive_without_allow() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();

    common::write_config(project_path, Dialect::SQLite, Some(":memory:"));
    common::write_schema_snapshot(project_path, "users");
    common::write_schema_file(project_path, "products");

    let handler = GenerateCommandHandler::new();
    let command = GenerateCommand {
        project_path: project_path.to_path_buf(),
        config_path: None,
        description: Some("drop_users".to_string()),
        dry_run: false,
        allow_destructive: false,
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Destructive changes detected"));
    assert!(err.contains("Tables to be dropped"));
}

#[test]
fn test_generate_allows_destructive_with_flag_and_writes_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();

    common::write_config(project_path, Dialect::SQLite, Some(":memory:"));
    common::write_schema_snapshot(project_path, "users");
    common::write_schema_file(project_path, "products");

    let handler = GenerateCommandHandler::new();
    let command = GenerateCommand {
        project_path: project_path.to_path_buf(),
        config_path: None,
        description: Some("drop_users".to_string()),
        dry_run: false,
        allow_destructive: true,
        format: strata::cli::OutputFormat::Text,
    };

    let output = handler.execute(&command).expect("generate should succeed");
    let migration_name = output
        .lines()
        .find(|line| line.starts_with("20"))
        .unwrap_or("");
    let meta_path = project_path
        .join("migrations")
        .join(migration_name)
        .join(".meta.yaml");
    let meta = fs::read_to_string(meta_path).expect("meta should exist");

    assert!(output.contains("Warning: Destructive changes allowed"));
    assert!(meta.contains("destructive_changes"));
    assert!(meta.contains("tables_dropped"));
}

#[tokio::test]
async fn test_apply_rejects_destructive_without_allow() {
    install_default_drivers();
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();

    common::write_config(
        &project_path,
        Dialect::SQLite,
        Some(db_path.to_str().unwrap()),
    );
    fs::create_dir_all(project_path.join("migrations")).unwrap();

    let meta = r#"version: "20260121120000"
description: "create_users"
dialect: sqlite
checksum: "test_checksum"
destructive_changes:
  tables_dropped:
    - "users"
"#;
    common::create_migration_with_meta(
        &project_path,
        "20260121120000",
        "create_users",
        "CREATE TABLE users (id INTEGER);",
        meta,
    );

    let handler = ApplyCommandHandler::new();
    let command = ApplyCommand {
        project_path: project_path.clone(),
        config_path: None,
        dry_run: false,
        env: "development".to_string(),
        timeout: None,
        allow_destructive: false,
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Destructive changes detected"));
    assert!(err.contains("Migration: 20260121120000"));
}

#[tokio::test]
async fn test_apply_allows_destructive_with_allow() {
    install_default_drivers();
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();

    common::write_config(
        &project_path,
        Dialect::SQLite,
        Some(db_path.to_str().unwrap()),
    );
    fs::create_dir_all(project_path.join("migrations")).unwrap();

    let meta = r#"version: "20260121120000"
description: "create_users"
dialect: sqlite
checksum: "test_checksum"
destructive_changes:
  tables_dropped:
    - "users"
"#;
    common::create_migration_with_meta(
        &project_path,
        "20260121120000",
        "create_users",
        "CREATE TABLE users (id INTEGER);",
        meta,
    );

    let handler = ApplyCommandHandler::new();
    let command = ApplyCommand {
        project_path: project_path.clone(),
        config_path: None,
        dry_run: false,
        env: "development".to_string(),
        timeout: None,
        allow_destructive: true,
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("Migration Apply Complete"));
}

#[tokio::test]
async fn test_apply_invalid_metadata_without_destructive_changes_fails() {
    install_default_drivers();
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();

    common::write_config(
        &project_path,
        Dialect::SQLite,
        Some(db_path.to_str().unwrap()),
    );
    fs::create_dir_all(project_path.join("migrations")).unwrap();

    let meta = r#"version: "20260121120000"
description: "legacy_migration"
dialect: sqlite
checksum: "test_checksum"
"#;
    common::create_migration_with_meta(
        &project_path,
        "20260121120000",
        "legacy_migration",
        "CREATE TABLE legacy (id INTEGER);",
        meta,
    );

    let handler = ApplyCommandHandler::new();
    let command = ApplyCommand {
        project_path: project_path.clone(),
        config_path: None,
        dry_run: false,
        env: "development".to_string(),
        timeout: None,
        allow_destructive: false,
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Failed to parse metadata"));
}

#[tokio::test]
async fn test_e2e_destructive_generate_apply_flow() {
    install_default_drivers();
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();
    let db_path = project_path.join("test.db");
    fs::File::create(&db_path).unwrap();

    common::write_config(
        &project_path,
        Dialect::SQLite,
        Some(db_path.to_str().unwrap()),
    );
    common::write_schema_snapshot(&project_path, "users");
    common::write_schema_file(&project_path, "products");

    let handler = GenerateCommandHandler::new();
    let command = GenerateCommand {
        project_path: project_path.clone(),
        config_path: None,
        description: Some("drop_users".to_string()),
        dry_run: false,
        allow_destructive: true,
        format: strata::cli::OutputFormat::Text,
    };

    let output = handler.execute(&command).expect("generate should succeed");
    let migration_name = output
        .lines()
        .find(|line| line.starts_with("20"))
        .unwrap_or("");

    let pool = sqlx::AnyPool::connect(&format!("sqlite://{}", db_path.display()))
        .await
        .expect("connect sqlite");
    pool.execute("CREATE TABLE users (id INTEGER);")
        .await
        .expect("create table");

    let apply_handler = ApplyCommandHandler::new();
    let apply_command = ApplyCommand {
        project_path: project_path.clone(),
        config_path: None,
        dry_run: false,
        env: "development".to_string(),
        timeout: None,
        allow_destructive: true,
        format: strata::cli::OutputFormat::Text,
    };

    let result = apply_handler.execute(&apply_command).await;
    assert!(result.is_ok());

    let meta_path = project_path
        .join("migrations")
        .join(migration_name)
        .join(".meta.yaml");
    assert!(meta_path.exists());
}
