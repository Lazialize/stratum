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
        writeln!(
            output,
            "{}",
            "=== Dry Run: Migration Preview ===".bold()
        )
        .unwrap();
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

        let dropped_column_count: usize = destructive_report
            .columns_dropped
            .iter()
            .map(|entry| entry.columns.len())
            .sum();

        writeln!(
            output,
            "  {}",
            format!(
                "Impact summary: tables dropped={}, columns dropped={}, columns renamed={}, enums dropped={}, enums recreated={}",
                destructive_report.tables_dropped.len(),
                dropped_column_count,
                destructive_report.columns_renamed.len(),
                destructive_report.enums_dropped.len(),
                destructive_report.enums_recreated.len()
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
