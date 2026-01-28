// generateコマンドハンドラー
//
// スキーマ差分検出とマイグレーションファイル生成を実装します。
// - スキーマ定義の読み込み
// - 前回のスキーマ状態の読み込み
// - 差分検出とマイグレーションファイル生成
// - 生成されたファイルパスの表示

use crate::cli::command_context::CommandContext;
use crate::cli::commands::destructive_change_formatter::DestructiveChangeFormatter;
use crate::cli::commands::dry_run_formatter::DryRunFormatter;
use crate::core::config::Config;
use crate::core::schema::Schema;
use crate::services::destructive_change_detector::DestructiveChangeDetector;
use crate::services::migration_generator::MigrationGeneratorService;
use crate::services::schema_checksum::SchemaChecksumService;
use crate::services::schema_diff_detector::SchemaDiffDetectorService;
use crate::services::schema_io::schema_parser::SchemaParserService;
use crate::services::schema_io::schema_serializer::SchemaSerializerService;
use crate::services::schema_validator::SchemaValidatorService;
use crate::services::traits::{MigrationGenerator, SchemaDiffDetector, SchemaValidator};
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// generateコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct GenerateCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// カスタム設定ファイルパス
    pub config_path: Option<PathBuf>,
    /// マイグレーションの説明（オプション）
    pub description: Option<String>,
    /// ドライラン（SQLを表示するがファイルは作成しない）
    pub dry_run: bool,
    /// 破壊的変更を許可
    pub allow_destructive: bool,
}

/// 差分検出・バリデーション結果
struct DiffValidationResult {
    diff: crate::core::schema_diff::SchemaDiff,
    diff_warnings: Vec<crate::core::error::ValidationWarning>,
    destructive_report: crate::core::destructive_change_report::DestructiveChangeReport,
    rename_validation: crate::core::error::ValidationResult,
    renamed_from_warnings: Vec<crate::core::error::ValidationWarning>,
    migration_name: String,
    timestamp: String,
    sanitized_description: String,
}

/// SQL生成結果
struct GeneratedSql {
    up_sql: String,
    down_sql: String,
    validation_result: crate::core::error::ValidationResult,
}

/// サービスプロバイダー
///
/// GenerateCommandHandler が使用するサービスをまとめて保持します。
/// テスト時にモックサービスを注入可能にするために使用します。
pub struct ServiceProvider {
    pub diff_detector: Box<dyn SchemaDiffDetector>,
    pub validator: Box<dyn SchemaValidator>,
    pub generator: Box<dyn MigrationGenerator>,
}

impl ServiceProvider {
    /// デフォルトの実体サービスを使用するプロバイダーを作成
    pub fn default_services() -> Self {
        Self {
            diff_detector: Box::new(SchemaDiffDetectorService::new()),
            validator: Box::new(SchemaValidatorService::new()),
            generator: Box::new(MigrationGeneratorService::new()),
        }
    }
}

/// generateコマンドハンドラー
pub struct GenerateCommandHandler {
    services: ServiceProvider,
}

impl std::fmt::Debug for GenerateCommandHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenerateCommandHandler").finish()
    }
}

impl GenerateCommandHandler {
    /// 新しいGenerateCommandHandlerを作成
    pub fn new() -> Self {
        Self {
            services: ServiceProvider::default_services(),
        }
    }

    /// カスタムサービスプロバイダーを注入してハンドラーを作成
    pub fn with_services(services: ServiceProvider) -> Self {
        Self { services }
    }

    /// generateコマンドを実行
    ///
    /// # Arguments
    ///
    /// * `command` - generateコマンドのパラメータ
    ///
    /// # Returns
    ///
    /// 成功時は生成されたマイグレーションディレクトリのパス、失敗時はエラーメッセージ
    pub fn execute(&self, command: &GenerateCommand) -> Result<String> {
        let context =
            CommandContext::load_with_config(command.project_path.clone(), command.config_path.clone())?;
        let config = &context.config;

        // スキーマの読み込み
        let (current_schema, previous_schema) =
            self.load_schemas(&context, &command.project_path, config)?;

        // 差分検出・バリデーション
        let dvr = match self.detect_and_validate_diff(command, &current_schema, &previous_schema)? {
            Some(dvr) => dvr,
            None => return Ok("No schema changes found. Schema is up to date.".to_string()),
        };

        // SQL生成
        let generated =
            self.generate_migration_sql(command, config, &dvr, &current_schema, &previous_schema)?;

        // dry-runモードの場合はSQLを表示して終了
        if command.dry_run {
            return self.execute_dry_run(
                &dvr.migration_name,
                &generated.up_sql,
                &generated.down_sql,
                &dvr.diff,
                &generated.validation_result,
                &dvr.destructive_report,
            );
        }

        // ファイル書き出し
        let migration_name = self.write_migration_files(
            &context,
            config,
            &dvr,
            &generated,
            &current_schema,
            command,
        )?;

        let destructive_warning =
            if dvr.destructive_report.has_destructive_changes() && command.allow_destructive {
                Some(DestructiveChangeFormatter::new().format_warning(&dvr.destructive_report))
            } else {
                None
            };

        if let Some(warning) = destructive_warning {
            Ok(format!("{}\n{}", warning, migration_name))
        } else {
            Ok(migration_name)
        }
    }

    /// スキーマの読み込み
    fn load_schemas(
        &self,
        context: &CommandContext,
        project_path: &Path,
        config: &Config,
    ) -> Result<(Schema, Schema)> {
        let schema_dir = context.require_schema_dir()?;
        let parser = SchemaParserService::new();
        let current_schema = parser
            .parse_schema_directory(&schema_dir)
            .with_context(|| "Failed to read schema")?;
        let previous_schema = self.load_previous_schema(project_path, config)?;
        Ok((current_schema, previous_schema))
    }

    /// 差分検出・バリデーション
    ///
    /// 差分がない場合は `Ok(None)` を返す
    fn detect_and_validate_diff(
        &self,
        command: &GenerateCommand,
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
            return Err(anyhow::anyhow!(
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

    /// SQL生成と警告統合
    fn generate_migration_sql(
        &self,
        command: &GenerateCommand,
        config: &Config,
        dvr: &DiffValidationResult,
        current_schema: &Schema,
        previous_schema: &Schema,
    ) -> Result<GeneratedSql> {
        let allow_destructive_for_sql = command.allow_destructive || command.dry_run;

        let sql_result = self
            .services
            .generator
            .generate_up_sql_with_schemas(
                &dvr.diff,
                previous_schema,
                current_schema,
                config.dialect,
                allow_destructive_for_sql,
            );

        // 型変更検証エラーの処理
        if let Err(e) = &sql_result {
            if command.dry_run {
                return Err(self
                    .execute_dry_run_with_error(&dvr.migration_name, e, &dvr.diff)
                    .unwrap_err());
            }
            return Err(anyhow::anyhow!("{}", e));
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
            .map_err(|e| anyhow::anyhow!("Failed to generate DOWN SQL: {}", e))?;

        Ok(GeneratedSql {
            up_sql,
            down_sql,
            validation_result,
        })
    }

    /// マイグレーションファイルの書き出し
    fn write_migration_files(
        &self,
        context: &CommandContext,
        config: &Config,
        dvr: &DiffValidationResult,
        generated: &GeneratedSql,
        current_schema: &Schema,
        command: &GenerateCommand,
    ) -> Result<String> {
        let migrations_dir = context.migrations_dir();
        let migration_dir = migrations_dir.join(&dvr.migration_name);
        fs::create_dir_all(&migration_dir).with_context(|| {
            format!("Failed to create migration directory: {:?}", migration_dir)
        })?;

        // UP SQL
        let up_sql_path = migration_dir.join("up.sql");
        fs::write(&up_sql_path, &generated.up_sql)
            .with_context(|| format!("Failed to write up.sql: {:?}", up_sql_path))?;

        // DOWN SQL
        let down_sql_path = migration_dir.join("down.sql");
        fs::write(&down_sql_path, &generated.down_sql)
            .with_context(|| format!("Failed to write down.sql: {:?}", down_sql_path))?;

        // チェックサム・メタデータ
        let checksum_calculator = SchemaChecksumService::new();
        let checksum = checksum_calculator.calculate_checksum(current_schema);

        let metadata = self
            .services
            .generator
            .generate_migration_metadata(
                &dvr.timestamp,
                &dvr.sanitized_description,
                config.dialect,
                &checksum,
                dvr.destructive_report.clone(),
            )
            .map_err(|e| anyhow::anyhow!(e))?;
        let meta_path = migration_dir.join(".meta.yaml");
        fs::write(&meta_path, metadata)
            .with_context(|| format!("Failed to write metadata: {:?}", meta_path))?;

        // スキーマスナップショット保存
        self.save_current_schema(&command.project_path, config, current_schema)?;

        Ok(dvr.migration_name.clone())
    }

    /// dry-runモードの実行
    fn execute_dry_run(
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
    fn execute_dry_run_with_error(
        &self,
        migration_name: &str,
        error: &str,
        diff: &crate::core::schema_diff::SchemaDiff,
    ) -> Result<String> {
        Err(anyhow::anyhow!(
            "{}",
            DryRunFormatter::format_error(migration_name, error, diff)
        ))
    }

    /// renamed_from属性削除推奨警告を生成
    fn generate_renamed_from_remove_warnings(
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

    fn generate_enum_recreate_deprecation_warning(
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

    /// 前回のスキーマ状態を読み込む
    fn load_previous_schema(&self, project_path: &Path, config: &Config) -> Result<Schema> {
        let snapshot_path = project_path
            .join(&config.migrations_dir)
            .join(".schema_snapshot.yaml");

        if !snapshot_path.exists() {
            // 初回の場合は空のスキーマを返す
            return Ok(Schema::new("1.0".to_string()));
        }

        // SchemaParserServiceを使って新構文形式のスナップショットを読み込む
        let parser = SchemaParserService::new();
        parser
            .parse_schema_file(&snapshot_path)
            .with_context(|| "Failed to parse schema snapshot")
    }

    /// 現在のスキーマを保存（新構文形式を使用）
    fn save_current_schema(
        &self,
        project_path: &Path,
        config: &Config,
        schema: &Schema,
    ) -> Result<()> {
        let snapshot_path = project_path
            .join(&config.migrations_dir)
            .join(".schema_snapshot.yaml");

        // SchemaSerializerServiceを使用して新構文形式でシリアライズ
        let serializer = SchemaSerializerService::new();
        let yaml = serializer
            .serialize_to_string(schema)
            .with_context(|| "Failed to serialize schema")?;

        fs::write(&snapshot_path, yaml)
            .with_context(|| format!("Failed to write schema snapshot: {:?}", snapshot_path))?;

        Ok(())
    }

    /// 差分から自動的にdescriptionを生成
    fn generate_auto_description(&self, diff: &crate::core::schema_diff::SchemaDiff) -> String {
        let mut parts = Vec::new();

        if !diff.added_tables.is_empty() {
            let table_names: Vec<&str> =
                diff.added_tables.iter().map(|t| t.name.as_str()).collect();
            parts.push(format!("add tables {}", table_names.join(", ")));
        }

        if !diff.removed_tables.is_empty() {
            let removed_names: Vec<&str> = diff.removed_tables.iter().map(|s| s.as_str()).collect();
            parts.push(format!("remove tables {}", removed_names.join(", ")));
        }

        if !diff.modified_tables.is_empty() {
            let table_names: Vec<&str> = diff
                .modified_tables
                .iter()
                .map(|t| t.table_name.as_str())
                .collect();
            parts.push(format!("modify tables {}", table_names.join(", ")));
        }

        if parts.is_empty() {
            "schema changes".to_string()
        } else {
            parts.join(" and ")
        }
    }
}

impl Default for GenerateCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            description: Some("test".to_string()),
            dry_run: true,
            allow_destructive: false,
        };
        assert!(command.dry_run);
    }

    #[test]
    fn test_collect_type_changes() {
        use crate::cli::commands::dry_run_formatter::DryRunFormatter;
        use crate::core::schema::{Column, ColumnType};
        use crate::core::schema_diff::{ColumnDiff, SchemaDiff, TableDiff};

        let mut diff = SchemaDiff::new();
        let old_column = Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let new_column = Column::new("age".to_string(), ColumnType::VARCHAR { length: 50 }, false);
        let column_diff = ColumnDiff::new("age".to_string(), old_column, new_column);

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_columns.push(column_diff);
        diff.modified_tables.push(table_diff);

        let changes = DryRunFormatter::collect_type_changes(&diff);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].table, "users");
        assert_eq!(changes[0].column, "age");
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

        let error_message = "Type change validation failed:\nType conversion error: JSONB → INTEGER is not supported";

        let result =
            handler.execute_dry_run_with_error("20260124120000_test", error_message, &diff);

        // エラーとして返されることを確認
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Type Changes") || err.contains("Errors"));
    }

    // ==========================================
    // Task 6.1: 警告統合とエラー表示のテスト
    // ==========================================

    #[test]
    fn test_collect_rename_type_changes() {
        // リネームカラムからも型変更情報を収集できることを確認
        use crate::cli::commands::dry_run_formatter::DryRunFormatter;
        use crate::core::schema::{Column, ColumnType};
        use crate::core::schema_diff::{ColumnChange, RenamedColumn, SchemaDiff, TableDiff};

        let mut diff = SchemaDiff::new();
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 50 },
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
            changes: vec![ColumnChange::TypeChanged {
                old_type: "VARCHAR(50)".to_string(),
                new_type: "VARCHAR(100)".to_string(),
            }],
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(renamed);
        diff.modified_tables.push(table_diff);

        let changes = DryRunFormatter::collect_type_changes(&diff);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].table, "users");
        assert_eq!(changes[0].column, "user_name");
        assert_eq!(changes[0].old_type, "VARCHAR(50)");
        assert_eq!(changes[0].new_type, "VARCHAR(100)");
    }

    #[test]
    fn test_collect_rename_changes_only() {
        // リネームのみ（型変更なし）の場合はTypeChangesに含まれない
        use crate::cli::commands::dry_run_formatter::DryRunFormatter;
        use crate::core::schema::{Column, ColumnType};
        use crate::core::schema_diff::{RenamedColumn, SchemaDiff, TableDiff};

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
            changes: vec![], // 型変更なし
        };

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.renamed_columns.push(renamed);
        diff.modified_tables.push(table_diff);

        let changes = DryRunFormatter::collect_type_changes(&diff);
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_collect_rename_info() {
        // リネーム情報を収集できることを確認
        use crate::cli::commands::dry_run_formatter::DryRunFormatter;
        use crate::core::schema::{Column, ColumnType};
        use crate::core::schema_diff::{RenamedColumn, SchemaDiff, TableDiff};

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

        let renames = DryRunFormatter::collect_rename_changes(&diff);
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].table, "users");
        assert_eq!(renames[0].old_name, "name");
        assert_eq!(renames[0].new_name, "user_name");
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
}
