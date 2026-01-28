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
use strata::cli::{Cli, Commands};
use strata::core::config::Dialect;

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
    // --no-color フラグの処理
    if cli.no_color {
        color_control::set_override(false);
    }

    // --verbose フラグの処理
    // 環境変数を設定して、ハンドラーや他のコンポーネントで参照可能にする
    if cli.verbose {
        env::set_var("STRATA_VERBOSE", "1");
        eprintln!("Verbose mode enabled");
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

        Commands::Generate {
            description,
            dry_run,
            allow_destructive,
        } => {
            let handler = GenerateCommandHandler::new();
            let command = GenerateCommand {
                project_path,
                config_path,
                description,
                dry_run,
                allow_destructive,
            };
            handler.execute(&command)
        }

        Commands::Apply {
            dry_run,
            env,
            timeout,
            allow_destructive,
        } => {
            let handler = ApplyCommandHandler::new();
            let command = ApplyCommand {
                project_path,
                config_path,
                dry_run,
                env,
                timeout,
                allow_destructive,
            };
            handler.execute(&command).await
        }

        Commands::Rollback {
            steps,
            env,
            dry_run,
            allow_destructive,
        } => {
            let handler = RollbackCommandHandler::new();
            let command = RollbackCommand {
                project_path,
                config_path,
                steps,
                env,
                dry_run,
                allow_destructive,
            };
            handler.execute(&command).await
        }

        Commands::Validate { schema_dir } => {
            let handler = ValidateCommandHandler::new();
            let command = ValidateCommand {
                project_path,
                config_path,
                schema_dir,
            };
            handler.execute(&command)
        }

        Commands::Status { env } => {
            let handler = StatusCommandHandler::new();
            let command = StatusCommand {
                project_path,
                config_path,
                env,
            };
            handler.execute(&command).await
        }

        Commands::Export {
            output,
            env,
            force,
        } => {
            let handler = ExportCommandHandler::new();
            let command = ExportCommand {
                project_path,
                config_path,
                env,
                output_dir: output,
                force,
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
