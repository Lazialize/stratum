// validateコマンドハンドラーのテスト

use std::fs;
use std::path::PathBuf;
use strata::cli::commands::validate::{ValidateCommand, ValidateCommandHandler};
use strata::core::config::Dialect;
use tempfile::TempDir;

mod common;

#[test]
fn test_new_handler() {
    let handler = ValidateCommandHandler::new();
    assert!(format!("{:?}", handler).contains("ValidateCommandHandler"));
}

#[test]
fn test_validate_command_struct() {
    let command = ValidateCommand {
        project_path: PathBuf::from("/test/path"),
        config_path: None,
        schema_dir: None,
    };

    assert_eq!(command.project_path, PathBuf::from("/test/path"));
    assert_eq!(command.schema_dir, None);
}

#[test]
fn test_validate_no_config_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path,
        config_path: None,
        schema_dir: None,
    };

    let result = handler.execute(&command);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Config file not found"));
}

#[test]
fn test_validate_no_schema_dir() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // スキーマディレクトリを削除
    fs::remove_dir_all(project_path.join("schema")).unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path,
        config_path: None,
        schema_dir: None,
    };

    let result = handler.execute(&command);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Schema directory not found"));
}

#[test]
fn test_validate_empty_schema_dir() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path,
        config_path: None,
        schema_dir: None,
    };

    let result = handler.execute(&command);
    assert!(result.is_ok());

    let summary = result.unwrap();
    assert!(summary.contains("Validation complete"));
    assert!(summary.contains("Tables: 0"));
}

#[test]
fn test_validate_valid_schema() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // 有効なスキーマファイルを作成（新構文）
    let schema_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: name
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_users_name
        columns:
          - name
        unique: false
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path,
        config_path: None,
        schema_dir: None,
    };

    let result = handler.execute(&command);
    assert!(result.is_ok(), "Validation failed: {:?}", result);

    let summary = result.unwrap();
    assert!(summary.contains("Validation complete"));
    assert!(summary.contains("Tables: 1"));
    assert!(summary.contains("No errors found"));
}

#[test]
fn test_validate_invalid_schema_no_primary_key() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // プライマリキーがないスキーマファイルを作成（新構文）
    let schema_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: name
        type:
          kind: VARCHAR
          length: 255
        nullable: false
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path,
        config_path: None,
        schema_dir: None,
    };

    let result = handler.execute(&command);
    assert!(result.is_err());

    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Validation failed"));
}

#[test]
fn test_validate_invalid_foreign_key() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // 存在しないテーブルを参照する外部キーを持つスキーマを作成（新構文）
    let schema_yaml = r#"
version: "1.0"
tables:
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: user_id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - user_id
        referenced_table: users
        referenced_columns:
          - id
"#;
    fs::write(project_path.join("schema/posts.yaml"), schema_yaml).unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path,
        config_path: None,
        schema_dir: None,
    };

    let result = handler.execute(&command);
    assert!(result.is_err());

    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Validation failed"));
}

#[test]
fn test_validate_custom_schema_dir() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // カスタムスキーマディレクトリを作成
    let custom_schema_dir = project_path.join("custom_schema");
    fs::create_dir_all(&custom_schema_dir).unwrap();

    // スキーマファイルを作成（新構文）
    let schema_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#;
    fs::write(custom_schema_dir.join("users.yaml"), schema_yaml).unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path: project_path.clone(),
        config_path: None,
        schema_dir: Some(custom_schema_dir),
    };

    let result = handler.execute(&command);
    assert!(result.is_ok());

    let summary = result.unwrap();
    assert!(summary.contains("Validation complete"));
    assert!(summary.contains("Tables: 1"));
}

#[test]
fn test_format_validation_summary() {
    use strata::cli::commands::validate::ValidationSummary;
    let handler = ValidateCommandHandler::new();

    // Format validation summary
    let summary_data = ValidationSummary {
        is_valid: true,
        error_count: 0,
        warning_count: 0,
        table_count: 2,
        column_count: 5,
        index_count: 3,
        constraint_count: 1,
    };
    let summary = handler.format_validation_summary(summary_data);

    assert!(summary.contains("Validation complete"));
    assert!(summary.contains("Tables: 2"));
    assert!(summary.contains("No errors found"));
}
