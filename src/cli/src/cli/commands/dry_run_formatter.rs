// dry-runモードのフォーマッター
//
// generateコマンドのdry-run出力を構造化して生成します。
// セクション別のフォーマットロジックを分離し、
// ビジネスロジックと表示整形の責務を分けます。

use crate::core::destructive_change_report::DestructiveChangeReport;
use crate::core::error::ValidationResult;
use crate::core::schema_diff::SchemaDiff;
use colored::Colorize;
use std::fmt::Write;

/// 型変更情報
#[derive(Debug)]
pub(crate) struct TypeChangeInfo {
    pub table: String,
    pub column: String,
    pub old_type: String,
    pub new_type: String,
}

/// リネーム情報
#[derive(Debug)]
pub(crate) struct RenameInfo {
    pub table: String,
    pub old_name: String,
    pub new_name: String,
}

/// dry-run出力のフォーマッター
pub(crate) struct DryRunFormatter;

impl DryRunFormatter {
    /// dry-run出力全体をフォーマット
    pub fn format(
        migration_name: &str,
        up_sql: &str,
        down_sql: &str,
        diff: &SchemaDiff,
        validation_result: &ValidationResult,
        destructive_report: &DestructiveChangeReport,
    ) -> String {
        let mut output = String::new();

        Self::append_header(&mut output, migration_name);

        let rename_changes = Self::collect_rename_changes(diff);
        Self::append_rename_section(&mut output, &rename_changes);

        let type_changes = Self::collect_type_changes(diff);
        Self::append_type_change_section(&mut output, &type_changes);

        if destructive_report.has_destructive_changes() {
            Self::append_destructive_section(&mut output, destructive_report);
        }

        Self::append_warning_section(&mut output, validation_result);
        Self::append_sql_section(&mut output, "UP SQL", up_sql);
        Self::append_sql_section(&mut output, "DOWN SQL", down_sql);
        Self::append_summary(&mut output, validation_result);

        output
    }

    /// dry-runエラー出力をフォーマット
    pub fn format_error(migration_name: &str, error: &str, diff: &SchemaDiff) -> String {
        let mut output = String::new();

        Self::append_header(&mut output, migration_name);

        let type_changes = Self::collect_type_changes(diff);
        Self::append_type_change_section(&mut output, &type_changes);

        Self::append_error_section(&mut output, error);
        Self::append_suggestion_section(&mut output);
        Self::append_error_summary(&mut output);

        output
    }

    // --- セクション別フォーマッタ ---

    fn append_header(output: &mut String, migration_name: &str) {
        writeln!(output, "{}", "=== Dry Run: Migration Preview ===".bold()).unwrap();
        writeln!(output, "Migration: {}", migration_name.cyan()).unwrap();
        writeln!(output).unwrap();
    }

    fn append_rename_section(output: &mut String, rename_changes: &[RenameInfo]) {
        if rename_changes.is_empty() {
            return;
        }
        writeln!(output, "{}", "--- Column Renames ---".bold()).unwrap();
        for rename in rename_changes {
            let table = rename.table.cyan();
            let arrow = "→".bold();
            writeln!(
                output,
                "  {}: {} {} {}",
                table, rename.old_name, arrow, rename.new_name
            )
            .unwrap();
        }
        writeln!(output).unwrap();
    }

    fn append_type_change_section(output: &mut String, type_changes: &[TypeChangeInfo]) {
        if type_changes.is_empty() {
            return;
        }
        writeln!(output, "{}", "--- Type Changes ---".bold()).unwrap();
        for change in type_changes {
            let location = format!("{}.{}", change.table, change.column).cyan();
            let arrow = "→".bold();
            writeln!(
                output,
                "  {}: {} {} {}",
                location, change.old_type, arrow, change.new_type
            )
            .unwrap();
        }
        writeln!(output).unwrap();
    }

    fn append_destructive_section(
        output: &mut String,
        destructive_report: &DestructiveChangeReport,
    ) {
        writeln!(output, "{}", "⚠ Destructive Changes Detected".red().bold()).unwrap();

        for table in &destructive_report.tables_dropped {
            writeln!(output, "  {}", format!("DROP TABLE: {}", table).red()).unwrap();
        }

        for entry in &destructive_report.columns_dropped {
            for column in &entry.columns {
                writeln!(
                    output,
                    "  {}",
                    format!("DROP COLUMN: {}.{}", entry.table, column).red()
                )
                .unwrap();
            }
        }

        for entry in &destructive_report.columns_renamed {
            writeln!(
                output,
                "  {}",
                format!(
                    "RENAME COLUMN: {}.{} -> {}",
                    entry.table, entry.old_name, entry.new_name
                )
                .red()
            )
            .unwrap();
        }

        for enum_name in &destructive_report.enums_dropped {
            writeln!(output, "  {}", format!("DROP ENUM: {}", enum_name).red()).unwrap();
        }

        for enum_name in &destructive_report.enums_recreated {
            writeln!(
                output,
                "  {}",
                format!("RECREATE ENUM: {}", enum_name).red()
            )
            .unwrap();
        }

        for view_name in &destructive_report.views_dropped {
            writeln!(output, "  {}", format!("DROP VIEW: {}", view_name).red()).unwrap();
        }

        for view_name in &destructive_report.views_modified {
            writeln!(output, "  {}", format!("MODIFY VIEW: {}", view_name).red()).unwrap();
        }

        let dropped_column_count: usize = destructive_report
            .columns_dropped
            .iter()
            .map(|entry| entry.columns.len())
            .sum();

        writeln!(
            output,
            "  {}",
            format!(
                "Impact summary: tables dropped={}, columns dropped={}, columns renamed={}, enums dropped={}, enums recreated={}, views dropped={}, views modified={}",
                destructive_report.tables_dropped.len(),
                dropped_column_count,
                destructive_report.columns_renamed.len(),
                destructive_report.enums_dropped.len(),
                destructive_report.enums_recreated.len(),
                destructive_report.views_dropped.len(),
                destructive_report.views_modified.len()
            )
            .red()
        )
        .unwrap();

        writeln!(
            output,
            "\n{}",
            "To proceed, run with --allow-destructive flag".red()
        )
        .unwrap();
        writeln!(output).unwrap();
    }

    fn append_warning_section(output: &mut String, validation_result: &ValidationResult) {
        if validation_result.warning_count() == 0 {
            return;
        }
        writeln!(
            output,
            "{}",
            format!("--- Warnings ({}) ---", validation_result.warning_count())
                .yellow()
                .bold()
        )
        .unwrap();
        for warning in &validation_result.warnings {
            let location = warning
                .location
                .as_ref()
                .map(|loc| {
                    let table = loc.table.as_deref().unwrap_or("");
                    let column = loc
                        .column
                        .as_ref()
                        .map(|c| format!(".{}", c))
                        .unwrap_or_default();
                    format!("[{}{}]", table, column).cyan().to_string()
                })
                .unwrap_or_default();
            writeln!(
                output,
                "  {} {} {}",
                "⚠".yellow(),
                location,
                warning.message.yellow()
            )
            .unwrap();
        }
        writeln!(output).unwrap();
    }

    fn append_sql_section(output: &mut String, label: &str, sql: &str) {
        writeln!(output, "{}", format!("--- {} ---", label).bold()).unwrap();
        writeln!(output, "{}", sql).unwrap();
        writeln!(output).unwrap();
    }

    fn append_summary(output: &mut String, validation_result: &ValidationResult) {
        writeln!(output, "{}", "=== Summary ===".bold()).unwrap();
        let warning_count = validation_result.warning_count();
        let warning_text = if warning_count > 0 {
            format!("Warnings: {}", warning_count)
                .yellow()
                .bold()
                .to_string()
        } else {
            format!("Warnings: {}", warning_count).green().to_string()
        };
        writeln!(output, "{}", warning_text).unwrap();
        writeln!(
            output,
            "Files would be created: 3 (up.sql, down.sql, .meta.yaml)"
        )
        .unwrap();
        writeln!(
            output,
            "\n{}",
            "No files were created (dry-run mode).".dimmed()
        )
        .unwrap();
    }

    fn append_error_section(output: &mut String, error: &str) {
        writeln!(output, "{}", "--- Errors ---".red().bold()).unwrap();
        for line in error.lines() {
            if line.starts_with("Type change validation failed:") {
                writeln!(output, "{}", line.red()).unwrap();
            } else if !line.is_empty() {
                writeln!(output, "  {} {}", "✗".red(), line.red()).unwrap();
            }
        }
        writeln!(output).unwrap();
    }

    fn append_suggestion_section(output: &mut String) {
        writeln!(output, "{}", "--- Suggestion ---".green().bold()).unwrap();
        writeln!(
            output,
            "  {}",
            "Use TEXT as an intermediate type or reconsider the type change".green()
        )
        .unwrap();
        writeln!(output).unwrap();
    }

    fn append_error_summary(output: &mut String) {
        writeln!(output, "{}", "=== Summary ===".bold()).unwrap();
        writeln!(
            output,
            "{}",
            "Errors: 1 (migration cannot be generated)".red().bold()
        )
        .unwrap();
        writeln!(
            output,
            "\n{}",
            "Migration generation aborted due to type conversion errors.".red()
        )
        .unwrap();
    }

    // --- データ収集ヘルパー ---

    /// 型変更情報を収集
    pub(crate) fn collect_type_changes(diff: &SchemaDiff) -> Vec<TypeChangeInfo> {
        let mut changes = Vec::new();

        for table_diff in &diff.modified_tables {
            for column_diff in &table_diff.modified_columns {
                for change in &column_diff.changes {
                    if let crate::core::schema_diff::ColumnChange::TypeChanged {
                        old_type,
                        new_type,
                    } = change
                    {
                        changes.push(TypeChangeInfo {
                            table: table_diff.table_name.clone(),
                            column: column_diff.column_name.clone(),
                            old_type: old_type.clone(),
                            new_type: new_type.clone(),
                        });
                    }
                }
            }

            for renamed in &table_diff.renamed_columns {
                for change in &renamed.changes {
                    if let crate::core::schema_diff::ColumnChange::TypeChanged {
                        old_type,
                        new_type,
                    } = change
                    {
                        changes.push(TypeChangeInfo {
                            table: table_diff.table_name.clone(),
                            column: renamed.new_column.name.clone(),
                            old_type: old_type.clone(),
                            new_type: new_type.clone(),
                        });
                    }
                }
            }
        }

        changes
    }

    /// リネーム情報を収集
    pub(crate) fn collect_rename_changes(diff: &SchemaDiff) -> Vec<RenameInfo> {
        let mut renames = Vec::new();

        for table_diff in &diff.modified_tables {
            for renamed in &table_diff.renamed_columns {
                renames.push(RenameInfo {
                    table: table_diff.table_name.clone(),
                    old_name: renamed.old_name.clone(),
                    new_name: renamed.new_column.name.clone(),
                });
            }
        }

        renames
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::destructive_change_report::{DroppedColumn, RenamedColumnInfo};
    use crate::core::error::{ErrorLocation, ValidationWarning, WarningKind};
    use crate::core::schema::{Column, ColumnType};
    use crate::core::schema_diff::{ColumnChange, ColumnDiff, RenamedColumn, TableDiff};

    fn make_column(name: &str, col_type: ColumnType) -> Column {
        Column::new(name.to_string(), col_type, false)
    }

    // --- collect_type_changes ---

    #[test]
    fn test_collect_type_changes_empty_diff() {
        let diff = SchemaDiff::new();
        let changes = DryRunFormatter::collect_type_changes(&diff);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_collect_type_changes_from_modified_columns() {
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_columns.push(ColumnDiff {
            column_name: "age".to_string(),
            old_column: make_column("age", ColumnType::INTEGER { precision: None }),
            new_column: make_column("age", ColumnType::TEXT),
            changes: vec![ColumnChange::TypeChanged {
                old_type: "INTEGER".to_string(),
                new_type: "TEXT".to_string(),
            }],
        });
        diff.modified_tables.push(table_diff);

        let changes = DryRunFormatter::collect_type_changes(&diff);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].table, "users");
        assert_eq!(changes[0].column, "age");
        assert_eq!(changes[0].old_type, "INTEGER");
        assert_eq!(changes[0].new_type, "TEXT");
    }

    #[test]
    fn test_collect_type_changes_from_renamed_columns() {
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(RenamedColumn {
            old_name: "old_col".to_string(),
            old_column: make_column("old_col", ColumnType::INTEGER { precision: None }),
            new_column: make_column("new_col", ColumnType::TEXT),
            changes: vec![ColumnChange::TypeChanged {
                old_type: "INTEGER".to_string(),
                new_type: "TEXT".to_string(),
            }],
        });
        diff.modified_tables.push(table_diff);

        let changes = DryRunFormatter::collect_type_changes(&diff);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].column, "new_col");
    }

    // --- collect_rename_changes ---

    #[test]
    fn test_collect_rename_changes_empty_diff() {
        let diff = SchemaDiff::new();
        let renames = DryRunFormatter::collect_rename_changes(&diff);
        assert!(renames.is_empty());
    }

    #[test]
    fn test_collect_rename_changes() {
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(RenamedColumn {
            old_name: "username".to_string(),
            old_column: make_column("username", ColumnType::VARCHAR { length: 255 }),
            new_column: make_column("display_name", ColumnType::VARCHAR { length: 255 }),
            changes: vec![],
        });
        diff.modified_tables.push(table_diff);

        let renames = DryRunFormatter::collect_rename_changes(&diff);
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].table, "users");
        assert_eq!(renames[0].old_name, "username");
        assert_eq!(renames[0].new_name, "display_name");
    }

    #[test]
    fn test_collect_rename_changes_only_no_type_change() {
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(RenamedColumn {
            old_name: "name".to_string(),
            old_column: make_column("name", ColumnType::VARCHAR { length: 100 }),
            new_column: make_column("user_name", ColumnType::VARCHAR { length: 100 }),
            changes: vec![],
        });
        diff.modified_tables.push(table_diff);

        let changes = DryRunFormatter::collect_type_changes(&diff);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_collect_type_changes_from_renamed_columns_includes_types() {
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(RenamedColumn {
            old_name: "name".to_string(),
            old_column: make_column("name", ColumnType::VARCHAR { length: 50 }),
            new_column: make_column("user_name", ColumnType::VARCHAR { length: 100 }),
            changes: vec![ColumnChange::TypeChanged {
                old_type: "VARCHAR(50)".to_string(),
                new_type: "VARCHAR(100)".to_string(),
            }],
        });
        diff.modified_tables.push(table_diff);

        let changes = DryRunFormatter::collect_type_changes(&diff);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].table, "users");
        assert_eq!(changes[0].column, "user_name");
        assert_eq!(changes[0].old_type, "VARCHAR(50)");
        assert_eq!(changes[0].new_type, "VARCHAR(100)");
    }

    // --- format ---

    #[test]
    fn test_format_contains_header() {
        let diff = SchemaDiff::new();
        let validation = ValidationResult::new();
        let report = DestructiveChangeReport::new();

        let output = DryRunFormatter::format(
            "test_migration",
            "SELECT 1;",
            "SELECT 2;",
            &diff,
            &validation,
            &report,
        );
        assert!(output.contains("Dry Run: Migration Preview"));
        assert!(output.contains("test_migration"));
    }

    #[test]
    fn test_format_contains_sql_sections() {
        let diff = SchemaDiff::new();
        let validation = ValidationResult::new();
        let report = DestructiveChangeReport::new();

        let output = DryRunFormatter::format(
            "m",
            "CREATE TABLE users;",
            "DROP TABLE users;",
            &diff,
            &validation,
            &report,
        );
        assert!(output.contains("UP SQL"));
        assert!(output.contains("CREATE TABLE users;"));
        assert!(output.contains("DOWN SQL"));
        assert!(output.contains("DROP TABLE users;"));
    }

    #[test]
    fn test_format_contains_summary() {
        let diff = SchemaDiff::new();
        let validation = ValidationResult::new();
        let report = DestructiveChangeReport::new();

        let output = DryRunFormatter::format("m", "", "", &diff, &validation, &report);
        assert!(output.contains("Summary"));
        assert!(output.contains("Warnings: 0"));
        assert!(output.contains("No files were created (dry-run mode)"));
    }

    #[test]
    fn test_format_with_warnings() {
        let diff = SchemaDiff::new();
        let mut validation = ValidationResult::new();
        validation.add_warning(ValidationWarning {
            message: "This is a test warning".to_string(),
            location: Some(ErrorLocation {
                table: Some("users".to_string()),
                column: Some("name".to_string()),
                line: None,
            }),
            kind: WarningKind::DialectSpecific,
        });
        let report = DestructiveChangeReport::new();

        let output = DryRunFormatter::format("m", "", "", &diff, &validation, &report);
        assert!(output.contains("Warnings (1)"));
        assert!(output.contains("This is a test warning"));
        assert!(output.contains("users.name"));
    }

    #[test]
    fn test_format_with_destructive_changes() {
        let diff = SchemaDiff::new();
        let validation = ValidationResult::new();
        let mut report = DestructiveChangeReport::new();
        report.tables_dropped.push("old_table".to_string());
        report.columns_dropped.push(DroppedColumn {
            table: "users".to_string(),
            columns: vec!["temp_col".to_string()],
        });
        report.columns_renamed.push(RenamedColumnInfo {
            table: "users".to_string(),
            old_name: "old_col".to_string(),
            new_name: "new_col".to_string(),
        });

        let output = DryRunFormatter::format("m", "", "", &diff, &validation, &report);
        assert!(output.contains("Destructive Changes Detected"));
        assert!(output.contains("DROP TABLE: old_table"));
        assert!(output.contains("DROP COLUMN: users.temp_col"));
        assert!(output.contains("RENAME COLUMN: users.old_col -> new_col"));
        assert!(output.contains("--allow-destructive"));
    }

    #[test]
    fn test_format_no_destructive_section_when_empty() {
        let diff = SchemaDiff::new();
        let validation = ValidationResult::new();
        let report = DestructiveChangeReport::new();

        let output = DryRunFormatter::format("m", "", "", &diff, &validation, &report);
        assert!(!output.contains("Destructive Changes Detected"));
    }

    // --- format_error ---

    #[test]
    fn test_format_error_contains_header_and_error() {
        let diff = SchemaDiff::new();

        let output = DryRunFormatter::format_error(
            "error_migration",
            "Type change validation failed:\nINTEGER to BOOLEAN is not allowed",
            &diff,
        );
        assert!(output.contains("Dry Run: Migration Preview"));
        assert!(output.contains("error_migration"));
        assert!(output.contains("Errors"));
        assert!(output.contains("Type change validation failed"));
        assert!(output.contains("INTEGER to BOOLEAN is not allowed"));
    }

    #[test]
    fn test_format_error_contains_suggestion() {
        let diff = SchemaDiff::new();
        let output = DryRunFormatter::format_error("m", "error", &diff);
        assert!(output.contains("Suggestion"));
        assert!(output.contains("TEXT as an intermediate type"));
    }

    #[test]
    fn test_format_error_contains_error_summary() {
        let diff = SchemaDiff::new();
        let output = DryRunFormatter::format_error("m", "error", &diff);
        assert!(output.contains("Errors: 1"));
        assert!(output.contains("migration cannot be generated"));
    }

    #[test]
    fn test_format_error_includes_type_changes() {
        let mut diff = SchemaDiff::new();
        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_columns.push(ColumnDiff {
            column_name: "status".to_string(),
            old_column: make_column("status", ColumnType::INTEGER { precision: None }),
            new_column: make_column("status", ColumnType::BOOLEAN),
            changes: vec![ColumnChange::TypeChanged {
                old_type: "INTEGER".to_string(),
                new_type: "BOOLEAN".to_string(),
            }],
        });
        diff.modified_tables.push(table_diff);

        let output = DryRunFormatter::format_error("m", "type error", &diff);
        assert!(output.contains("Type Changes"));
        assert!(output.contains("users.status"));
    }
}
