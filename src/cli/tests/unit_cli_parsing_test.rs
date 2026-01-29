/// CLI エントリーポイントのテスト
///
/// このテストは、CLIの構造が正しく定義され、すべてのサブコマンドとオプションが
/// 期待通りに動作することを確認します。
use clap::Parser;

#[cfg(test)]
mod cli_tests {
    use super::*;

    /// CLIメイン構造体がパース可能であることを確認
    #[test]
    fn test_cli_can_parse() {
        // CLIのメイン構造体をインポート
        use strata::cli::Cli;

        // ヘルプフラグでパース可能であることを確認
        let result = Cli::try_parse_from(["strata", "--help"]);
        // ヘルプは成功ではなくエラーを返すが、それは正常な動作
        assert!(result.is_err());

        // バージョンフラグでパース可能であることを確認
        let result = Cli::try_parse_from(["strata", "--version"]);
        assert!(result.is_err());
    }

    /// initサブコマンドがパース可能であることを確認
    #[test]
    fn test_init_command_parses() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "init"]).unwrap();
        assert!(matches!(cli.command, strata::cli::Commands::Init { .. }));
    }

    /// generateサブコマンドがパース可能であることを確認
    #[test]
    fn test_generate_command_parses() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "generate"]).unwrap();
        assert!(matches!(
            cli.command,
            strata::cli::Commands::Generate { .. }
        ));
    }

    /// applyサブコマンドがパース可能であることを確認
    #[test]
    fn test_apply_command_parses() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "apply"]).unwrap();
        assert!(matches!(cli.command, strata::cli::Commands::Apply { .. }));
    }

    /// rollbackサブコマンドがパース可能であることを確認
    #[test]
    fn test_rollback_command_parses() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "rollback"]).unwrap();
        assert!(matches!(
            cli.command,
            strata::cli::Commands::Rollback { .. }
        ));
    }

    /// validateサブコマンドがパース可能であることを確認
    #[test]
    fn test_validate_command_parses() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "validate"]).unwrap();
        assert!(matches!(
            cli.command,
            strata::cli::Commands::Validate { .. }
        ));
    }

    /// statusサブコマンドがパース可能であることを確認
    #[test]
    fn test_status_command_parses() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "status"]).unwrap();
        assert!(matches!(cli.command, strata::cli::Commands::Status { .. }));
    }

    /// exportサブコマンドがパース可能であることを確認
    #[test]
    fn test_export_command_parses() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "export"]).unwrap();
        assert!(matches!(cli.command, strata::cli::Commands::Export { .. }));
    }

    /// グローバルオプション --config がパース可能であることを確認
    #[test]
    fn test_global_config_option() {
        use std::path::Path;
        use strata::cli::Cli;

        let cli =
            Cli::try_parse_from(["strata", "--config", "/path/to/config.yaml", "status"]).unwrap();

        assert_eq!(
            cli.config.as_deref(),
            Some(Path::new("/path/to/config.yaml"))
        );
    }

    /// グローバルオプション --verbose がパース可能であることを確認
    #[test]
    fn test_global_verbose_option() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "--verbose", "status"]).unwrap();

        assert!(cli.verbose);
    }

    /// グローバルオプション --no-color がパース可能であることを確認
    #[test]
    fn test_global_no_color_option() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "--no-color", "status"]).unwrap();

        assert!(cli.no_color);
    }

    /// apply コマンドの --dry-run オプションがパース可能であることを確認
    #[test]
    fn test_apply_dry_run_option() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "apply", "--dry-run"]).unwrap();

        match cli.command {
            strata::cli::Commands::Apply { dry_run, .. } => {
                assert!(dry_run);
            }
            _ => panic!("Expected Apply command"),
        }
    }

    /// generate コマンドの --allow-destructive オプションがパース可能であることを確認
    #[test]
    fn test_generate_allow_destructive_option() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "generate", "--allow-destructive"]).unwrap();

        match cli.command {
            strata::cli::Commands::Generate {
                allow_destructive, ..
            } => {
                assert!(allow_destructive);
            }
            _ => panic!("Expected Generate command"),
        }
    }

    /// apply コマンドの --allow-destructive オプションがパース可能であることを確認
    #[test]
    fn test_apply_allow_destructive_option() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "apply", "--allow-destructive"]).unwrap();

        match cli.command {
            strata::cli::Commands::Apply {
                allow_destructive, ..
            } => {
                assert!(allow_destructive);
            }
            _ => panic!("Expected Apply command"),
        }
    }

    /// rollback コマンドの --steps オプションがパース可能であることを確認
    #[test]
    fn test_rollback_steps_option() {
        use strata::cli::Cli;

        let cli = Cli::try_parse_from(["strata", "rollback", "--steps", "3"]).unwrap();

        match cli.command {
            strata::cli::Commands::Rollback { steps, .. } => {
                assert_eq!(steps, Some(3));
            }
            _ => panic!("Expected Rollback command"),
        }
    }
}
