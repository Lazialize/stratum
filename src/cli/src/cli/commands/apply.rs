// applyコマンドハンドラー
//
// マイグレーションの適用機能を実装します。
// - データベース接続の確立
// - 未適用マイグレーションの検出
// - マイグレーションの順次実行（トランザクション内）
// - 実行結果の記録とチェックサムの保存
// - 実行ログの表示

use crate::adapters::database_migrator::DatabaseMigratorService;
use crate::cli::command_context::CommandContext;
use crate::cli::commands::destructive_change_formatter::DestructiveChangeFormatter;
use crate::cli::commands::migration_loader;
use crate::cli::commands::split_sql_statements;
use crate::cli::commands::DESTRUCTIVE_SQL_REGEX;
use crate::core::config::Dialect;
use crate::core::migration::{
    AppliedMigration, DestructiveChangeStatus, Migration, MigrationMetadata, MigrationRecord,
};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

/// applyコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct ApplyCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// カスタム設定ファイルパス
    pub config_path: Option<PathBuf>,
    /// Dry run - 実行せずにSQLを表示
    pub dry_run: bool,
    /// 対象環境
    pub env: String,
    /// タイムアウト（秒）
    pub timeout: Option<u64>,
    /// 破壊的変更を許可
    pub allow_destructive: bool,
}

/// applyコマンドハンドラー
#[derive(Debug, Default)]
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
        let context =
            CommandContext::load_with_config(command.project_path.clone(), command.config_path.clone())?;
        let config = &context.config;

        // マイグレーションディレクトリのパスを解決
        let migrations_dir = context.require_migrations_dir()?;

        // 利用可能なマイグレーションファイルを読み込む
        let available_migrations = migration_loader::load_available_migrations(&migrations_dir)?;

        if available_migrations.is_empty() {
            return Ok("No migration files found.".to_string());
        }

        // データベース接続を確立し、マイグレーション履歴を取得
        // dry-run モードでも DB に接続して適用済みマイグレーションを確認する
        let (pool, applied_migrations) = context
            .connect_and_load_migrations_with_timeout(&command.env, command.timeout)
            .await?;

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
            return Ok("No pending migrations to apply. Database is up to date.".to_string());
        }

        // 適用済みマイグレーションのチェックサム検証
        let checksum_warnings =
            self.verify_applied_checksums(&available_migrations, &applied_migrations);
        for warning in &checksum_warnings {
            eprintln!("{}", warning.yellow());
        }

        // Dry run モードの場合は SQL を表示して終了
        if command.dry_run {
            return self.execute_dry_run(&pending_migrations);
        }

        let migrator = DatabaseMigratorService::new();

        // マイグレーションを順次適用
        let mut applied = Vec::new();
        let mut warnings = Vec::new();
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
            let metadata: MigrationMetadata = serde_saphyr::from_str(&meta_content)
                .with_context(|| "Failed to parse metadata")?;

            // 破壊的変更の判定
            match metadata.destructive_change_status() {
                DestructiveChangeStatus::Present => {
                    let report = &metadata.destructive_changes;
                    if !command.allow_destructive {
                        let formatter = DestructiveChangeFormatter::new();
                        let mut message = String::new();
                        message.push_str(&format!("Migration: {}\n\n", version));
                        message.push_str(&formatter.format_error(report, "strata apply"));
                        return Err(anyhow!(message));
                    }
                    warnings.push(DestructiveChangeFormatter::new().format_warning(report));
                }
                DestructiveChangeStatus::None => {}
            }

            let checksum = metadata.checksum.clone();

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
        let summary = self.generate_summary(&applied);
        if warnings.is_empty() {
            Ok(summary)
        } else {
            Ok(format!("{}\n{}", warnings.join("\n"), summary))
        }
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

        let mut has_destructive = false;

        for (version, description, migration_dir) in pending_migrations {
            let up_sql_path = migration_dir.join("up.sql");
            let up_sql = fs::read_to_string(&up_sql_path)
                .with_context(|| format!("Failed to read migration file: {:?}", up_sql_path))?;

            let meta_path = migration_dir.join(".meta.yaml");
            let meta_content = fs::read_to_string(&meta_path)
                .with_context(|| format!("Failed to read metadata file: {:?}", meta_path))?;
            let metadata: MigrationMetadata = serde_saphyr::from_str(&meta_content)
                .with_context(|| format!("Failed to parse metadata: {:?}", meta_path))?;
            let destructive_status = metadata.destructive_change_status();

            output.push_str(&format!("\u{25b6} {} - {}\n", version, description));

            match destructive_status {
                DestructiveChangeStatus::Present => {
                    has_destructive = true;
                    output.push_str(
                        &format!("{}\n", "⚠ Destructive Changes Detected".red().bold()).to_string(),
                    );
                }
                DestructiveChangeStatus::None => {}
            }

            output.push_str("SQL:\n");
            let rendered_sql = if destructive_status == DestructiveChangeStatus::Present {
                self.highlight_destructive_sql(&up_sql)
            } else {
                up_sql
            };
            output.push_str(&format!("{}\n\n", rendered_sql));
        }

        if has_destructive {
            output.push_str("To proceed, run with --allow-destructive flag\n");
        }

        Ok(output)
    }

    fn highlight_destructive_sql(&self, sql: &str) -> String {
        let regex = &*DESTRUCTIVE_SQL_REGEX;

        let mut rendered = Vec::new();
        for line in sql.lines() {
            if regex.is_match(line) {
                rendered.push(line.red().to_string());
            } else {
                rendered.push(line.to_string());
            }
        }
        rendered.join("\n")
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

    /// 適用済みマイグレーションのチェックサム検証
    ///
    /// ローカルファイルのチェックサムと DB 記録のチェックサムを比較し、
    /// 不一致がある場合は警告を返す。
    fn verify_applied_checksums(
        &self,
        available_migrations: &[(String, String, PathBuf)],
        applied_migrations: &[MigrationRecord],
    ) -> Vec<String> {
        let mut warnings = Vec::new();

        for record in applied_migrations {
            // ローカルにファイルがあるか確認
            if let Some((_, _, migration_dir)) = available_migrations
                .iter()
                .find(|(v, _, _)| v == &record.version)
            {
                let meta_path = migration_dir.join(".meta.yaml");
                if meta_path.exists() {
                    if let Ok(meta_content) = fs::read_to_string(&meta_path) {
                        if let Ok(metadata) =
                            serde_saphyr::from_str::<MigrationMetadata>(&meta_content)
                        {
                            if metadata.checksum != record.checksum {
                                warnings.push(format!(
                                    "Warning: Checksum mismatch for migration {}: local={}, applied={}",
                                    record.version,
                                    metadata.checksum,
                                    record.checksum
                                ));
                            }
                        }
                    }
                }
            }
        }

        warnings
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

    #[test]
    fn test_highlight_destructive_sql_marks_drop() {
        use colored::control;

        let handler = ApplyCommandHandler::new();
        let sql = "CREATE TABLE users (id INTEGER);\nDROP TABLE users;";

        control::set_override(true);
        let rendered = handler.highlight_destructive_sql(sql);
        control::set_override(false);

        assert!(rendered.contains("\u{1b}[31m"));
        assert!(rendered.contains("DROP TABLE users;"));
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
