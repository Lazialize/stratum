// initコマンドハンドラー
//
// プロジェクトの初期化処理を実装します。
// - ディレクトリ構造の作成（schema/, migrations/）
// - デフォルト設定ファイルの生成（.strata.yaml）
// - 初期化済みプロジェクトの検出と警告

use crate::core::config::{Config, DatabaseConfig, Dialect};
use crate::services::config_serializer::ConfigSerializer;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    /// 成功時はOk(())、失敗時はエラーメッセージ
    pub fn execute(&self, command: &InitCommand) -> Result<()> {
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

        // .gitignoreに設定ファイルが含まれていない場合は警告
        self.warn_gitignore(&command.project_path);

        Ok(())
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
            "Warning: '{}' is not listed in .gitignore. The config file may contain sensitive information (e.g., database passwords). Consider adding '{}' to your .gitignore file or using environment variable references (e.g., password: \"${{DB_PASSWORD}}\").",
            config_file_name, config_file_name
        );
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
        let host = params.host.unwrap_or("localhost".to_string());

        // データベース設定を作成
        // portがNoneの場合はDialectのデフォルトポートが使用される
        let db_config = DatabaseConfig {
            host,
            port: params.port,
            database: params.database_name,
            user: params.user,
            password: params.password,
            timeout: Some(30),
            ssl_mode: None,
            max_connections: None,
            min_connections: None,
            idle_timeout: None,
            options: None,
        };

        // 環境設定を作成
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
}
