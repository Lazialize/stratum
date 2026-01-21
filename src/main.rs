use clap::Parser;
use stratum::cli::Cli;

fn main() {
    // CLIをパースして実行
    let cli = Cli::parse();

    // TODO: コマンドハンドラーの実装（後続のタスクで実装）
    println!("Stratum - Database Schema Management CLI");
    println!("Command: {:?}", cli.command);
    println!("Verbose: {}", cli.verbose);
    if let Some(config) = cli.config {
        println!("Config: {:?}", config);
    }
}
