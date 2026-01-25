// コマンド共通コンテキスト
//
// 設定ファイル読み込みやパス解決の重複をCLI層で集約する。

use crate::core::config::Config;
use crate::services::config_loader::ConfigLoader;
use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

/// CLIコマンド共通の実行コンテキスト
#[derive(Debug, Clone)]
pub struct CommandContext {
    pub project_path: PathBuf,
    pub config_path: PathBuf,
    pub config: Config,
}

impl CommandContext {
    /// プロジェクトルートから設定を読み込んでコンテキストを作成
    pub fn load(project_path: PathBuf) -> Result<Self> {
        let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
        if !config_path.exists() {
            return Err(anyhow!(
                "Config file not found: {:?}. Please initialize the project first with the `init` command.",
                config_path
            ));
        }

        let config =
            ConfigLoader::from_file(&config_path).with_context(|| "Failed to read config file")?;

        Ok(Self {
            project_path,
            config_path,
            config,
        })
    }

    /// スキーマディレクトリの絶対パス
    pub fn schema_dir(&self) -> PathBuf {
        self.project_path.join(&self.config.schema_dir)
    }

    /// スキーマディレクトリが存在することを確認して返す
    pub fn require_schema_dir(&self) -> Result<PathBuf> {
        let path = self.schema_dir();
        if !path.exists() {
            return Err(anyhow!("Schema directory not found: {:?}", path));
        }
        Ok(path)
    }

    /// マイグレーションディレクトリの絶対パス
    pub fn migrations_dir(&self) -> PathBuf {
        self.project_path.join(&self.config.migrations_dir)
    }

    /// マイグレーションディレクトリが存在することを確認して返す
    pub fn require_migrations_dir(&self) -> Result<PathBuf> {
        let path = self.migrations_dir();
        if !path.exists() {
            return Err(anyhow!("Migrations directory not found: {:?}", path));
        }
        Ok(path)
    }
}
