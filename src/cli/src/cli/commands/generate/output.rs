use super::GenerateCommandHandler;
use crate::cli::commands::dry_run_formatter::DryRunFormatter;
use anyhow::{anyhow, Result};

impl GenerateCommandHandler {
    /// dry-runモードの実行
    pub(super) fn execute_dry_run(
        &self,
        migration_name: &str,
        up_sql: &str,
        down_sql: &str,
        diff: &crate::core::schema_diff::SchemaDiff,
        validation_result: &crate::core::error::ValidationResult,
        destructive_report: &crate::core::destructive_change_report::DestructiveChangeReport,
    ) -> Result<String> {
        Ok(DryRunFormatter::format(
            migration_name,
            up_sql,
            down_sql,
            diff,
            validation_result,
            destructive_report,
        ))
    }

    /// dry-runモードでのエラー表示
    pub(super) fn execute_dry_run_with_error(
        &self,
        migration_name: &str,
        error: &str,
        diff: &crate::core::schema_diff::SchemaDiff,
    ) -> Result<String> {
        Err(anyhow!(
            "{}",
            DryRunFormatter::format_error(migration_name, error, diff)
        ))
    }
}
