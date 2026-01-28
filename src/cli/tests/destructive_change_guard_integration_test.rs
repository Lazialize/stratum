use std::fs;
use std::path::{Path, PathBuf};

use sqlx::any::install_default_drivers;
use sqlx::Executor;
use strata::cli::commands::apply::{ApplyCommand, ApplyCommandHandler};
use strata::cli::commands::generate::{GenerateCommand, GenerateCommandHandler};
use strata::core::config::{Config, Dialect};
use strata::services::config_serializer::ConfigSerializer;
use tempfile::TempDir;

mod common;

fn write_config(project_path: &Path, db_path: &str) {
    let config = common::create_test_config(Dialect::SQLite, Some(db_path));
    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    let config_yaml = ConfigSerializer::to_yaml(&config).unwrap();
    fs::write(config_path, config_yaml).unwrap();
}

fn write_schema_file(project_path: &Path, table_name: &str) {
    let schema_dir = project_path.join("schema");
    fs::create_dir_all(&schema_dir).unwrap();
    let schema_content = format!(
        r#"version: "1.0"
tables:
  {}:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#,
        table_name
    );
    fs::write(
        schema_dir.join(format!("{}.yaml", table_name)),
        schema_content,
    )
    .unwrap();
}

fn write_schema_snapshot(project_path: &Path, table_name: &str) {
    let migrations_dir = project_path.join("migrations");
    fs::create_dir_all(&migrations_dir).unwrap();
    let snapshot_content = format!(
        r#"version: "1.0"
tables:
  {}:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#,
        table_name
    );
    fs::write(
        migrations_dir.join(".schema_snapshot.yaml"),
        snapshot_content,
    )
    .unwrap();
}

fn create_migration(
    project_path: &Path,
    version: &str,
    description: &str,
    up_sql: &str,
    destructive_meta: &str,
) -> PathBuf {
    let migration_dir = project_path
        .join("migrations")
        .join(format!("{}_{}", version, description));
    fs::create_dir_all(&migration_dir).unwrap();
    fs::write(migration_dir.join("up.sql"), up_sql).unwrap();
    fs::write(migration_dir.join("down.sql"), "--").unwrap();
    fs::write(migration_dir.join(".meta.yaml"), destructive_meta).unwrap();
    migration_dir
}

#[test]
fn test_generate_rejects_destructive_without_allow() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();

    write_config(project_path, ":memory:");
    write_schema_snapshot(project_path, "users");
    write_schema_file(project_path, "products");

    let handler = GenerateCommandHandler::new();
    let command = GenerateCommand {
        project_path: project_path.to_path_buf(),
        config_path: None,
        description: Some("drop_users".to_string()),
        dry_run: false,
        allow_destructive: false,
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

    write_config(project_path, ":memory:");
    write_schema_snapshot(project_path, "users");
    write_schema_file(project_path, "products");

    let handler = GenerateCommandHandler::new();
    let command = GenerateCommand {
        project_path: project_path.to_path_buf(),
        config_path: None,
        description: Some("drop_users".to_string()),
        dry_run: false,
        allow_destructive: true,
    };

    let output = handler.execute(&command).expect("generate should succeed");
    let migration_name = output.lines().last().unwrap_or("");
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

    write_config(&project_path, db_path.to_str().unwrap());
    fs::create_dir_all(project_path.join("migrations")).unwrap();

    let meta = r#"version: "20260121120000"
description: "create_users"
dialect: sqlite
checksum: "test_checksum"
destructive_changes:
  tables_dropped:
    - "users"
"#;
    create_migration(
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

    write_config(&project_path, db_path.to_str().unwrap());
    fs::create_dir_all(project_path.join("migrations")).unwrap();

    let meta = r#"version: "20260121120000"
description: "create_users"
dialect: sqlite
checksum: "test_checksum"
destructive_changes:
  tables_dropped:
    - "users"
"#;
    create_migration(
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

    write_config(&project_path, db_path.to_str().unwrap());
    fs::create_dir_all(project_path.join("migrations")).unwrap();

    let meta = r#"version: "20260121120000"
description: "legacy_migration"
dialect: sqlite
checksum: "test_checksum"
"#;
    create_migration(
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

    write_config(&project_path, db_path.to_str().unwrap());
    write_schema_snapshot(&project_path, "users");
    write_schema_file(&project_path, "products");

    let handler = GenerateCommandHandler::new();
    let command = GenerateCommand {
        project_path: project_path.clone(),
        config_path: None,
        description: Some("drop_users".to_string()),
        dry_run: false,
        allow_destructive: true,
    };

    let output = handler.execute(&command).expect("generate should succeed");
    let migration_name = output.lines().last().unwrap_or("");

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
    };

    let result = apply_handler.execute(&apply_command).await;
    assert!(result.is_ok());

    let meta_path = project_path
        .join("migrations")
        .join(migration_name)
        .join(".meta.yaml");
    assert!(meta_path.exists());
}
