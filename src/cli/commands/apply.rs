// applyコマンドハンドラー
//
// マイグレーションの適用機能を実装します。
// - データベース接続の確立
// - 未適用マイグレーションの検出
// - マイグレーションの順次実行（トランザクション内）
// - 実行結果の記録とチェックサムの保存
// - 実行ログの表示

use crate::adapters::database::DatabaseConnectionService;
use crate::adapters::database_migrator::DatabaseMigratorService;
use crate::cli::command_context::CommandContext;
use crate::cli::commands::split_sql_statements;
use crate::core::config::Dialect;
use crate::core::migration::{AppliedMigration, Migration};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

/// applyコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct ApplyCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// Dry run - 実行せずにSQLを表示
    pub dry_run: bool,
    /// 対象環境
    pub env: String,
    /// タイムアウト（秒）
    pub timeout: Option<u64>,
}

/// applyコマンドハンドラー
#[derive(Debug, Clone)]
pub struct ApplyCommandHandler {}

impl ApplyCommandHandler {
    /// 新しいApplyCommandHandlerを作成
    pub fn new() -> Self {
        Self {}
    }

    /// applyコマンドを実行
    ///
    /// # Arguments
    ///
    /// * `command` - applyコマンドのパラメータ
    ///
    /// # Returns
    ///
    /// 成功時は適用されたマイグレーションの概要、失敗時はエラーメッセージ
    pub async fn execute(&self, command: &ApplyCommand) -> Result<String> {
        // 設定ファイルを読み込む
        let context = CommandContext::load(command.project_path.clone())?;
        let config = &context.config;

        // マイグレーションディレクトリのパスを解決
        let migrations_dir = context.migrations_dir();
        if !migrations_dir.exists() {
            return Err(anyhow!(
                "Migrations directory not found: {:?}",
                migrations_dir
            ));
        }

        // 利用可能なマイグレーションファイルを読み込む
        let available_migrations = self.load_available_migrations(&migrations_dir)?;

        if available_migrations.is_empty() {
            return Err(anyhow!("No migration files found"));
        }

        if command.dry_run {
            // Dry runモード: データベースに接続せずに全てのマイグレーションをpendingとみなす
            let pending_migrations: Vec<_> = available_migrations.iter().collect();
            return self.execute_dry_run(&pending_migrations);
        }

        // データベース接続を確立
        let db_config = config
            .get_database_config(&command.env)
            .with_context(|| format!("Config for environment '{}' not found", command.env))?;

        let db_service = DatabaseConnectionService::new();
        let pool = db_service
            .create_pool(config.dialect, &db_config)
            .await
            .with_context(|| "Failed to connect to database")?;

        // マイグレーション履歴テーブルを作成（存在しない場合）
        let migrator = DatabaseMigratorService::new();
        migrator
            .create_migration_table(&pool, config.dialect)
            .await
            .with_context(|| "Failed to create migration history table")?;

        // 適用済みマイグレーションを取得
        let applied_migrations = migrator
            .get_migrations(&pool, config.dialect)
            .await
            .with_context(|| "Failed to get applied migration history")?;

        // 未適用のマイグレーションを特定
        let pending_migrations: Vec<_> = available_migrations
            .iter()
            .filter(|(version, _, _)| {
                !applied_migrations
                    .iter()
                    .any(|record| &record.version == version)
            })
            .collect();

        if pending_migrations.is_empty() {
            return Err(anyhow!("No pending migrations to apply"));
        }

        // マイグレーションを順次適用
        let mut applied = Vec::new();
        for (version, description, migration_dir) in pending_migrations {
            let start_time = Utc::now();

            // up.sqlを読み込み
            let up_sql_path = migration_dir.join("up.sql");
            let up_sql = fs::read_to_string(&up_sql_path)
                .with_context(|| format!("Failed to read migration file: {:?}", up_sql_path))?;

            // メタデータを読み込み
            let meta_path = migration_dir.join(".meta.yaml");
            let meta_content = fs::read_to_string(&meta_path)
                .with_context(|| format!("Failed to read metadata file: {:?}", meta_path))?;

            // メタデータをHashMapとしてパース
            use std::collections::HashMap as StdHashMap;
            let metadata: StdHashMap<String, String> = serde_saphyr::from_str(&meta_content)
                .with_context(|| "Failed to parse metadata")?;

            let checksum = metadata
                .get("checksum")
                .ok_or_else(|| anyhow!("Metadata does not contain checksum"))?
                .to_string();

            // トランザクション内でマイグレーションを実行
            let result = self
                .apply_migration_with_transaction(
                    &pool,
                    &migrator,
                    version,
                    description,
                    &up_sql,
                    &checksum,
                    config.dialect,
                )
                .await;

            if let Err(e) = result {
                return Err(anyhow!("Failed to apply migration {}: {}", version, e));
            }

            let end_time = Utc::now();
            let duration = end_time.signed_duration_since(start_time);

            applied.push(AppliedMigration::new(
                version.clone(),
                description.clone(),
                end_time,
                duration,
            ));
        }

        // 結果サマリーを生成
        Ok(self.generate_summary(&applied))
    }

    /// 利用可能なマイグレーションファイルを読み込む
    ///
    /// マイグレーションディレクトリをスキャンし、(version, description, path)のタプルを返す
    fn load_available_migrations(
        &self,
        migrations_dir: &Path,
    ) -> Result<Vec<(String, String, PathBuf)>> {
        let mut migrations = Vec::new();

        let entries = fs::read_dir(migrations_dir).with_context(|| {
            format!("Failed to read migrations directory: {:?}", migrations_dir)
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow!("Invalid directory name"))?;

                // .で始まるディレクトリはスキップ
                if dir_name.starts_with('.') {
                    continue;
                }

                // ディレクトリ名から version と description を抽出
                // 形式: {timestamp}_{description}
                let parts: Vec<&str> = dir_name.splitn(2, '_').collect();
                if parts.len() == 2 {
                    let version = parts[0].to_string();
                    let description = parts[1].to_string();
                    migrations.push((version, description, path));
                }
            }
        }

        // バージョン順にソート
        migrations.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(migrations)
    }

    /// マイグレーションをトランザクション内で適用
    #[allow(clippy::too_many_arguments)]
    async fn apply_migration_with_transaction(
        &self,
        pool: &sqlx::AnyPool,
        migrator: &DatabaseMigratorService,
        version: &str,
        description: &str,
        up_sql: &str,
        checksum: &str,
        dialect: Dialect,
    ) -> Result<()> {
        // トランザクションを開始
        let mut tx = pool
            .begin()
            .await
            .with_context(|| "Failed to start transaction")?;

        // マイグレーションSQLを文単位で実行
        for statement in split_sql_statements(up_sql) {
            sqlx::query(&statement)
                .execute(&mut *tx)
                .await
                .with_context(|| {
                    format!(
                        "Failed to execute migration SQL: {}\nSQL: {}",
                        version, statement
                    )
                })?;
        }

        // マイグレーション履歴を記録（パラメータバインディング使用）
        let migration = Migration::new(
            version.to_string(),
            description.to_string(),
            checksum.to_string(),
        );
        let (record_sql, params) = migrator.generate_record_migration_query(&migration, dialect);

        let mut query = sqlx::query(&record_sql);
        for param in &params {
            query = query.bind(param);
        }

        query.execute(&mut *tx).await.map_err(|e| {
            anyhow!(
                "Failed to record migration history: SQL={}, Error={}",
                record_sql,
                e
            )
        })?;

        // トランザクションをコミット
        tx.commit()
            .await
            .with_context(|| "Failed to commit transaction")?;

        Ok(())
    }

    /// Dry runモードの実行
    fn execute_dry_run(&self, pending_migrations: &[&(String, String, PathBuf)]) -> Result<String> {
        let mut output = String::from("=== DRY RUN MODE ===\n");
        output.push_str(&format!(
            "The following {} migration(s) will be applied:\n\n",
            pending_migrations.len()
        ));

        for (version, description, migration_dir) in pending_migrations {
            let up_sql_path = migration_dir.join("up.sql");
            let up_sql = fs::read_to_string(&up_sql_path)
                .with_context(|| format!("Failed to read migration file: {:?}", up_sql_path))?;

            output.push_str(&format!("\u{25b6} {} - {}\n", version, description));
            output.push_str("SQL:\n");
            output.push_str(&format!("{}\n\n", up_sql));
        }

        Ok(output)
    }

    /// 適用結果のサマリーを生成
    fn generate_summary(&self, applied: &[AppliedMigration]) -> String {
        let mut summary = String::from("=== Migration Apply Complete ===\n");
        summary.push_str(&format!("{} migration(s) applied:\n\n", applied.len()));

        for migration in applied {
            summary.push_str(&format!(
                "✓ {} - {} ({}ms)\n",
                migration.version,
                migration.description,
                migration.duration.num_milliseconds()
            ));
        }

        let total_duration: i64 = applied.iter().map(|m| m.duration.num_milliseconds()).sum();
        summary.push_str(&format!("\nTotal execution time: {}ms\n", total_duration));

        summary
    }
}

impl Default for ApplyCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Dialect;
    use chrono::Duration;
    use sqlx::any::install_default_drivers;
    use sqlx::any::AnyPoolOptions;
    use sqlx::Row;
    use tempfile::TempDir;

    #[test]
    fn test_new_handler() {
        let handler = ApplyCommandHandler::new();
        assert!(format!("{:?}", handler).contains("ApplyCommandHandler"));
    }

    #[test]
    fn test_generate_summary() {
        let handler = ApplyCommandHandler::new();

        let applied = vec![
            AppliedMigration::new(
                "20260121120000".to_string(),
                "create_users".to_string(),
                Utc::now(),
                Duration::milliseconds(100),
            ),
            AppliedMigration::new(
                "20260121120001".to_string(),
                "create_posts".to_string(),
                Utc::now(),
                Duration::milliseconds(200),
            ),
        ];

        let summary = handler.generate_summary(&applied);
        assert!(summary.contains("2 migration(s) applied"));
        assert!(summary.contains("20260121120000"));
        assert!(summary.contains("20260121120001"));
        assert!(summary.contains("300ms")); // 100 + 200
    }

    #[tokio::test]
    async fn test_apply_migration_failure_does_not_record() {
        install_default_drivers();
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let connection_string = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect(&connection_string)
            .await
            .unwrap();

        let migrator = DatabaseMigratorService::new();
        migrator
            .create_migration_table(&pool, Dialect::SQLite)
            .await
            .unwrap();

        let handler = ApplyCommandHandler::new();
        let result = handler
            .apply_migration_with_transaction(
                &pool,
                &migrator,
                "20260122120000",
                "invalid_sql",
                "INVALID SQL",
                "checksum",
                Dialect::SQLite,
            )
            .await;

        assert!(result.is_err());

        let row = sqlx::query("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        let count: i64 = row.get(0);
        assert_eq!(count, 0);
    }
}
