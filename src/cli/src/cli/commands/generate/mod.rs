// generateコマンドハンドラー
//
// スキーマ差分検出とマイグレーションファイル生成を実装します。
// - スキーマ定義の読み込み
// - 前回のスキーマ状態の読み込み
// - 差分検出とマイグレーションファイル生成
// - 生成されたファイルパスの表示

mod diff;
mod io;
mod output;
mod sql;
mod summary;

#[cfg(test)]
mod tests;

use crate::cli::command_context::CommandContext;
use crate::cli::commands::destructive_change_formatter::DestructiveChangeFormatter;
use crate::cli::commands::{render_output, CommandOutput};
use crate::cli::OutputFormat;
use crate::services::migration_generator::MigrationGeneratorService;
use crate::services::schema_diff_detector::SchemaDiffDetectorService;
use crate::services::schema_validator::SchemaValidatorService;
use crate::services::traits::{MigrationGenerator, SchemaDiffDetector, SchemaValidator};
use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;
use tracing::debug;

/// generateコマンドの出力構造体
#[derive(Debug, Clone, Serialize)]
pub struct GenerateOutput {
    /// Dry runモードかどうか
    pub dry_run: bool,
    /// マイグレーション名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_name: Option<String>,
    /// マイグレーションパス
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_path: Option<String>,
    /// UP SQL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up_sql: Option<String>,
    /// DOWN SQL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down_sql: Option<String>,
    /// 警告メッセージ
    pub warnings: Vec<String>,
    /// メッセージ
    #[serde(skip)]
    pub message: String,
}

impl CommandOutput for GenerateOutput {
    fn to_text(&self) -> String {
        self.message.clone()
    }
}

/// generateコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct GenerateCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// カスタム設定ファイルパス
    pub config_path: Option<PathBuf>,
    /// スキーマディレクトリのパス（指定されない場合は設定ファイルから取得）
    pub schema_dir: Option<PathBuf>,
    /// マイグレーションの説明（オプション）
    pub description: Option<String>,
    /// ドライラン（SQLを表示するがファイルは作成しない）
    pub dry_run: bool,
    /// 破壊的変更を許可
    pub allow_destructive: bool,
    /// 詳細出力モード
    pub verbose: bool,
    /// 出力フォーマット
    pub format: OutputFormat,
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
        let context = CommandContext::load_with_config(
            command.project_path.clone(),
            command.config_path.clone(),
        )?;
        let config = &context.config;

        // スキーマの読み込み
        debug!("Loading current and previous schemas");
        let (current_schema, previous_schema) = self.load_schemas(
            &context,
            &command.project_path,
            config,
            command.schema_dir.as_ref(),
        )?;
        debug!(
            current_tables = current_schema.table_count(),
            current_views = current_schema.view_count(),
            previous_tables = previous_schema.table_count(),
            previous_views = previous_schema.view_count(),
            "Schemas loaded"
        );

        // 差分検出・バリデーション
        debug!("Detecting schema differences");
        let dvr = match self.detect_and_validate_diff(command, &current_schema, &previous_schema)? {
            Some(dvr) => dvr,
            None => {
                let output = GenerateOutput {
                    dry_run: command.dry_run,
                    migration_name: None,
                    migration_path: None,
                    up_sql: None,
                    down_sql: None,
                    warnings: vec![],
                    message: "No schema changes found. Schema is up to date.".to_string(),
                };
                return render_output(&output, &command.format);
            }
        };

        // SQL生成
        let generated =
            self.generate_migration_sql(command, config, &dvr, &current_schema, &previous_schema)?;

        // dry-runモードの場合はSQLを表示して終了
        if command.dry_run {
            let text_output = self.execute_dry_run(
                &dvr.migration_name,
                &generated.up_sql,
                &generated.down_sql,
                &dvr.diff,
                &generated.validation_result,
                &dvr.destructive_report,
            )?;

            let output = GenerateOutput {
                dry_run: true,
                migration_name: Some(dvr.migration_name.clone()),
                migration_path: None,
                up_sql: Some(generated.up_sql.clone()),
                down_sql: Some(generated.down_sql.clone()),
                warnings: vec![],
                message: text_output,
            };
            return render_output(&output, &command.format);
        }

        // ファイル書き出し
        debug!(migration_name = %dvr.migration_name, "Writing migration files");
        let (migration_name, migration_dir) = self.write_migration_files(
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

        let change_summary = self.format_change_summary(&dvr.diff, command.verbose);

        let mut text_message = String::new();
        if let Some(ref warning) = destructive_warning {
            text_message.push_str(warning);
            text_message.push('\n');
        }
        text_message.push_str(&migration_name);
        if !change_summary.is_empty() {
            text_message.push_str("\n\nChanges:\n");
            text_message.push_str(&change_summary);
        }

        let output = GenerateOutput {
            dry_run: false,
            migration_name: Some(migration_name),
            migration_path: Some(migration_dir.to_string_lossy().to_string()),
            up_sql: None,
            down_sql: None,
            warnings: destructive_warning.into_iter().collect(),
            message: text_message,
        };
        render_output(&output, &command.format)
    }
}

impl Default for GenerateCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}
