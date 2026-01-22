// validateコマンドハンドラーのテスト

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use stratum::cli::commands::validate::{ValidateCommand, ValidateCommandHandler};
use stratum::core::config::{Config, DatabaseConfig, Dialect};
use tempfile::TempDir;

/// テスト用のConfig作成ヘルパー
fn create_test_config(dialect: Dialect) -> Config {
    let mut environments = HashMap::new();

    let db_config = DatabaseConfig {
        host: String::new(),
        port: 0,
        database: ":memory:".to_string(),
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
fn setup_test_project() -> Result<(TempDir, PathBuf)> {
    let temp_dir = TempDir::new()?;
    let project_path = temp_dir.path().to_path_buf();

    // 設定ファイルを作成
    let config = create_test_config(Dialect::SQLite);
    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    let config_yaml = serde_saphyr::to_string(&config)?;
    fs::write(&config_path, config_yaml)?;

    // スキーマディレクトリを作成
    fs::create_dir_all(project_path.join("schema"))?;

    // マイグレーションディレクトリを作成
    fs::create_dir_all(project_path.join("migrations"))?;

    Ok((temp_dir, project_path))
}

#[test]
fn test_new_handler() {
    let handler = ValidateCommandHandler::new();
    assert!(format!("{:?}", handler).contains("ValidateCommandHandler"));
}

#[test]
fn test_validate_command_struct() {
    let command = ValidateCommand {
        project_path: PathBuf::from("/test/path"),
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
    let (_temp_dir, project_path) = setup_test_project().unwrap();

    // スキーマディレクトリを削除
    fs::remove_dir_all(project_path.join("schema")).unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path,
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
    let (_temp_dir, project_path) = setup_test_project().unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path,
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
    let (_temp_dir, project_path) = setup_test_project().unwrap();

    // 有効なスキーマファイルを作成
    let schema_yaml = r#"
version: "1.0"
tables:
  users:
    name: users
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: true
      - name: name
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        default_value: null
        auto_increment: null
    indexes:
      - name: idx_users_name
        columns:
          - name
        unique: false
    constraints:
      - type: PRIMARY_KEY
        columns:
          - id
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path,
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
    let (_temp_dir, project_path) = setup_test_project().unwrap();

    // プライマリキーがないスキーマファイルを作成
    let schema_yaml = r#"
version: "1.0"
tables:
  users:
    name: users
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: null
      - name: name
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        default_value: null
        auto_increment: null
    indexes: []
    constraints: []
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path,
        schema_dir: None,
    };

    let result = handler.execute(&command);
    assert!(result.is_ok());

    let summary = result.unwrap();
    assert!(summary.contains("Validation Statistics"));
    assert!(summary.contains("error(s) found"));
}

#[test]
fn test_validate_invalid_foreign_key() {
    let (_temp_dir, project_path) = setup_test_project().unwrap();

    // 存在しないテーブルを参照する外部キーを持つスキーマを作成
    let schema_yaml = r#"
version: "1.0"
tables:
  posts:
    name: posts
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: null
      - name: user_id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: null
    indexes: []
    constraints:
      - type: PRIMARY_KEY
        columns:
          - id
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
        schema_dir: None,
    };

    let result = handler.execute(&command);
    assert!(result.is_ok());

    let summary = result.unwrap();
    assert!(summary.contains("Validation Statistics"));
    assert!(summary.contains("error(s) found"));
}

#[test]
fn test_validate_custom_schema_dir() {
    let (_temp_dir, project_path) = setup_test_project().unwrap();

    // カスタムスキーマディレクトリを作成
    let custom_schema_dir = project_path.join("custom_schema");
    fs::create_dir_all(&custom_schema_dir).unwrap();

    // スキーマファイルを作成
    let schema_yaml = r#"
version: "1.0"
tables:
  users:
    name: users
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: null
    indexes: []
    constraints:
      - type: PRIMARY_KEY
        columns:
          - id
"#;
    fs::write(custom_schema_dir.join("users.yaml"), schema_yaml).unwrap();

    let handler = ValidateCommandHandler::new();
    let command = ValidateCommand {
        project_path: project_path.clone(),
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
    use stratum::cli::commands::validate::ValidationSummary;
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
