use anyhow::Result;
use clap::Parser;
use std::env;
use std::process;
use stratum::cli::commands::apply::{ApplyCommand, ApplyCommandHandler};
use stratum::cli::commands::export::{ExportCommand, ExportCommandHandler};
use stratum::cli::commands::generate::{GenerateCommand, GenerateCommandHandler};
use stratum::cli::commands::init::{InitCommand, InitCommandHandler};
use stratum::cli::commands::rollback::{RollbackCommand, RollbackCommandHandler};
use stratum::cli::commands::status::{StatusCommand, StatusCommandHandler};
use stratum::cli::commands::validate::{ValidateCommand, ValidateCommandHandler};
use stratum::cli::{Cli, Commands};
use stratum::core::config::Dialect;

fn main() {
    // CLIをパースして実行
    let cli = Cli::parse();

    // 非同期ランタイムを作成して実行
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    let result = runtime.block_on(run_command(cli));

    match result {
        Ok(output) => {
            if !output.is_empty() {
                println!("{}", output);
            }
        }
        Err(e) => {
            eprintln!("Error: {:#}", e);
            process::exit(1);
        }
    }
}

/// コマンドを実行する
async fn run_command(cli: Cli) -> Result<String> {
    // プロジェクトのルートパスを取得
    let project_path = env::current_dir()?;

    match cli.command {
        Commands::Init { dialect, force } => {
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
            };
            handler.execute(&command)?;
            Ok("Project initialized.".to_string())
        }

        Commands::Generate { description } => {
            let handler = GenerateCommandHandler::new();
            let command = GenerateCommand {
                project_path,
                description,
            };
            handler.execute(&command)
        }

        Commands::Apply {
            dry_run,
            env,
            timeout,
        } => {
            let handler = ApplyCommandHandler::new();
            let command = ApplyCommand {
                project_path,
                dry_run,
                env,
                timeout,
            };
            handler.execute(&command).await
        }

        Commands::Rollback { steps, env } => {
            let handler = RollbackCommandHandler::new();
            let command = RollbackCommand {
                project_path,
                steps,
                env,
            };
            handler.execute(&command).await
        }

        Commands::Validate { schema_dir } => {
            let handler = ValidateCommandHandler::new();
            let command = ValidateCommand {
                project_path,
                schema_dir,
            };
            handler.execute(&command)
        }

        Commands::Status { env } => {
            let handler = StatusCommandHandler::new();
            let command = StatusCommand { project_path, env };
            handler.execute(&command).await
        }

        Commands::Export { output, env, force: _ } => {
            let handler = ExportCommandHandler::new();
            let command = ExportCommand {
                project_path,
                env,
                output_dir: output,
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
        None => Ok(Dialect::SQLite), // デフォルトはSQLite
    }
}
