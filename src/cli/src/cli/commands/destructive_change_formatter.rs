use crate::core::destructive_change_report::DestructiveChangeReport;
use colored::Colorize;

pub struct DestructiveChangeFormatter;

impl Default for DestructiveChangeFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl DestructiveChangeFormatter {
    pub fn new() -> Self {
        Self
    }

    pub fn format_error(&self, report: &DestructiveChangeReport, command: &str) -> String {
        let mut output = String::new();

        output.push_str(format!("{}\n\n", "Destructive changes detected".red().bold()).as_str());

        for line in format_change_lines(report) {
            output.push_str(line.red().to_string().as_str());
            output.push('\n');
        }

        output.push('\n');
        output.push_str("To proceed, choose one of the following:\n");
        output.push_str(&format!("  1. Review changes: {} --dry-run\n", command));
        output.push_str(&format!(
            "  2. Allow destructive changes: {} --allow-destructive\n",
            command
        ));
        output.push_str("  3. Reconsider your schema changes\n");

        output
    }

    pub fn format_warning(&self, report: &DestructiveChangeReport) -> String {
        let mut output = String::new();

        output.push_str(
            format!(
                "{}\n",
                "Warning: Destructive changes allowed".yellow().bold()
            )
            .as_str(),
        );

        let summary_lines = format_change_lines(report);
        if summary_lines.is_empty() {
            output.push_str("  No destructive changes were listed.\n");
            return output;
        }

        for line in summary_lines {
            output.push_str(&format!("  {}\n", line.yellow()));
        }

        output
    }
}

fn format_change_lines(report: &DestructiveChangeReport) -> Vec<String> {
    let mut lines = Vec::new();

    if !report.tables_dropped.is_empty() {
        lines.push(format!(
            "Tables to be dropped: {}",
            report.tables_dropped.join(", ")
        ));
    }

    if !report.columns_dropped.is_empty() {
        lines.push("Columns to be dropped:".to_string());
        for entry in &report.columns_dropped {
            lines.push(format!("  - {}: {}", entry.table, entry.columns.join(", ")));
        }
    }

    if !report.columns_renamed.is_empty() {
        lines.push("Columns to be renamed:".to_string());
        for entry in &report.columns_renamed {
            lines.push(format!(
                "  - {}: {} -> {}",
                entry.table, entry.old_name, entry.new_name
            ));
        }
    }

    if !report.enums_dropped.is_empty() {
        lines.push(format!(
            "Enums to be dropped: {}",
            report.enums_dropped.join(", ")
        ));
    }

    if !report.enums_recreated.is_empty() {
        lines.push(format!(
            "Enums to be recreated: {}",
            report.enums_recreated.join(", ")
        ));
    }

    if !report.views_dropped.is_empty() {
        lines.push(format!(
            "Views to be dropped: {}",
            report.views_dropped.join(", ")
        ));
    }

    if !report.views_modified.is_empty() {
        lines.push(format!(
            "Views with definition changes: {}",
            report.views_modified.join(", ")
        ));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::DestructiveChangeFormatter;
    use crate::core::destructive_change_report::{
        DestructiveChangeReport, DroppedColumn, RenamedColumnInfo,
    };

    fn sample_report() -> DestructiveChangeReport {
        DestructiveChangeReport {
            tables_dropped: vec!["users".to_string()],
            columns_dropped: vec![DroppedColumn {
                table: "products".to_string(),
                columns: vec!["legacy_field".to_string(), "unused".to_string()],
            }],
            columns_renamed: vec![RenamedColumnInfo {
                table: "orders".to_string(),
                old_name: "old_status".to_string(),
                new_name: "status".to_string(),
            }],
            enums_dropped: vec!["old_status".to_string()],
            enums_recreated: vec!["priority".to_string()],
            views_dropped: vec!["old_summary".to_string()],
            views_modified: vec!["active_users".to_string()],
        }
    }

    #[test]
    fn format_error_includes_grouped_changes_and_actions() {
        let formatter = DestructiveChangeFormatter::new();
        let output = formatter.format_error(&sample_report(), "strata generate");

        assert!(output.contains("Destructive changes detected"));
        assert!(output.contains("Tables to be dropped: users"));
        assert!(output.contains("Columns to be dropped:"));
        assert!(output.contains("products: legacy_field, unused"));
        assert!(output.contains("Columns to be renamed:"));
        assert!(output.contains("orders: old_status -> status"));
        assert!(output.contains("Enums to be dropped: old_status"));
        assert!(output.contains("Enums to be recreated: priority"));
        assert!(output.contains("Views to be dropped: old_summary"));
        assert!(output.contains("Views with definition changes: active_users"));
        assert!(output.contains("Review changes: strata generate --dry-run"));
        assert!(output.contains("Allow destructive changes: strata generate --allow-destructive"));
    }

    #[test]
    fn format_warning_includes_summary() {
        let formatter = DestructiveChangeFormatter::new();
        let output = formatter.format_warning(&sample_report());

        assert!(output.contains("Warning: Destructive changes allowed"));
        assert!(output.contains("Tables to be dropped: users"));
        assert!(output.contains("Columns to be dropped:"));
        assert!(output.contains("products: legacy_field, unused"));
    }
}
