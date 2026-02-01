use super::{DiffValidationResult, GenerateCommand, GenerateCommandHandler, GeneratedSql};
use crate::core::config::Config;
use crate::core::schema::Schema;
use anyhow::{anyhow, Context, Result};

impl GenerateCommandHandler {
    /// SQL生成と警告統合
    pub(super) fn generate_migration_sql(
        &self,
        command: &GenerateCommand,
        config: &Config,
        dvr: &DiffValidationResult,
        current_schema: &Schema,
        previous_schema: &Schema,
    ) -> Result<GeneratedSql> {
        let allow_destructive_for_sql = command.allow_destructive || command.dry_run;

        let sql_result = self.services.generator.generate_up_sql_with_schemas(
            &dvr.diff,
            previous_schema,
            current_schema,
            config.dialect,
            allow_destructive_for_sql,
        );

        // 型変更検証エラーの処理
        if let Err(e) = &sql_result {
            if command.dry_run {
                let error_msg = e.to_string();
                return Err(self
                    .execute_dry_run_with_error(&dvr.migration_name, &error_msg, &dvr.diff)
                    .unwrap_err());
            }
            return Err(anyhow!("{}", e));
        }

        let (up_sql, mut validation_result) = sql_result.unwrap();

        // 全警告を統合
        for warning in &dvr.diff_warnings {
            validation_result.add_warning(warning.clone());
        }
        for warning in &dvr.rename_validation.warnings {
            validation_result.add_warning(warning.clone());
        }
        for warning in &dvr.renamed_from_warnings {
            validation_result.add_warning(warning.clone());
        }
        if let Some(warning) = self.generate_enum_recreate_deprecation_warning(current_schema) {
            validation_result.add_warning(warning);
        }

        let (down_sql, _) = self
            .services
            .generator
            .generate_down_sql_with_schemas(
                &dvr.diff,
                previous_schema,
                current_schema,
                config.dialect,
                allow_destructive_for_sql,
            )
            .context("Failed to generate DOWN SQL")?;

        Ok(GeneratedSql {
            up_sql,
            down_sql,
            validation_result,
        })
    }
}
