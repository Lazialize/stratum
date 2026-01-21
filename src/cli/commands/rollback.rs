// rollbackコマンドハンドラー
//
// マイグレーションのロールバック機能を実装します。
// - 最新の適用済みマイグレーションの特定
// - down.sqlの実行（トランザクション内）
// - マイグレーション履歴からの削除
// - ロールバック結果の表示

use crate::adapters::database::DatabaseConnectionService;
use crate::adapters::database_migrator::DatabaseMigratorService;
use crate::core::config::Config;
use crate::core::migration::AppliedMigration;
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

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
        let config_path = command.project_path.join(Config::DEFAULT_CONFIG_PATH);
        if !config_path.exists() {
            return Err(anyhow!(
                "設定ファイルが見つかりません: {:?}。まず `init` コマンドでプロジェクトを初期化してください。",
                config_path
            ));
        }

        let config = Config::from_file(&config_path)
            .with_context(|| "設定ファイルの読み込みに失敗しました")?;

        // マイグレーションディレクトリのパスを解決
        let migrations_dir = command.project_path.join(&config.migrations_dir);
        if !migrations_dir.exists() {
            return Err(anyhow!(
                "マイグレーションディレクトリが見つかりません: {:?}",
                migrations_dir
            ));
        }

        // 利用可能なマイグレーションファイルを読み込む
        let available_migrations = self.load_available_migrations(&migrations_dir)?;

        if available_migrations.is_empty() {
            return Err(anyhow!("マイグレーションファイルが見つかりません"));
        }

        // データベース接続を確立
        let db_config = config
            .get_database_config(&command.env)
            .with_context(|| format!("環境 '{}' の設定が見つかりません", command.env))?;

        let db_service = DatabaseConnectionService::new();
        let pool = db_service
            .create_pool(config.dialect, &db_config)
            .await
            .with_context(|| "データベース接続に失敗しました")?;

        // マイグレーション履歴テーブルが存在するか確認
        let migrator = DatabaseMigratorService::new();
        let table_exists = migrator
            .migration_table_exists(&pool, config.dialect)
            .await
            .with_context(|| "マイグレーションテーブルの存在確認に失敗しました")?;

        if !table_exists {
            return Err(anyhow!(
                "マイグレーション履歴テーブルが存在しません。まず `apply` コマンドでマイグレーションを適用してください。"
            ));
        }

        // 適用済みマイグレーションを取得
        let applied_migrations = migrator
            .get_migrations(&pool)
            .await
            .with_context(|| "適用済みマイグレーション履歴の取得に失敗しました")?;

        if applied_migrations.is_empty() {
            return Err(anyhow!("ロールバック可能なマイグレーションがありません"));
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
                .ok_or_else(|| {
                    anyhow!(
                        "マイグレーションファイルが見つかりません: {}",
                        record.version
                    )
                })?;

            let migration_dir = &migration_info.2;

            // down.sqlを読み込み
            let down_sql_path = migration_dir.join("down.sql");
            let down_sql = fs::read_to_string(&down_sql_path).with_context(|| {
                format!(
                    "マイグレーションファイルの読み込みに失敗しました: {:?}",
                    down_sql_path
                )
            })?;

            // トランザクション内でロールバックを実行
            let result = self
                .rollback_migration_with_transaction(
                    &pool,
                    &migrator,
                    &record.version,
                    &down_sql,
                )
                .await;

            if let Err(e) = result {
                return Err(anyhow!(
                    "マイグレーション {} のロールバックに失敗しました: {}",
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

    /// 利用可能なマイグレーションファイルを読み込む
    ///
    /// マイグレーションディレクトリをスキャンし、(version, description, path)のタプルを返す
    pub fn load_available_migrations(
        &self,
        migrations_dir: &Path,
    ) -> Result<Vec<(String, String, PathBuf)>> {
        let mut migrations = Vec::new();

        let entries = fs::read_dir(migrations_dir).with_context(|| {
            format!(
                "マイグレーションディレクトリの読み込みに失敗しました: {:?}",
                migrations_dir
            )
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow!("ディレクトリ名が無効です"))?;

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

    /// マイグレーションをトランザクション内でロールバック
    async fn rollback_migration_with_transaction(
        &self,
        pool: &sqlx::AnyPool,
        migrator: &DatabaseMigratorService,
        version: &str,
        down_sql: &str,
    ) -> Result<()> {
        // トランザクションを開始
        let mut tx = pool
            .begin()
            .await
            .with_context(|| "トランザクションの開始に失敗しました")?;

        // マイグレーションdown SQLを実行
        sqlx::query(down_sql)
            .execute(&mut *tx)
            .await
            .with_context(|| {
                format!(
                    "マイグレーションdown SQLの実行に失敗しました: {}\nSQL: {}",
                    version, down_sql
                )
            })?;

        // マイグレーション履歴から削除
        let remove_sql = migrator.generate_remove_migration_sql(version);

        sqlx::query(&remove_sql)
            .execute(&mut *tx)
            .await
            .with_context(|| "マイグレーション履歴の削除に失敗しました")?;

        // トランザクションをコミット
        tx.commit()
            .await
            .with_context(|| "トランザクションのコミットに失敗しました")?;

        Ok(())
    }

    /// ロールバック結果のサマリーを生成
    pub fn generate_summary(&self, rolled_back: &[AppliedMigration]) -> String {
        let mut summary = String::from("=== マイグレーションロールバック完了 ===\n");
        summary.push_str(&format!(
            "{} 個のマイグレーションをロールバックしました:\n\n",
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
        summary.push_str(&format!("\n合計実行時間: {}ms\n", total_duration));

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
        assert!(summary.contains("2 個のマイグレーション"));
        assert!(summary.contains("20260121120000"));
        assert!(summary.contains("20260121120001"));
        assert!(summary.contains("300ms")); // 100 + 200
    }
}
