use anyhow::{Context, Result};
use clap::Parser;
use colored::control as color_control;
use std::env;
use std::path::PathBuf;
use std::process;
use strata::cli::commands::apply::{ApplyCommand, ApplyCommandHandler};
use strata::cli::commands::export::{ExportCommand, ExportCommandHandler};
use strata::cli::commands::generate::{GenerateCommand, GenerateCommandHandler};
use strata::cli::commands::init::{InitCommand, InitCommandHandler};
use strata::cli::commands::rollback::{RollbackCommand, RollbackCommandHandler};
use strata::cli::commands::status::{StatusCommand, StatusCommandHandler};
use strata::cli::commands::validate::{ValidateCommand, ValidateCommandHandler};
use strata::cli::commands::ErrorOutput;
use strata::cli::{Cli, Commands, OutputFormat};
use strata::core::config::Dialect;
use tracing::debug;
use tracing_subscriber::EnvFilter;

fn main() {
    sqlx::any::install_default_drivers();

    // CLIをパースして実行
    let cli = Cli::parse();

    // 非同期ランタイムを作成して実行
    let runtime = tokio::runtime::Runtime::new()
        .context("Failed to create Tokio runtime")
        .unwrap_or_else(|e| {
            eprintln!("Error: {:#}", e);
            process::exit(1);
        });

    let is_json = matches!(cli.format, OutputFormat::Json);
    let result = runtime.block_on(run_command(cli));

    match result {
        Ok(output) => {
            if !output.is_empty() {
                println!("{}", output);
            }
        }
        Err(e) => {
            if is_json {
                // JSON モードではエラーも構造化JSON形式で出力
                let error_output = ErrorOutput::new(format!("{:#}", e));
                eprintln!("{}", error_output.to_json());
            } else {
                eprintln!("Error: {:#}", e);
            }
            process::exit(1);
        }
    }
}

/// コマンドを実行する
async fn run_command(cli: Cli) -> Result<String> {
    // --no-color フラグの処理
    if cli.no_color {
        color_control::set_override(false);
    }

    // --verbose フラグの処理: tracing subscriber を初期化
    // STRATA_LOG 環境変数が設定されている場合はそちらを優先する
    // 例: STRATA_LOG=info strata status
    let filter = if let Ok(env_filter) = env::var("STRATA_LOG") {
        EnvFilter::new(env_filter)
    } else if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("warn")
    };
    // try_init() を使用して二重登録時のパニックを防止
    // （テストや他のコンテキストで既にsubscriberが登録されている場合がある）
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_writer(std::io::stderr)
        .try_init();

    if cli.verbose {
        debug!("Verbose mode enabled");
    }

    // プロジェクトのルートパスを取得
    let project_path = env::current_dir()?;

    // --config フラグの処理（絶対パスに変換）
    let config_path: Option<PathBuf> = cli.config.map(|p| {
        if p.is_absolute() {
            p
        } else {
            project_path.join(p)
        }
    });

    let format = cli.format;

    debug!(project_path = %project_path.display(), "Resolved project path");
    if let Some(ref cp) = config_path {
        debug!(config_path = %cp.display(), "Using custom config path");
    }

    match cli.command {
        Commands::Init { dialect, force } => {
            debug!(dialect = ?dialect, force = force, "Executing init command");
            let dialect = parse_dialect(dialect.as_deref())?;
            let handler = InitCommandHandler::new();
            let command = InitCommand {
                project_path,
                dialect,
                force,
                database_name: format!("{}_db", dialect),
                host: None,
                port: None,
                user: None,
                password: None,
                format,
            };
            handler.execute(&command)
        }

        Commands::Generate {
            description,
            dry_run,
            allow_destructive,
        } => {
            debug!(description = ?description, dry_run = dry_run, allow_destructive = allow_destructive, "Executing generate command");
            let handler = GenerateCommandHandler::new();
            let command = GenerateCommand {
                project_path,
                config_path,
                description,
                dry_run,
                allow_destructive,
                format,
            };
            handler.execute(&command)
        }

        Commands::Apply {
            dry_run,
            env,
            timeout,
            allow_destructive,
        } => {
            debug!(env = %env, dry_run = dry_run, timeout = ?timeout, allow_destructive = allow_destructive, "Executing apply command");
            let handler = ApplyCommandHandler::new();
            let command = ApplyCommand {
                project_path,
                config_path,
                dry_run,
                env,
                timeout,
                allow_destructive,
                format,
            };
            handler.execute(&command).await
        }

        Commands::Rollback {
            steps,
            env,
            dry_run,
            allow_destructive,
        } => {
            debug!(env = %env, steps = ?steps, dry_run = dry_run, allow_destructive = allow_destructive, "Executing rollback command");
            let handler = RollbackCommandHandler::new();
            let command = RollbackCommand {
                project_path,
                config_path,
                steps,
                env,
                dry_run,
                allow_destructive,
                format,
            };
            handler.execute(&command).await
        }

        Commands::Validate { schema_dir } => {
            debug!(schema_dir = ?schema_dir, "Executing validate command");
            let handler = ValidateCommandHandler::new();
            let command = ValidateCommand {
                project_path,
                config_path,
                schema_dir,
                format,
            };
            handler.execute(&command)
        }

        Commands::Status { env } => {
            debug!(env = %env, "Executing status command");
            let handler = StatusCommandHandler::new();
            let command = StatusCommand {
                project_path,
                config_path,
                env,
                format,
            };
            handler.execute(&command).await
        }

        Commands::Export {
            output,
            env,
            force,
            split,
            tables,
            exclude_tables,
        } => {
            debug!(env = %env, output = ?output, force = force, split = split, tables = ?tables, exclude_tables = ?exclude_tables, "Executing export command");
            let handler = ExportCommandHandler::new();
            let command = ExportCommand {
                project_path,
                config_path,
                env,
                output_dir: output,
                force,
                format,
                split,
                tables,
                exclude_tables,
            };
            handler.execute(&command).await
        }
    }
}

/// Dialect文字列をDialect型に変換する
fn parse_dialect(dialect: Option<&str>) -> Result<Dialect> {
    match dialect {
        Some("postgresql") | Some("postgres") => Ok(Dialect::PostgreSQL),
        Some("mysql") => Ok(Dialect::MySQL),
        Some("sqlite") => Ok(Dialect::SQLite),
        Some(other) => Err(anyhow::anyhow!(
            "Unsupported database dialect: {}. Please specify one of: postgresql, mysql, sqlite.",
            other
        )),
        None => Err(anyhow::anyhow!(
            "Database dialect is required. Please specify one of: postgresql, mysql, sqlite.\n  Example: strata init --dialect postgresql"
        ))
    }
}
