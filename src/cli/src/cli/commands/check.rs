// checkコマンドハンドラー
//
// validateとgenerate --dry-runを連携させた安全な検証フローを提供します。
// - validate実行（失敗時はgenerateを実行しない）
// - validate成功時にgenerate dry-run相当の処理を実行
// - 結果の統合出力（Text/JSON）

use crate::cli::command_context::CommandContext;
use crate::cli::commands::generate::{GenerateCommand, GenerateCommandHandler};
use crate::cli::commands::validate::{
    ValidateCommand, ValidateCommandHandler, ValidationStatistics,
};
use crate::cli::commands::{render_output, CommandOutput};
use crate::cli::OutputFormat;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::PathBuf;
use tracing::debug;

/// checkコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct CheckCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// カスタム設定ファイルパス
    pub config_path: Option<PathBuf>,
    /// スキーマディレクトリのパス（指定されない場合は設定ファイルから取得）
    pub schema_dir: Option<PathBuf>,
    /// 出力フォーマット
    pub format: OutputFormat,
}

/// checkコマンドの出力構造体
#[derive(Debug, Clone, Serialize)]
pub struct CheckOutput {
    /// validate結果
    pub validate: CheckValidateResult,
    /// generate dry-run結果（validate失敗時はnull）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate: Option<CheckGenerateResult>,
    /// サマリー
    pub summary: CheckSummary,
    /// テキスト出力メッセージ
    #[serde(skip)]
    pub text_message: String,
}

/// validate結果の構造化出力
#[derive(Debug, Clone, Serialize)]
pub struct CheckValidateResult {
    /// 検証が成功したかどうか
    pub is_valid: bool,
    /// 読み込んだスキーマファイル
    pub schema_files: Vec<String>,
    /// エラー一覧
    pub errors: Vec<crate::cli::commands::validate::ValidationIssue>,
    /// 警告一覧
    pub warnings: Vec<crate::cli::commands::validate::ValidationIssue>,
    /// 統計情報
    pub statistics: ValidationStatistics,
}

/// generate dry-run結果の構造化出力
#[derive(Debug, Clone, Serialize)]
pub struct CheckGenerateResult {
    /// マイグレーション名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_name: Option<String>,
    /// UP SQL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up_sql: Option<String>,
    /// DOWN SQL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down_sql: Option<String>,
    /// 警告メッセージ
    pub warnings: Vec<String>,
    /// 変更なしかどうか
    pub no_changes: bool,
}

/// サマリー情報
#[derive(Debug, Clone, Serialize)]
pub struct CheckSummary {
    /// validate成功フラグ
    pub validate_success: bool,
    /// generate成功フラグ（validate失敗時はnull）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate_success: Option<bool>,
}

impl CommandOutput for CheckOutput {
    fn to_text(&self) -> String {
        self.text_message.clone()
    }
}

/// checkコマンドハンドラー
#[derive(Debug, Default)]
pub struct CheckCommandHandler {}

impl CheckCommandHandler {
    /// 新しいCheckCommandHandlerを作成
    pub fn new() -> Self {
        Self {}
    }

    /// checkコマンドを実行
    pub fn execute(&self, command: &CheckCommand) -> Result<String> {
        debug!(schema_dir = ?command.schema_dir, "Executing check command");

        // Phase 1: validate を実行
        let validate_handler = ValidateCommandHandler::new();
        let validate_command = ValidateCommand {
            project_path: command.project_path.clone(),
            config_path: command.config_path.clone(),
            schema_dir: command.schema_dir.clone(),
            format: OutputFormat::Text, // 内部実行はText固定（出力を自前で統合するため）
        };

        let validate_result = validate_handler.execute(&validate_command);

        // validate結果の構造化データを構築
        let validate_data = self.build_validate_result(command);

        match validate_result {
            Ok(validate_text) => {
                // validate 成功 → generate dry-run を実行
                debug!("Validation succeeded, proceeding to generate dry-run");
                let validate_data = validate_data?;
                self.execute_with_generate(command, &validate_text, validate_data)
            }
            Err(validate_err) => {
                // validate 失敗 → generate を実行しない
                debug!("Validation failed, skipping generate dry-run");
                let validate_data =
                    validate_data.unwrap_or_else(|build_err| CheckValidateResult {
                        is_valid: false,
                        schema_files: vec![],
                        errors: vec![crate::cli::commands::validate::ValidationIssue {
                            message: build_err.to_string(),
                            table: None,
                            column: None,
                            suggestion: None,
                        }],
                        warnings: vec![],
                        statistics: ValidationStatistics {
                            tables: 0,
                            columns: 0,
                            indexes: 0,
                            constraints: 0,
                            views: 0,
                        },
                    });
                self.handle_validate_failure(command, &validate_err, validate_data)
            }
        }
    }

    /// validate成功時: generate dry-runを実行し結果を統合
    fn execute_with_generate(
        &self,
        command: &CheckCommand,
        validate_text: &str,
        validate_data: CheckValidateResult,
    ) -> Result<String> {
        // generate dry-run を実行（Text出力）
        let generate_handler = GenerateCommandHandler::new();
        let generate_command = GenerateCommand {
            project_path: command.project_path.clone(),
            config_path: command.config_path.clone(),
            schema_dir: command.schema_dir.clone(),
            description: None,
            dry_run: true,
            allow_destructive: false,
            verbose: false,
            format: OutputFormat::Text,
        };

        let generate_result = generate_handler.execute(&generate_command);

        // JSON出力用にgenerate結果をJSON形式でも取得
        let generate_json_data = self.build_generate_result(command);

        match generate_result {
            Ok(generate_text) => {
                let generate_data = generate_json_data.unwrap_or_else(|_| CheckGenerateResult {
                    migration_name: None,
                    up_sql: None,
                    down_sql: None,
                    warnings: vec![],
                    no_changes: generate_text.contains("No schema changes found"),
                });

                let text_message =
                    self.format_text_output(validate_text, Some(&generate_text), true, true);

                let output = CheckOutput {
                    validate: validate_data,
                    generate: Some(generate_data),
                    summary: CheckSummary {
                        validate_success: true,
                        generate_success: Some(true),
                    },
                    text_message,
                };

                render_output(&output, &command.format)
            }
            Err(generate_err) => {
                let text_message = self.format_text_output(
                    validate_text,
                    Some(&format!("Error: {:#}", generate_err)),
                    true,
                    false,
                );

                let output = CheckOutput {
                    validate: validate_data,
                    generate: None,
                    summary: CheckSummary {
                        validate_success: true,
                        generate_success: Some(false),
                    },
                    text_message: text_message.clone(),
                };

                match &command.format {
                    OutputFormat::Json => {
                        let json_output = render_output(&output, &command.format)?;
                        println!("{}", json_output);
                        Err(anyhow!("Generate dry-run failed: {:#}", generate_err))
                    }
                    OutputFormat::Text => {
                        eprintln!("{}", text_message);
                        Err(anyhow!("Generate dry-run failed: {:#}", generate_err))
                    }
                }
            }
        }
    }

    /// validate失敗時の出力を構築
    fn handle_validate_failure(
        &self,
        command: &CheckCommand,
        validate_err: &anyhow::Error,
        validate_data: CheckValidateResult,
    ) -> Result<String> {
        let text_message = self.format_text_output(
            &format!("Validation failed: {:#}", validate_err),
            None,
            false,
            false,
        );

        let output = CheckOutput {
            validate: validate_data,
            generate: None,
            summary: CheckSummary {
                validate_success: false,
                generate_success: None,
            },
            text_message: text_message.clone(),
        };

        match &command.format {
            OutputFormat::Json => {
                let json_output = render_output(&output, &command.format)?;
                println!("{}", json_output);
                Err(anyhow!("Validation failed: {:#}", validate_err))
            }
            OutputFormat::Text => {
                eprintln!("{}", text_message);
                Err(anyhow!("Validation failed: {:#}", validate_err))
            }
        }
    }

    /// validate結果を構造化データとして取得
    fn build_validate_result(&self, command: &CheckCommand) -> Result<CheckValidateResult> {
        let context = CommandContext::load_with_config(
            command.project_path.clone(),
            command.config_path.clone(),
        )?;
        let config = &context.config;
        let schema_dir = context.resolve_schema_dir(command.schema_dir.as_ref())?;

        let parser = crate::services::schema_io::schema_parser::SchemaParserService::new();
        let (schema, schema_files) = parser.parse_schema_directory_with_files(&schema_dir)?;

        let validator = crate::services::schema_validator::SchemaValidatorService::new();
        let validation_result = validator.validate_with_dialect(&schema, config.dialect);

        let file_names: Vec<String> = schema_files
            .iter()
            .map(|f| {
                f.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| f.display().to_string())
            })
            .collect();

        let validate_handler = ValidateCommandHandler::new();

        let errors: Vec<crate::cli::commands::validate::ValidationIssue> = validation_result
            .errors
            .iter()
            .map(|error| {
                let location = validate_handler.get_error_location(error);
                crate::cli::commands::validate::ValidationIssue {
                    message: format!("{}", error),
                    table: location.and_then(|l| l.table.clone()),
                    column: location.and_then(|l| l.column.clone()),
                    suggestion: validate_handler
                        .get_error_suggestion(error)
                        .map(|s| s.to_string()),
                }
            })
            .collect();

        let warnings: Vec<crate::cli::commands::validate::ValidationIssue> = validation_result
            .warnings
            .iter()
            .map(|warning| {
                let loc = &warning.location;
                crate::cli::commands::validate::ValidationIssue {
                    message: warning.message.clone(),
                    table: loc.as_ref().and_then(|l| l.table.clone()),
                    column: loc.as_ref().and_then(|l| l.column.clone()),
                    suggestion: None,
                }
            })
            .collect();

        let mut column_count = 0;
        let mut index_count = 0;
        let mut constraint_count = 0;
        for table in schema.tables.values() {
            column_count += table.columns.len();
            index_count += table.indexes.len();
            constraint_count += table.constraints.len();
        }

        Ok(CheckValidateResult {
            is_valid: validation_result.is_valid(),
            schema_files: file_names,
            errors,
            warnings,
            statistics: ValidationStatistics {
                tables: schema.table_count(),
                columns: column_count,
                indexes: index_count,
                constraints: constraint_count,
                views: schema.view_count(),
            },
        })
    }

    /// generate dry-run結果を構造化データとして取得
    fn build_generate_result(&self, command: &CheckCommand) -> Result<CheckGenerateResult> {
        let generate_handler = GenerateCommandHandler::new();
        let generate_command = GenerateCommand {
            project_path: command.project_path.clone(),
            config_path: command.config_path.clone(),
            schema_dir: command.schema_dir.clone(),
            description: None,
            dry_run: true,
            allow_destructive: true,
            verbose: false,
            format: OutputFormat::Json,
        };

        let json_output = generate_handler.execute(&generate_command)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_output)
            .map_err(|e| anyhow!("Failed to parse generate output: {}", e))?;

        Ok(CheckGenerateResult {
            migration_name: parsed
                .get("migration_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            up_sql: parsed
                .get("up_sql")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            down_sql: parsed
                .get("down_sql")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            warnings: parsed
                .get("warnings")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            no_changes: parsed.get("migration_name").is_none()
                || parsed.get("migration_name").unwrap().is_null(),
        })
    }

    /// テキスト出力のフォーマット
    fn format_text_output(
        &self,
        validate_text: &str,
        generate_text: Option<&str>,
        validate_success: bool,
        generate_success: bool,
    ) -> String {
        let mut output = String::new();

        // Validate セクション
        output.push_str("=== Check Results ===\n\n");
        output.push_str("--- Validate ---\n");
        output.push_str(validate_text);
        output.push('\n');

        // Generate dry-run セクション
        if let Some(gen_text) = generate_text {
            output.push_str("\n--- Generate (dry-run) ---\n");
            output.push_str(gen_text);
            output.push('\n');
        }

        // サマリー
        output.push_str("\n=== Summary ===\n");
        if validate_success {
            output.push_str("✓ Validate: passed\n");
        } else {
            output.push_str("✗ Validate: failed\n");
        }

        if validate_success {
            if generate_success {
                output.push_str("✓ Generate (dry-run): passed\n");
            } else {
                output.push_str("✗ Generate (dry-run): failed\n");
            }
        } else {
            output.push_str("- Generate (dry-run): skipped\n");
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            format: OutputFormat::Text,
        };
        assert_eq!(command.project_path, PathBuf::from("/test/path"));
        assert_eq!(command.schema_dir, None);
    }

    #[test]
    fn test_check_output_json_serialization() {
        let output = CheckOutput {
            validate: CheckValidateResult {
                is_valid: true,
                schema_files: vec!["users.yaml".to_string()],
                errors: vec![],
                warnings: vec![],
                statistics: ValidationStatistics {
                    tables: 1,
                    columns: 2,
                    indexes: 0,
                    constraints: 1,
                    views: 0,
                },
            },
            generate: Some(CheckGenerateResult {
                migration_name: Some("20260202_test".to_string()),
                up_sql: Some("CREATE TABLE test;".to_string()),
                down_sql: Some("DROP TABLE test;".to_string()),
                warnings: vec![],
                no_changes: false,
            }),
            summary: CheckSummary {
                validate_success: true,
                generate_success: Some(true),
            },
            text_message: "should not appear in JSON".to_string(),
        };

        let json = serde_json::to_string_pretty(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // text_message は #[serde(skip)] のため含まれない
        assert!(parsed.get("text_message").is_none());
        assert_eq!(parsed["validate"]["is_valid"], true);
        assert_eq!(parsed["summary"]["validate_success"], true);
        assert_eq!(parsed["summary"]["generate_success"], true);
        assert!(parsed["generate"].is_object());
    }

    #[test]
    fn test_check_output_json_validate_failure() {
        let output = CheckOutput {
            validate: CheckValidateResult {
                is_valid: false,
                schema_files: vec![],
                errors: vec![],
                warnings: vec![],
                statistics: ValidationStatistics {
                    tables: 0,
                    columns: 0,
                    indexes: 0,
                    constraints: 0,
                    views: 0,
                },
            },
            generate: None,
            summary: CheckSummary {
                validate_success: false,
                generate_success: None,
            },
            text_message: "text".to_string(),
        };

        let json = serde_json::to_string_pretty(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["summary"]["validate_success"], false);
        assert!(parsed["summary"].get("generate_success").is_none());
        assert!(parsed.get("generate").is_none());
    }

    #[test]
    fn test_check_output_json_includes_error_location_and_suggestion() {
        use crate::cli::commands::validate::ValidationIssue;

        let output = CheckOutput {
            validate: CheckValidateResult {
                is_valid: false,
                schema_files: vec!["users.yaml".to_string()],
                errors: vec![ValidationIssue {
                    message: "No primary key defined".to_string(),
                    table: Some("users".to_string()),
                    column: None,
                    suggestion: Some("Add a primary key constraint".to_string()),
                }],
                warnings: vec![ValidationIssue {
                    message: "Wide column detected".to_string(),
                    table: Some("users".to_string()),
                    column: Some("bio".to_string()),
                    suggestion: None,
                }],
                statistics: ValidationStatistics {
                    tables: 1,
                    columns: 2,
                    indexes: 0,
                    constraints: 0,
                    views: 0,
                },
            },
            generate: None,
            summary: CheckSummary {
                validate_success: false,
                generate_success: None,
            },
            text_message: "text".to_string(),
        };

        let json = serde_json::to_string_pretty(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // エラーの location/suggestion が含まれる
        assert_eq!(parsed["validate"]["errors"][0]["table"], "users");
        assert!(parsed["validate"]["errors"][0].get("column").is_none()); // None はスキップ
        assert_eq!(
            parsed["validate"]["errors"][0]["suggestion"],
            "Add a primary key constraint"
        );

        // 警告の location が含まれる
        assert_eq!(parsed["validate"]["warnings"][0]["table"], "users");
        assert_eq!(parsed["validate"]["warnings"][0]["column"], "bio");
        assert!(parsed["validate"]["warnings"][0]
            .get("suggestion")
            .is_none());
    }

    #[test]
    fn test_format_text_output_all_success() {
        let handler = CheckCommandHandler::new();
        let text = handler.format_text_output("validate ok", Some("generate ok"), true, true);
        assert!(text.contains("Check Results"));
        assert!(text.contains("Validate"));
        assert!(text.contains("Generate (dry-run)"));
        assert!(text.contains("✓ Validate: passed"));
        assert!(text.contains("✓ Generate (dry-run): passed"));
    }

    #[test]
    fn test_format_text_output_validate_failure() {
        let handler = CheckCommandHandler::new();
        let text = handler.format_text_output("validate failed", None, false, false);
        assert!(text.contains("✗ Validate: failed"));
        assert!(text.contains("Generate (dry-run): skipped"));
        assert!(!text.contains("--- Generate"));
    }

    #[test]
    fn test_format_text_output_generate_failure() {
        let handler = CheckCommandHandler::new();
        let text = handler.format_text_output("validate ok", Some("generate error"), true, false);
        assert!(text.contains("✓ Validate: passed"));
        assert!(text.contains("✗ Generate (dry-run): failed"));
    }
}
