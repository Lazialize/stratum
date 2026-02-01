use super::{DiffValidationResult, GenerateCommandHandler};
use crate::cli::commands::destructive_change_formatter::DestructiveChangeFormatter;
use crate::core::schema::Schema;
use crate::services::destructive_change_detector::DestructiveChangeDetector;
use anyhow::{anyhow, Result};

impl GenerateCommandHandler {
    /// 差分検出・バリデーション
    ///
    /// 差分がない場合は `Ok(None)` を返す
    pub(super) fn detect_and_validate_diff(
        &self,
        command: &super::GenerateCommand,
        current_schema: &Schema,
        previous_schema: &Schema,
    ) -> Result<Option<DiffValidationResult>> {
        let (diff, diff_warnings) = self
            .services
            .diff_detector
            .detect_diff_with_warnings(previous_schema, current_schema);

        if diff.is_empty() {
            return Ok(None);
        }

        // 破壊的変更の検出
        let destructive_detector = DestructiveChangeDetector::new();
        let destructive_report = destructive_detector.detect(&diff);

        // リネーム検証
        let rename_validation = self
            .services
            .validator
            .validate_renames_with_old_schema(previous_schema, current_schema);

        let renamed_from_warnings = self.generate_renamed_from_remove_warnings(current_schema);

        // マイグレーション名の生成
        let timestamp = self.services.generator.generate_timestamp();
        let description = command
            .description
            .clone()
            .unwrap_or_else(|| self.generate_auto_description(&diff));
        let sanitized_description = self.services.generator.sanitize_description(&description);
        let migration_name = self
            .services
            .generator
            .generate_migration_filename(&timestamp, &sanitized_description);

        // リネーム検証エラーがある場合は処理を中止
        if !rename_validation.is_valid() {
            return Err(anyhow!(
                "Rename validation errors:\n{}",
                rename_validation.errors_to_string()
            ));
        }

        // 破壊的変更がある場合はデフォルト拒否
        if destructive_report.has_destructive_changes()
            && !command.allow_destructive
            && !command.dry_run
        {
            let formatter = DestructiveChangeFormatter::new();
            return Err(anyhow!(
                formatter.format_error(&destructive_report, "strata generate")
            ));
        }

        Ok(Some(DiffValidationResult {
            diff,
            diff_warnings,
            destructive_report,
            rename_validation,
            renamed_from_warnings,
            migration_name,
            timestamp,
            sanitized_description,
        }))
    }

    /// renamed_from属性削除推奨警告を生成
    pub(super) fn generate_renamed_from_remove_warnings(
        &self,
        schema: &Schema,
    ) -> Vec<crate::core::error::ValidationWarning> {
        use crate::core::error::{ErrorLocation, ValidationWarning};

        let mut warnings = Vec::new();

        for (table_name, table) in &schema.tables {
            for column in &table.columns {
                if column.renamed_from.is_some() {
                    let location = Some(ErrorLocation::with_table_and_column(
                        table_name,
                        &column.name,
                    ));
                    warnings.push(ValidationWarning::renamed_from_remove_recommendation(
                        format!(
                            "Column '{}.{}' still has 'renamed_from' attribute. Consider removing it after migration is applied.",
                            table_name, column.name
                        ),
                        location,
                    ));
                }
            }
        }

        warnings
    }

    pub(super) fn generate_enum_recreate_deprecation_warning(
        &self,
        schema: &Schema,
    ) -> Option<crate::core::error::ValidationWarning> {
        use crate::core::error::{ValidationWarning, WarningKind};

        if schema.enum_recreate_allowed {
            Some(ValidationWarning::new(
                "Warning: 'enum_recreate_allowed' is deprecated. Use '--allow-destructive' instead."
                    .to_string(),
                None,
                WarningKind::Compatibility,
            ))
        } else {
            None
        }
    }
}
