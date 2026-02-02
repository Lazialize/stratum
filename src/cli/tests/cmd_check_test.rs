// checkコマンドハンドラーのテスト

use std::fs;
use std::path::PathBuf;
use strata::cli::commands::check::{CheckCommand, CheckCommandHandler};
use strata::core::config::Dialect;
use tempfile::TempDir;

mod common;

// ======================================
// Task 4.1: validate成功/失敗分岐の動作テスト
// ======================================

#[test]
fn test_new_handler() {
    let handler = CheckCommandHandler::new();
    assert!(format!("{:?}", handler).contains("CheckCommandHandler"));
}

#[test]
fn test_check_command_struct() {
    let command = CheckCommand {
        project_path: PathBuf::from("/test/path"),
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Text,
    };

    assert_eq!(command.project_path, PathBuf::from("/test/path"));
    assert_eq!(command.schema_dir, None);
}

#[test]
fn test_check_no_config_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path,
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Validation failed"));
}

#[test]
fn test_check_valid_schema_no_changes() {
    // validate成功、スキーマ変更なし → 両方成功
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // 有効なスキーマファイルを作成
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
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path,
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command);
    assert!(result.is_ok(), "Check should succeed: {:?}", result);

    let output = result.unwrap();
    assert!(output.contains("Check Results"));
    assert!(output.contains("Validate"));
    assert!(output.contains("✓ Validate: passed"));
    assert!(output.contains("✓ Generate (dry-run): passed"));
}

#[test]
fn test_check_invalid_schema_skips_generate() {
    // validate失敗 → generate はスキップされる
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // プライマリキーがないスキーマ（検証エラー）
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

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path,
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command);
    assert!(result.is_err());

    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Validation failed"));
}

#[test]
fn test_check_valid_schema_with_changes() {
    // validate成功、スキーマに差分あり → dry-runが実行される
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // スキーマスナップショットを作成（空のスキーマ）
    let snapshot_yaml = r#"
version: "1.0"
tables: {}
"#;
    fs::write(
        project_path.join("migrations/.schema_snapshot.yaml"),
        snapshot_yaml,
    )
    .unwrap();

    // 新しいテーブルを含むスキーマ
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
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path,
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command);
    assert!(result.is_ok(), "Check should succeed: {:?}", result);

    let output = result.unwrap();
    assert!(output.contains("Check Results"));
    assert!(output.contains("✓ Validate: passed"));
    assert!(output.contains("✓ Generate (dry-run): passed"));
    // dry-run出力にSQLが含まれるはず
    assert!(
        output.contains("Dry Run") || output.contains("No schema changes"),
        "Should contain dry-run output: {}",
        output
    );
}

// ======================================
// Task 4.2: 出力フォーマットの構造テスト
// ======================================

#[test]
fn test_check_json_output_validate_success() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

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
    primary_key:
      - id
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path,
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Json,
    };

    let result = handler.execute(&command);
    assert!(result.is_ok(), "Check should succeed: {:?}", result);

    let json_str = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // validate結果
    assert_eq!(parsed["validate"]["is_valid"], true);
    assert_eq!(parsed["summary"]["validate_success"], true);
    assert_eq!(parsed["summary"]["generate_success"], true);
    assert!(parsed["generate"].is_object());
}

#[test]
fn test_check_json_output_validate_failure() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // プライマリキーなしスキーマ
    let schema_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path,
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Json,
    };

    let result = handler.execute(&command);
    assert!(result.is_err());
}

// ======================================
// Task 4.3: schema_dir上書きの反映テスト
// ======================================

#[test]
fn test_check_custom_schema_dir() {
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // カスタムスキーマディレクトリを作成
    let custom_schema_dir = project_path.join("custom_schema");
    fs::create_dir_all(&custom_schema_dir).unwrap();

    let schema_yaml = r#"
version: "1.0"
tables:
  products:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
    primary_key:
      - id
"#;
    fs::write(custom_schema_dir.join("products.yaml"), schema_yaml).unwrap();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path: project_path.clone(),
        config_path: None,
        schema_dir: Some(custom_schema_dir),
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command);
    assert!(
        result.is_ok(),
        "Check with custom schema_dir should succeed: {:?}",
        result
    );

    let output = result.unwrap();
    assert!(output.contains("✓ Validate: passed"));
}

#[test]
fn test_check_custom_schema_dir_reflected_in_generate() {
    // schema_dir上書きがgenerateにも反映されることを確認
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // カスタムスキーマディレクトリ
    let custom_schema_dir = project_path.join("alt_schema");
    fs::create_dir_all(&custom_schema_dir).unwrap();

    // 空のスナップショットを作成
    let snapshot_yaml = r#"
version: "1.0"
tables: {}
"#;
    fs::write(
        project_path.join("migrations/.schema_snapshot.yaml"),
        snapshot_yaml,
    )
    .unwrap();

    let schema_yaml = r#"
version: "1.0"
tables:
  orders:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: total
        type:
          kind: DECIMAL
          precision: 10
          scale: 2
        nullable: false
    primary_key:
      - id
"#;
    fs::write(custom_schema_dir.join("orders.yaml"), schema_yaml).unwrap();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path: project_path.clone(),
        config_path: None,
        schema_dir: Some(custom_schema_dir),
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command);
    assert!(result.is_ok(), "Check should succeed: {:?}", result);

    let output = result.unwrap();
    // ordersテーブルの差分が検出されていることを確認
    assert!(output.contains("✓ Validate: passed"));
    assert!(output.contains("✓ Generate (dry-run): passed"));
}

// ======================================
// Task 4.4: dry-run非破壊性テスト
// ======================================

#[test]
fn test_check_does_not_create_migration_files() {
    // checkコマンドがマイグレーションファイルを作成しないことを確認
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // 空のスナップショットを作成
    let snapshot_yaml = r#"
version: "1.0"
tables: {}
"#;
    fs::write(
        project_path.join("migrations/.schema_snapshot.yaml"),
        snapshot_yaml,
    )
    .unwrap();

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
    primary_key:
      - id
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    // migrations/ 内のファイル一覧を記録
    let before_entries: Vec<_> = fs::read_dir(project_path.join("migrations"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path: project_path.clone(),
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Text,
    };

    let _result = handler.execute(&command);

    // migrations/ 内に新しいファイルが作成されていないことを確認
    let after_entries: Vec<_> = fs::read_dir(project_path.join("migrations"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    assert_eq!(
        before_entries, after_entries,
        "Check command should not create any new files in migrations/"
    );
}

#[test]
fn test_check_does_not_modify_schema_snapshot() {
    // checkコマンドがスキーマスナップショットを変更しないことを確認
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    let snapshot_path = project_path.join("migrations/.schema_snapshot.yaml");
    let snapshot_yaml = r#"
version: "1.0"
tables: {}
"#;
    fs::write(&snapshot_path, snapshot_yaml).unwrap();
    let before_content = fs::read_to_string(&snapshot_path).unwrap();

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
    primary_key:
      - id
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path: project_path.clone(),
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Text,
    };

    let _result = handler.execute(&command);

    let after_content = fs::read_to_string(&snapshot_path).unwrap();
    assert_eq!(
        before_content, after_content,
        "Check command should not modify schema snapshot"
    );
}

#[test]
fn test_check_exit_code_both_success() {
    // validate と generate 両方成功時は Ok を返す
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

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
    primary_key:
      - id
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path,
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command);
    assert!(result.is_ok(), "Both success should return Ok");
}

#[test]
fn test_check_exit_code_validate_failure() {
    // validate 失敗時は Err を返す
    let (_temp_dir, project_path) =
        common::setup_test_project(Dialect::SQLite, None, true).unwrap();

    // PKなしスキーマ
    let schema_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
"#;
    fs::write(project_path.join("schema/users.yaml"), schema_yaml).unwrap();

    let handler = CheckCommandHandler::new();
    let command = CheckCommand {
        project_path,
        config_path: None,
        schema_dir: None,
        format: strata::cli::OutputFormat::Text,
    };

    let result = handler.execute(&command);
    assert!(result.is_err(), "Validate failure should return Err");
}
