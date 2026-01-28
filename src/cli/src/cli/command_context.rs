// コマンド共通コンテキスト
//
// 設定ファイル読み込みやパス解決の重複をCLI層で集約する。

use crate::adapters::database::DatabaseConnectionService;
use crate::adapters::database_migrator::DatabaseMigratorService;
use crate::core::config::{Config, DatabaseConfig, Dialect};
use crate::core::migration::MigrationRecord;
use crate::services::config_loader::ConfigLoader;
use crate::services::database_config_resolver::DatabaseConfigResolver;
use anyhow::{anyhow, Context, Result};
use sqlx::AnyPool;
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
        Self::load_with_config(project_path, None)
    }

    /// カスタム設定ファイルパスを指定してコンテキストを作成
    pub fn load_with_config(
        project_path: PathBuf,
        custom_config_path: Option<PathBuf>,
    ) -> Result<Self> {
        let config_path = custom_config_path
            .unwrap_or_else(|| project_path.join(Config::DEFAULT_CONFIG_PATH));

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

    /// スキーマディレクトリを解決（カスタム指定があれば優先）
    pub fn resolve_schema_dir(&self, custom_dir: Option<&PathBuf>) -> Result<PathBuf> {
        if let Some(dir) = custom_dir {
            if !dir.exists() {
                return Err(anyhow!("Schema directory not found: {:?}", dir));
            }
            return Ok(dir.clone());
        }

        self.require_schema_dir()
    }

    /// 環境に応じたデータベース設定を取得（環境変数上書き込み）
    pub fn database_config(&self, env: &str) -> Result<DatabaseConfig> {
        let config = self
            .config
            .get_database_config(env)
            .with_context(|| format!("Config for environment '{}' not found", env))?;
        Ok(DatabaseConfigResolver::apply_env_overrides(&config))
    }

    /// データベース方言を取得
    pub fn dialect(&self) -> Dialect {
        self.config.dialect
    }

    /// 接続プールを作成
    pub async fn connect_pool(&self, env: &str) -> Result<AnyPool> {
        self.connect_pool_with_timeout(env, None).await
    }

    /// タイムアウト付きで接続プールを作成
    pub async fn connect_pool_with_timeout(
        &self,
        env: &str,
        timeout: Option<u64>,
    ) -> Result<AnyPool> {
        let mut db_config = self.database_config(env)?;
        if let Some(t) = timeout {
            db_config.timeout = Some(t);
        }
        let db_service = DatabaseConnectionService::new();
        db_service
            .create_pool(self.config.dialect, &db_config)
            .await
            .with_context(|| "Failed to connect to database")
    }

    /// DB接続を確立し、マイグレーション履歴テーブルを作成（未作成の場合）、適用済みマイグレーションを取得
    pub async fn connect_and_load_migrations(
        &self,
        env: &str,
    ) -> Result<(AnyPool, Vec<MigrationRecord>)> {
        self.connect_and_load_migrations_with_timeout(env, None)
            .await
    }

    /// タイムアウト付きでDB接続を確立し、マイグレーション履歴テーブルを作成（未作成の場合）、適用済みマイグレーションを取得
    pub async fn connect_and_load_migrations_with_timeout(
        &self,
        env: &str,
        timeout: Option<u64>,
    ) -> Result<(AnyPool, Vec<MigrationRecord>)> {
        let pool = self.connect_pool_with_timeout(env, timeout).await?;

        let migrator = DatabaseMigratorService::new();
        migrator
            .create_migration_table(&pool, self.config.dialect)
            .await
            .with_context(|| "Failed to create migration history table")?;

        let applied_migrations = migrator
            .get_migrations(&pool, self.config.dialect)
            .await
            .with_context(|| "Failed to get applied migration history")?;

        Ok((pool, applied_migrations))
    }
}
