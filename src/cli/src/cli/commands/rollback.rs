// rollbackコマンドハンドラー
//
// マイグレーションのロールバック機能を実装します。
// - 最新の適用済みマイグレーションの特定
// - down.sqlの実行（トランザクション内）
// - マイグレーション履歴からの削除
// - ロールバック結果の表示

use crate::adapters::database_migrator::DatabaseMigratorService;
use crate::cli::command_context::CommandContext;
use crate::cli::commands::migration_loader;
use crate::cli::commands::split_sql_statements;
use crate::core::config::Dialect;
use crate::core::migration::AppliedMigration;
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;

/// rollbackコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct RollbackCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// ロールバックするマイグレーションの数
    pub steps: Option<u32>,
    /// 対象環境
    pub env: String,
}

/// rollbackコマンドハンドラー
#[derive(Debug, Clone)]
pub struct RollbackCommandHandler {}

impl RollbackCommandHandler {
    /// 新しいRollbackCommandHandlerを作成
    pub fn new() -> Self {
        Self {}
    }

    /// rollbackコマンドを実行
    ///
    /// # Arguments
    ///
    /// * `command` - rollbackコマンドのパラメータ
    ///
    /// # Returns
    ///
    /// 成功時はロールバックされたマイグレーションの概要、失敗時はエラーメッセージ
    pub async fn execute(&self, command: &RollbackCommand) -> Result<String> {
        // 設定ファイルを読み込む
        let context = CommandContext::load(command.project_path.clone())?;
        let config = &context.config;

        // マイグレーションディレクトリのパスを解決
        let migrations_dir = context.require_migrations_dir()?;

        // 利用可能なマイグレーションファイルを読み込む
        let available_migrations = migration_loader::load_available_migrations(&migrations_dir)?;

        if available_migrations.is_empty() {
            return Err(anyhow!("No migration files found"));
        }

        // データベース接続を確立
        let pool = context.connect_pool(&command.env).await?;

        // マイグレーション履歴テーブルが存在するか確認
        let migrator = DatabaseMigratorService::new();
        let table_exists = migrator
            .migration_table_exists(&pool, config.dialect)
            .await
            .with_context(|| "Failed to check migration table existence")?;

        if !table_exists {
            return Err(anyhow!(
                "Migration history table does not exist. Please apply migrations first with the `apply` command."
            ));
        }

        // 適用済みマイグレーションを取得
        let applied_migrations = migrator
            .get_migrations(&pool, config.dialect)
            .await
            .with_context(|| "Failed to get applied migration history")?;

        if applied_migrations.is_empty() {
            return Err(anyhow!("No migrations to rollback"));
        }

        // ロールバックする件数を決定（デフォルトは1）
        let steps = command.steps.unwrap_or(1) as usize;

        // ロールバックするマイグレーションを選択（最新のものから）
        let to_rollback_count = steps.min(applied_migrations.len());
        let to_rollback: Vec<_> = applied_migrations
            .iter()
            .rev()
            .take(to_rollback_count)
            .collect();

        // マイグレーションを順次ロールバック
        let mut rolled_back = Vec::new();
        for record in to_rollback {
            let start_time = Utc::now();

            // マイグレーションディレクトリを検索
            let migration_info = available_migrations
                .iter()
                .find(|(v, _, _)| v == &record.version)
                .ok_or_else(|| anyhow!("Migration file not found: {}", record.version))?;

            let migration_dir = &migration_info.2;

            // down.sqlを読み込み
            let down_sql_path = migration_dir.join("down.sql");
            let down_sql = fs::read_to_string(&down_sql_path)
                .with_context(|| format!("Failed to read migration file: {:?}", down_sql_path))?;

            // トランザクション内でロールバックを実行
            let result = self
                .rollback_migration_with_transaction(
                    &pool,
                    &migrator,
                    &record.version,
                    &down_sql,
                    config.dialect,
                )
                .await;

            if let Err(e) = result {
                return Err(anyhow!(
                    "Failed to rollback migration {}: {}",
                    record.version,
                    e
                ));
            }

            let end_time = Utc::now();
            let duration = end_time.signed_duration_since(start_time);

            rolled_back.push(AppliedMigration::new(
                record.version.clone(),
                record.description.clone(),
                end_time,
                duration,
            ));
        }

        // 結果サマリーを生成
        Ok(self.generate_summary(&rolled_back))
    }

    /// マイグレーションをトランザクション内でロールバック
    async fn rollback_migration_with_transaction(
        &self,
        pool: &sqlx::AnyPool,
        migrator: &DatabaseMigratorService,
        version: &str,
        down_sql: &str,
        dialect: Dialect,
    ) -> Result<()> {
        // トランザクションを開始
        let mut tx = pool
            .begin()
            .await
            .with_context(|| "Failed to start transaction")?;

        // マイグレーションdown SQLを文単位で実行
        for statement in split_sql_statements(down_sql) {
            sqlx::query(&statement)
                .execute(&mut *tx)
                .await
                .with_context(|| {
                    format!(
                        "Failed to execute migration down SQL: {}\nSQL: {}",
                        version, statement
                    )
                })?;
        }

        // マイグレーション履歴から削除（パラメータバインディング使用）
        let (remove_sql, params) = migrator.generate_remove_migration_query(version, dialect);

        let mut query = sqlx::query(&remove_sql);
        for param in &params {
            query = query.bind(param);
        }

        query
            .execute(&mut *tx)
            .await
            .with_context(|| "Failed to remove migration history")?;

        // トランザクションをコミット
        tx.commit()
            .await
            .with_context(|| "Failed to commit transaction")?;

        Ok(())
    }

    /// ロールバック結果のサマリーを生成
    pub fn generate_summary(&self, rolled_back: &[AppliedMigration]) -> String {
        let mut summary = String::from("=== Migration Rollback Complete ===\n");
        summary.push_str(&format!(
            "{} migration(s) rolled back:\n\n",
            rolled_back.len()
        ));

        for migration in rolled_back {
            summary.push_str(&format!(
                "✓ {} - {} ({}ms)\n",
                migration.version,
                migration.description,
                migration.duration.num_milliseconds()
            ));
        }

        let total_duration: i64 = rolled_back
            .iter()
            .map(|m| m.duration.num_milliseconds())
            .sum();
        summary.push_str(&format!("\nTotal execution time: {}ms\n", total_duration));

        summary
    }
}

impl Default for RollbackCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Dialect;
    use crate::core::migration::Migration;
    use sqlx::any::install_default_drivers;
    use sqlx::any::AnyPoolOptions;
    use sqlx::Row;
    use tempfile::TempDir;

    #[test]
    fn test_new_handler() {
        let handler = RollbackCommandHandler::new();
        assert!(format!("{:?}", handler).contains("RollbackCommandHandler"));
    }

    #[test]
    fn test_generate_summary() {
        use chrono::Duration;

        let handler = RollbackCommandHandler::new();

        let rolled_back = vec![
            AppliedMigration::new(
                "20260121120001".to_string(),
                "create_posts".to_string(),
                Utc::now(),
                Duration::milliseconds(100),
            ),
            AppliedMigration::new(
                "20260121120000".to_string(),
                "create_users".to_string(),
                Utc::now(),
                Duration::milliseconds(200),
            ),
        ];

        let summary = handler.generate_summary(&rolled_back);
        assert!(summary.contains("2 migration(s) rolled back"));
        assert!(summary.contains("20260121120000"));
        assert!(summary.contains("20260121120001"));
        assert!(summary.contains("300ms")); // 100 + 200
    }

    #[tokio::test]
    async fn test_rollback_failure_keeps_record() {
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

        let migration = Migration::new(
            "20260122120001".to_string(),
            "create_users".to_string(),
            "checksum".to_string(),
        );
        let (record_sql, params) =
            migrator.generate_record_migration_query(&migration, Dialect::SQLite);
        let mut query = sqlx::query(&record_sql);
        for param in &params {
            query = query.bind(param);
        }
        query.execute(&pool).await.unwrap();

        let handler = RollbackCommandHandler::new();
        let result = handler
            .rollback_migration_with_transaction(
                &pool,
                &migrator,
                "20260122120001",
                "INVALID SQL",
                Dialect::SQLite,
            )
            .await;

        assert!(result.is_err());

        let row = sqlx::query("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        let count: i64 = row.get(0);
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_rollback_success_removes_record() {
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

        sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();

        let migration = Migration::new(
            "20260122120002".to_string(),
            "create_users".to_string(),
            "checksum".to_string(),
        );
        let (record_sql, params) =
            migrator.generate_record_migration_query(&migration, Dialect::SQLite);
        let mut query = sqlx::query(&record_sql);
        for param in &params {
            query = query.bind(param);
        }
        query.execute(&pool).await.unwrap();

        let handler = RollbackCommandHandler::new();
        handler
            .rollback_migration_with_transaction(
                &pool,
                &migrator,
                "20260122120002",
                "DROP TABLE users",
                Dialect::SQLite,
            )
            .await
            .unwrap();

        let row = sqlx::query("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        let count: i64 = row.get(0);
        assert_eq!(count, 0);
    }
}
