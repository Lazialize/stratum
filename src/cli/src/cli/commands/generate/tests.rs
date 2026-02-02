use super::*;
use crate::core::schema::Schema;

#[test]
fn test_new_handler() {
    let handler = GenerateCommandHandler::new();
    assert!(format!("{:?}", handler).contains("GenerateCommandHandler"));
}

#[test]
fn test_generate_auto_description() {
    use crate::core::schema::Table;
    use crate::core::schema_diff::SchemaDiff;

    let handler = GenerateCommandHandler::new();

    let mut diff = SchemaDiff::new();
    let table = Table::new("users".to_string());
    diff.added_tables.push(table);

    let description = handler.generate_auto_description(&diff);
    assert!(description.contains("users"));
}

#[test]
fn test_generate_command_has_dry_run_field() {
    let command = GenerateCommand {
        project_path: std::path::PathBuf::from("/tmp"),
        config_path: None,
        schema_dir: None,
        description: Some("test".to_string()),
        dry_run: true,
        allow_destructive: false,
        verbose: false,
        format: crate::cli::OutputFormat::Text,
    };
    assert!(command.dry_run);
}

#[test]
fn test_execute_dry_run_output_format() {
    use crate::core::destructive_change_report::DestructiveChangeReport;
    use crate::core::error::ValidationResult;
    use crate::core::schema_diff::SchemaDiff;

    let handler = GenerateCommandHandler::new();
    let diff = SchemaDiff::new();
    let validation_result = ValidationResult::new();
    let destructive_report = DestructiveChangeReport::new();

    let result = handler.execute_dry_run(
        "20260124120000_test",
        "CREATE TABLE users (id INTEGER);",
        "DROP TABLE users;",
        &diff,
        &validation_result,
        &destructive_report,
    );

    assert!(result.is_ok());
    let output = result.unwrap();
    // ANSI escape codes may be present, so just check key content
    assert!(output.contains("Dry Run"));
    assert!(output.contains("Migration"));
    assert!(output.contains("UP SQL"));
    assert!(output.contains("DOWN SQL"));
    assert!(output.contains("Summary"));
}

#[test]
fn test_execute_dry_run_includes_destructive_preview() {
    use crate::core::destructive_change_report::{DestructiveChangeReport, DroppedColumn};
    use crate::core::error::ValidationResult;
    use crate::core::schema_diff::SchemaDiff;

    let handler = GenerateCommandHandler::new();
    let diff = SchemaDiff::new();
    let validation_result = ValidationResult::new();
    let destructive_report = DestructiveChangeReport {
        tables_dropped: vec!["users".to_string()],
        columns_dropped: vec![DroppedColumn {
            table: "orders".to_string(),
            columns: vec!["legacy".to_string()],
        }],
        columns_renamed: Vec::new(),
        enums_dropped: Vec::new(),
        enums_recreated: Vec::new(),
        views_dropped: Vec::new(),
        views_modified: Vec::new(),
    };

    let result = handler.execute_dry_run(
        "20260124120000_drop_table",
        "DROP TABLE users;",
        "CREATE TABLE users (id INTEGER);",
        &diff,
        &validation_result,
        &destructive_report,
    );

    let output = result.expect("dry-run output");
    assert!(output.contains("Destructive Changes Detected"));
    assert!(output.contains("DROP TABLE: users"));
    assert!(output.contains("DROP COLUMN: orders.legacy"));
    assert!(output.contains("--allow-destructive"));
}

#[test]
fn test_execute_dry_run_with_warnings() {
    use crate::core::destructive_change_report::DestructiveChangeReport;
    use crate::core::error::{ErrorLocation, ValidationResult, ValidationWarning};
    use crate::core::schema_diff::SchemaDiff;

    let handler = GenerateCommandHandler::new();
    let diff = SchemaDiff::new();
    let mut validation_result = ValidationResult::new();
    let destructive_report = DestructiveChangeReport::new();
    validation_result.add_warning(ValidationWarning::data_loss(
        "VARCHAR(255) → VARCHAR(100) may truncate data".to_string(),
        Some(ErrorLocation {
            table: Some("users".to_string()),
            column: Some("email".to_string()),
            line: None,
        }),
    ));

    let result = handler.execute_dry_run(
        "20260124120000_test",
        "ALTER TABLE users ...",
        "ALTER TABLE users ...",
        &diff,
        &validation_result,
        &destructive_report,
    );

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("Warnings"));
    assert!(output.contains("1"));
}

#[test]
fn test_execute_dry_run_with_error() {
    use crate::core::schema::{Column, ColumnType};
    use crate::core::schema_diff::{ColumnDiff, SchemaDiff, TableDiff};

    let handler = GenerateCommandHandler::new();

    // 型変更を含むdiffを作成
    let mut diff = SchemaDiff::new();
    let old_column = Column::new("data".to_string(), ColumnType::JSONB, false);
    let new_column = Column::new(
        "data".to_string(),
        ColumnType::INTEGER { precision: None },
        false,
    );
    let column_diff = ColumnDiff::new("data".to_string(), old_column, new_column);
    let mut table_diff = TableDiff::new("documents".to_string());
    table_diff.modified_columns.push(column_diff);
    diff.modified_tables.push(table_diff);

    let error_message =
        "Type change validation failed:\nType conversion error: JSONB → INTEGER is not supported";

    let result = handler.execute_dry_run_with_error("20260124120000_test", error_message, &diff);

    // エラーとして返されることを確認
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Type Changes") || err.contains("Errors"));
}

#[test]
fn test_execute_dry_run_with_renames() {
    use crate::core::destructive_change_report::DestructiveChangeReport;
    use crate::core::error::ValidationResult;
    use crate::core::schema::{Column, ColumnType};
    use crate::core::schema_diff::{RenamedColumn, SchemaDiff, TableDiff};

    let handler = GenerateCommandHandler::new();

    let mut diff = SchemaDiff::new();
    let old_column = Column::new(
        "name".to_string(),
        ColumnType::VARCHAR { length: 100 },
        false,
    );
    let new_column = Column::new(
        "user_name".to_string(),
        ColumnType::VARCHAR { length: 100 },
        false,
    );
    let renamed = RenamedColumn {
        old_name: "name".to_string(),
        old_column,
        new_column,
        changes: vec![],
    };

    let mut table_diff = TableDiff::new("users".to_string());
    table_diff.renamed_columns.push(renamed);
    diff.modified_tables.push(table_diff);

    let validation_result = ValidationResult::new();
    let destructive_report = DestructiveChangeReport::new();

    let result = handler.execute_dry_run(
        "20260124120000_rename_column",
        "ALTER TABLE users RENAME COLUMN name TO user_name;",
        "ALTER TABLE users RENAME COLUMN user_name TO name;",
        &diff,
        &validation_result,
        &destructive_report,
    );

    assert!(result.is_ok());
    let output = result.unwrap();
    // リネーム情報セクションが表示されることを確認
    assert!(
        output.contains("Column Renames"),
        "Should contain 'Column Renames' section, got: {}",
        output
    );
    // リネーム情報が表示されることを確認
    assert!(
        output.contains("name") && output.contains("user_name"),
        "Should contain rename info, got: {}",
        output
    );
    // UP SQLにリネームSQLが含まれることを確認
    assert!(
        output.contains("RENAME COLUMN"),
        "Should contain RENAME COLUMN SQL, got: {}",
        output
    );
}

#[test]
fn test_dry_run_displays_rename_sql_preview() {
    // Task 6.2: dry-runモードでリネームSQLがプレビュー表示されることを確認
    use crate::core::destructive_change_report::DestructiveChangeReport;
    use crate::core::error::ValidationResult;
    use crate::core::schema::{Column, ColumnType};
    use crate::core::schema_diff::{RenamedColumn, SchemaDiff, TableDiff};

    let handler = GenerateCommandHandler::new();

    let mut diff = SchemaDiff::new();
    let old_column = Column::new(
        "email".to_string(),
        ColumnType::VARCHAR { length: 255 },
        false,
    );
    let new_column = Column::new(
        "email_address".to_string(),
        ColumnType::VARCHAR { length: 255 },
        false,
    );
    let renamed = RenamedColumn {
        old_name: "email".to_string(),
        old_column,
        new_column,
        changes: vec![],
    };

    let mut table_diff = TableDiff::new("contacts".to_string());
    table_diff.renamed_columns.push(renamed);
    diff.modified_tables.push(table_diff);

    let validation_result = ValidationResult::new();
    let destructive_report = DestructiveChangeReport::new();

    let up_sql = "ALTER TABLE contacts RENAME COLUMN email TO email_address;";
    let down_sql = "ALTER TABLE contacts RENAME COLUMN email_address TO email;";

    let result = handler.execute_dry_run(
        "20260124120000_rename_email",
        up_sql,
        down_sql,
        &diff,
        &validation_result,
        &destructive_report,
    );

    assert!(result.is_ok());
    let output = result.unwrap();

    // Column Renamesセクションが表示される
    assert!(output.contains("Column Renames"));
    // リネーム元/先が表示される
    assert!(output.contains("email"));
    assert!(output.contains("email_address"));
    // UP SQLセクションにリネームSQLが含まれる
    assert!(output.contains("UP SQL"));
    assert!(output.contains("RENAME COLUMN email TO email_address"));
    // DOWN SQLセクションに逆リネームSQLが含まれる
    assert!(output.contains("DOWN SQL"));
    assert!(output.contains("RENAME COLUMN email_address TO email"));
}

#[test]
fn test_generate_renamed_from_remove_warnings() {
    // renamed_from属性の削除推奨警告が生成されることを確認
    use crate::core::error::WarningKind;
    use crate::core::schema::{Column, ColumnType, Table};

    let handler = GenerateCommandHandler::new();

    let mut schema = Schema::new("1.0".to_string());
    let mut table = Table::new("users".to_string());
    let mut column = Column::new(
        "user_name".to_string(),
        ColumnType::VARCHAR { length: 100 },
        false,
    );
    column.renamed_from = Some("name".to_string());
    table.columns.push(column);
    schema.tables.insert("users".to_string(), table);

    let warnings = handler.generate_renamed_from_remove_warnings(&schema);
    assert_eq!(warnings.len(), 1);
    assert!(matches!(
        warnings[0].kind,
        WarningKind::RenamedFromRemoveRecommendation
    ));
    assert!(warnings[0].message.contains("renamed_from"));
}

#[test]
fn test_generate_enum_recreate_deprecation_warning() {
    use crate::core::error::WarningKind;

    let handler = GenerateCommandHandler::new();
    let mut schema = Schema::new("1.0".to_string());
    schema.enum_recreate_allowed = true;

    let warning = handler
        .generate_enum_recreate_deprecation_warning(&schema)
        .expect("warning should exist");

    assert_eq!(warning.kind, WarningKind::Compatibility);
    assert!(warning.message.contains("enum_recreate_allowed"));
}

// ======================================
// Task 4.2: スナップショット保存の新構文テスト
// ======================================

#[test]
fn test_snapshot_serialization_uses_new_syntax() {
    use crate::core::schema::{Column, ColumnType, Constraint, Index, Table};
    use crate::services::schema_io::schema_serializer::SchemaSerializerService;

    // 内部モデルを作成
    let mut schema = Schema::new("1.0".to_string());
    let mut table = Table::new("products".to_string());
    table.add_column(Column::new(
        "id".to_string(),
        ColumnType::INTEGER { precision: None },
        false,
    ));
    table.add_column(Column::new(
        "name".to_string(),
        ColumnType::VARCHAR { length: 255 },
        false,
    ));
    table.add_constraint(Constraint::PRIMARY_KEY {
        columns: vec!["id".to_string()],
    });
    table.add_index(Index::new(
        "idx_name".to_string(),
        vec!["name".to_string()],
        false,
    ));
    schema.add_table(table);

    // シリアライザーサービスを使用してシリアライズ
    let serializer = SchemaSerializerService::new();
    let yaml = serializer.serialize_to_string(&schema).unwrap();

    // 新構文形式の確認
    // 1. テーブル名がキーとして出力される
    assert!(yaml.contains("products:"));
    // 2. nameフィールドは出力されない
    assert!(!yaml.contains("name: products"));
    // 3. primary_keyフィールドが出力される
    assert!(yaml.contains("primary_key:"));
    // 4. constraints内にPRIMARY_KEYは含まれない
    assert!(!yaml.contains("type: PRIMARY_KEY"));
}

#[test]
fn test_generate_output_json_serialization() {
    let output = GenerateOutput {
        dry_run: true,
        migration_name: Some("20260121120000_create_users".to_string()),
        migration_path: Some("/path/to/migrations/20260121120000_create_users".to_string()),
        up_sql: Some("CREATE TABLE users (id INTEGER PRIMARY KEY);".to_string()),
        down_sql: Some("DROP TABLE users;".to_string()),
        warnings: vec!["destructive change".to_string()],
        message: "should not appear in JSON".to_string(),
    };

    let json = serde_json::to_string_pretty(&output).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // message は #[serde(skip)] のため含まれない
    assert!(parsed.get("message").is_none());
    assert_eq!(parsed["dry_run"], true);
    assert_eq!(parsed["migration_name"], "20260121120000_create_users");
    assert_eq!(
        parsed["migration_path"],
        "/path/to/migrations/20260121120000_create_users"
    );
    assert!(parsed["up_sql"].as_str().unwrap().contains("CREATE TABLE"));
    assert_eq!(parsed["warnings"][0], "destructive change");

    // None フィールドはスキップされる
    let output_minimal = GenerateOutput {
        dry_run: false,
        migration_name: None,
        migration_path: None,
        up_sql: None,
        down_sql: None,
        warnings: vec![],
        message: "text".to_string(),
    };
    let json2 = serde_json::to_string_pretty(&output_minimal).unwrap();
    let parsed2: serde_json::Value = serde_json::from_str(&json2).unwrap();
    assert!(parsed2.get("migration_name").is_none());
    assert!(parsed2.get("migration_path").is_none());
    assert!(parsed2.get("up_sql").is_none());
    assert!(parsed2.get("down_sql").is_none());
}
