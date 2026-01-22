// initコマンドハンドラー
//
// プロジェクトの初期化処理を実装します。
// - ディレクトリ構造の作成（schema/, migrations/）
// - デフォルト設定ファイルの生成（.stratum.yaml）
// - 初期化済みプロジェクトの検出と警告

use crate::core::config::{Config, DatabaseConfig, Dialect};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
#[derive(Debug, Clone)]
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
        self.generate_config_file(
            &command.project_path,
            command.dialect,
            &command.database_name,
            command.host.clone(),
            command.port,
            command.user.clone(),
            command.password.clone(),
        )?;

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

    /// 設定ファイルを生成
    ///
    /// # Arguments
    ///
    /// * `project_path` - プロジェクトのルートパス
    /// * `dialect` - データベース方言
    /// * `database_name` - データベース名
    /// * `host` - ホスト名（オプション）
    /// * `port` - ポート番号（オプション）
    /// * `user` - ユーザー名（オプション）
    /// * `password` - パスワード（オプション）
    pub fn generate_config_file(
        &self,
        project_path: &Path,
        dialect: Dialect,
        database_name: &str,
        host: Option<String>,
        port: Option<u16>,
        user: Option<String>,
        password: Option<String>,
    ) -> Result<()> {
        // デフォルト値を設定
        let host = host.unwrap_or_else(|| "localhost".to_string());
        let port = port.unwrap_or_else(|| match dialect {
            Dialect::PostgreSQL => 5432,
            Dialect::MySQL => 3306,
            Dialect::SQLite => 0,
        });

        // データベース設定を作成
        let db_config = DatabaseConfig {
            host: host.clone(),
            port,
            database: database_name.to_string(),
            user,
            password,
            timeout: Some(30),
        };

        // 環境設定を作成
        let mut environments = HashMap::new();
        environments.insert("development".to_string(), db_config);

        // 設定オブジェクトを作成
        let config = Config {
            version: "1.0".to_string(),
            dialect,
            schema_dir: PathBuf::from("schema"),
            migrations_dir: PathBuf::from("migrations"),
            environments,
        };

        // YAMLにシリアライズ
        let yaml =
            serde_saphyr::to_string(&config).with_context(|| "Failed to serialize config file")?;

        // ファイルに書き込み
        let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
        fs::write(&config_path, yaml)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

        Ok(())
    }
}

impl Default for InitCommandHandler {
    fn default() -> Self {
        Self::new()
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
        fs::write(project_path.join(".stratum.yaml"), "version: 1.0\n").unwrap();

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
}
