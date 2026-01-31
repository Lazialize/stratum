// initコマンドハンドラー
//
// プロジェクトの初期化処理を実装します。
// - ディレクトリ構造の作成（schema/, migrations/）
// - デフォルト設定ファイルの生成（.strata.yaml）
// - 初期化済みプロジェクトの検出と警告

use crate::cli::commands::{render_output, CommandOutput};
use crate::cli::OutputFormat;
use crate::core::config::{Config, DatabaseConfig, Dialect};
use crate::services::config_serializer::ConfigSerializer;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

/// initコマンドの出力構造体
#[derive(Debug, Clone, Serialize)]
pub struct InitOutput {
    /// メッセージ
    pub message: String,
    /// 作成されたディレクトリ
    pub created_dirs: Vec<String>,
    /// 作成された設定ファイル
    pub config_file: String,
    /// 使用されたDialect
    pub dialect: String,
}

impl CommandOutput for InitOutput {
    fn to_text(&self) -> String {
        self.message.clone()
    }
}

/// 設定ファイル生成のパラメータ
#[derive(Debug, Clone)]
pub struct ConfigFileParams {
    pub dialect: Dialect,
    pub database_name: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub password: Option<String>,
}

/// initコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct InitCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// データベース方言
    pub dialect: Dialect,
    /// 強制的に初期化（既存の設定を上書き）
    pub force: bool,
    /// データベース名
    pub database_name: String,
    /// ホスト名
    pub host: Option<String>,
    /// ポート番号
    pub port: Option<u16>,
    /// ユーザー名
    pub user: Option<String>,
    /// パスワード
    pub password: Option<String>,
    /// .gitignoreに自動追記
    pub add_gitignore: bool,
    /// 出力フォーマット
    pub format: OutputFormat,
}

/// initコマンドハンドラー
#[derive(Debug, Default)]
pub struct InitCommandHandler {}

impl InitCommandHandler {
    /// 新しいInitCommandHandlerを作成
    pub fn new() -> Self {
        Self {}
    }

    /// initコマンドを実行
    ///
    /// # Arguments
    ///
    /// * `command` - initコマンドのパラメータ
    ///
    /// # Returns
    ///
    /// 成功時は出力文字列、失敗時はエラーメッセージ
    pub fn execute(&self, command: &InitCommand) -> Result<String> {
        debug!(project_path = %command.project_path.display(), dialect = ?command.dialect, force = command.force, "Initializing project");
        // 初期化済みチェック
        if self.is_already_initialized(&command.project_path) && !command.force {
            return Err(anyhow!(
                "Project is already initialized. Use --force option to force re-initialization."
            ));
        }

        // ディレクトリ構造を作成
        self.create_directory_structure(&command.project_path)?;

        // 設定ファイルを生成
        let config_params = ConfigFileParams {
            dialect: command.dialect,
            database_name: command.database_name.clone(),
            host: command.host.clone(),
            port: command.port,
            user: command.user.clone(),
            password: command.password.clone(),
        };
        self.generate_config_file(&command.project_path, config_params)?;

        // .gitignoreに設定ファイルを自動追記 or 警告
        if command.add_gitignore {
            self.add_to_gitignore(&command.project_path)?;
        } else {
            self.warn_gitignore(&command.project_path);
        }

        let output = InitOutput {
            message: "Project initialized.".to_string(),
            created_dirs: vec!["schema/".to_string(), "migrations/".to_string()],
            config_file: Config::DEFAULT_CONFIG_PATH.to_string(),
            dialect: format!("{}", command.dialect),
        };

        render_output(&output, &command.format)
    }

    /// プロジェクトが既に初期化されているかチェック
    ///
    /// # Arguments
    ///
    /// * `project_path` - プロジェクトのルートパス
    ///
    /// # Returns
    ///
    /// 初期化済みならtrue
    pub fn is_already_initialized(&self, project_path: &Path) -> bool {
        let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
        config_path.exists()
    }

    /// ディレクトリ構造を作成
    ///
    /// # Arguments
    ///
    /// * `project_path` - プロジェクトのルートパス
    pub fn create_directory_structure(&self, project_path: &Path) -> Result<()> {
        // schema/ディレクトリを作成
        let schema_dir = project_path.join("schema");
        fs::create_dir_all(&schema_dir)
            .with_context(|| format!("Failed to create schema/ directory: {:?}", schema_dir))?;

        // migrations/ディレクトリを作成
        let migrations_dir = project_path.join("migrations");
        fs::create_dir_all(&migrations_dir).with_context(|| {
            format!(
                "Failed to create migrations/ directory: {:?}",
                migrations_dir
            )
        })?;

        Ok(())
    }

    /// .gitignoreに設定ファイルが含まれているかチェックし、警告を出力
    fn warn_gitignore(&self, project_path: &Path) {
        let config_file_name = Config::DEFAULT_CONFIG_PATH;
        let gitignore_path = project_path.join(".gitignore");

        if gitignore_path.exists() {
            if let Ok(content) = fs::read_to_string(&gitignore_path) {
                if content.lines().any(|line| {
                    let trimmed = line.trim();
                    trimmed == config_file_name || trimmed == format!("/{}", config_file_name)
                }) {
                    return; // 既に含まれている
                }
            }
        }

        eprintln!(
            "Warning: '{}' is not listed in .gitignore. The config file may contain sensitive information (e.g., database passwords). Consider adding '{}' to your .gitignore file, using --add-gitignore flag, or using environment variable references (e.g., password: \"${{DB_PASSWORD}}\").",
            config_file_name, config_file_name
        );
    }

    /// .gitignoreに設定ファイルを追記
    fn add_to_gitignore(&self, project_path: &Path) -> Result<()> {
        let config_file_name = Config::DEFAULT_CONFIG_PATH;
        let gitignore_path = project_path.join(".gitignore");

        // 既に含まれているかチェック
        if gitignore_path.exists() {
            if let Ok(content) = fs::read_to_string(&gitignore_path) {
                if content.lines().any(|line| {
                    let trimmed = line.trim();
                    trimmed == config_file_name || trimmed == format!("/{}", config_file_name)
                }) {
                    return Ok(()); // 既に含まれている
                }
            }
            // 既存の .gitignore に追記
            let mut content =
                fs::read_to_string(&gitignore_path).with_context(|| "Failed to read .gitignore")?;
            if !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(config_file_name);
            content.push('\n');
            fs::write(&gitignore_path, content).with_context(|| "Failed to write .gitignore")?;
        } else {
            // 新しい .gitignore を作成
            fs::write(&gitignore_path, format!("{}\n", config_file_name))
                .with_context(|| "Failed to create .gitignore")?;
        }

        Ok(())
    }

    /// 設定ファイルを生成
    ///
    /// # Arguments
    ///
    /// * `project_path` - プロジェクトのルートパス
    /// * `params` - 設定ファイル生成のパラメータ
    pub fn generate_config_file(
        &self,
        project_path: &Path,
        params: ConfigFileParams,
    ) -> Result<()> {
        // デフォルト値を設定
        let is_sqlite = matches!(params.dialect, Dialect::SQLite);

        // SQLiteはファイルベースのためhost不要
        let host = if is_sqlite {
            params.host.unwrap_or_default()
        } else {
            params.host.unwrap_or("localhost".to_string())
        };

        // 非SQLiteの場合はデフォルトのport/user/passwordを設定
        let port = if is_sqlite {
            params.port
        } else {
            Some(
                params
                    .port
                    .unwrap_or_else(|| params.dialect.default_port().unwrap_or(0)),
            )
        };

        let user = if is_sqlite {
            params.user
        } else {
            Some(params.user.unwrap_or_else(|| "your_user".to_string()))
        };

        let password = if is_sqlite {
            params.password
        } else {
            Some(
                params
                    .password
                    .unwrap_or_else(|| "your_password".to_string()),
            )
        };

        // データベース設定を作成
        let db_config = DatabaseConfig {
            host,
            port,
            database: params.database_name.clone(),
            user,
            password,
            timeout: if is_sqlite { None } else { Some(30) },
            ssl_mode: None,
            max_connections: None,
            min_connections: None,
            idle_timeout: None,
            options: None,
        };

        // 環境設定を作成（developmentのみ）
        let mut environments = HashMap::new();
        environments.insert("development".to_string(), db_config);

        // 設定オブジェクトを作成
        let config = Config {
            version: "1.0".to_string(),
            dialect: params.dialect,
            schema_dir: PathBuf::from("schema"),
            migrations_dir: PathBuf::from("migrations"),
            environments,
        };

        // YAMLにシリアライズ
        let yaml = ConfigSerializer::to_yaml(&config)?;

        // ファイルに書き込み
        let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
        fs::write(&config_path, yaml)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_handler() {
        let handler = InitCommandHandler::new();
        assert!(format!("{:?}", handler).contains("InitCommandHandler"));
    }

    #[test]
    fn test_is_already_initialized() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let handler = InitCommandHandler::new();
        assert!(!handler.is_already_initialized(project_path));

        // 設定ファイルを作成
        fs::write(
            project_path.join(Config::DEFAULT_CONFIG_PATH),
            "version: 1.0\n",
        )
        .unwrap();

        assert!(handler.is_already_initialized(project_path));
    }

    #[test]
    fn test_create_directory_structure() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let handler = InitCommandHandler::new();
        handler.create_directory_structure(project_path).unwrap();

        assert!(project_path.join("schema").exists());
        assert!(project_path.join("migrations").exists());
    }

    #[test]
    fn test_warn_gitignore_no_gitignore_file() {
        // .gitignoreが存在しない場合 → 警告が出力される（パニックしないことを確認）
        let temp_dir = TempDir::new().unwrap();
        let handler = InitCommandHandler::new();
        handler.warn_gitignore(temp_dir.path()); // パニックしなければOK
    }

    #[test]
    fn test_warn_gitignore_not_listed() {
        // .gitignoreが存在するが .strata.yaml が含まれていない場合
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".gitignore"), "target/\n").unwrap();
        let handler = InitCommandHandler::new();
        handler.warn_gitignore(temp_dir.path()); // パニックしなければOK
    }

    #[test]
    fn test_warn_gitignore_already_listed() {
        // .gitignoreに .strata.yaml が含まれている場合 → 警告なし
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join(".gitignore"),
            format!("target/\n{}\n", Config::DEFAULT_CONFIG_PATH),
        )
        .unwrap();
        let handler = InitCommandHandler::new();
        handler.warn_gitignore(temp_dir.path()); // パニックしなければOK
    }

    #[test]
    fn test_warn_gitignore_listed_with_slash_prefix() {
        // .gitignoreに /.strata.yaml が含まれている場合 → 警告なし
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join(".gitignore"),
            format!("target/\n/{}\n", Config::DEFAULT_CONFIG_PATH),
        )
        .unwrap();
        let handler = InitCommandHandler::new();
        handler.warn_gitignore(temp_dir.path()); // パニックしなければOK
    }

    #[test]
    fn test_init_output_json_serialization() {
        let output = InitOutput {
            message: "Project initialized.".to_string(),
            created_dirs: vec!["schema/".to_string(), "migrations/".to_string()],
            config_file: ".strata.yaml".to_string(),
            dialect: "sqlite".to_string(),
        };

        let json = serde_json::to_string_pretty(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["message"], "Project initialized.");
        assert_eq!(parsed["created_dirs"][0], "schema/");
        assert_eq!(parsed["created_dirs"][1], "migrations/");
        assert_eq!(parsed["config_file"], ".strata.yaml");
        assert_eq!(parsed["dialect"], "sqlite");
    }
}
