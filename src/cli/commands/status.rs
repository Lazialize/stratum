// statusコマンドハンドラー
//
// マイグレーション状態の確認機能を実装します。
// - データベース接続と履歴テーブルの読み込み
// - ローカルマイグレーションファイルとの照合
// - 適用済み/未適用の状態表示（テーブル形式）
// - チェックサム不一致の検出と警告

use crate::adapters::database::DatabaseConnectionService;
use crate::adapters::database_migrator::DatabaseMigratorService;
use crate::cli::command_context::CommandContext;
use crate::core::migration::{Migration, MigrationRecord};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// statusコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct StatusCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// 環境名
    pub env: String,
}

/// statusコマンドハンドラー
#[derive(Debug, Clone)]
pub struct StatusCommandHandler {}

impl StatusCommandHandler {
    /// 新しいStatusCommandHandlerを作成
    pub fn new() -> Self {
        Self {}
    }

    /// statusコマンドを実行
    ///
    /// # Arguments
    ///
    /// * `command` - statusコマンドのパラメータ
    ///
    /// # Returns
    ///
    /// 成功時はマイグレーション状態のサマリー、失敗時はエラーメッセージ
    pub async fn execute(&self, command: &StatusCommand) -> Result<String> {
        // 設定ファイルを読み込む
        let context = CommandContext::load(command.project_path.clone())?;
        let config = &context.config;

        // マイグレーションディレクトリのパスを解決
        let migrations_dir = context.require_migrations_dir()?;

        // ローカルマイグレーションファイルを読み込む
        let local_migrations = self.load_local_migrations(&migrations_dir)?;

        // マイグレーションが存在しない場合
        if local_migrations.is_empty() {
            return Ok(self.format_no_migrations());
        }

        // データベースに接続して適用済みマイグレーションを取得
        let db_config = context.database_config(&command.env)?;

        let db_service = DatabaseConnectionService::new();
        let pool = db_service
            .create_pool(config.dialect, &db_config)
            .await
            .with_context(|| "Failed to connect to database")?;

        let migrator = DatabaseMigratorService::new();

        // マイグレーション履歴テーブルを作成（存在しない場合）
        migrator
            .create_migration_table(&pool, config.dialect)
            .await
            .with_context(|| "Failed to create migration history table")?;

        // 適用済みマイグレーションを取得
        let applied_migrations = migrator
            .get_migrations(&pool, config.dialect)
            .await
            .with_context(|| "Failed to get applied migrations")?;

        // マイグレーション状態を生成
        let status_list = self.build_migration_status(&local_migrations, &applied_migrations);

        // 適用済み/未適用の数を計算
        let applied_count = status_list
            .iter()
            .filter(|(_, _, status)| status.contains("Applied"))
            .count();
        let pending_count = status_list
            .iter()
            .filter(|(_, _, status)| status.contains("Pending"))
            .count();

        // フォーマット用に参照のベクタを作成
        let status_list_refs: Vec<(&str, &str, &str)> = status_list
            .iter()
            .map(|(v, d, s)| (*v, *d, s.as_str()))
            .collect();

        // フォーマットして返す
        Ok(self.format_migration_status(&status_list_refs, applied_count, pending_count))
    }

    /// ローカルマイグレーションファイルを読み込む
    fn load_local_migrations(&self, migrations_dir: &PathBuf) -> Result<Vec<Migration>> {
        let mut migrations = Vec::new();

        let entries = fs::read_dir(migrations_dir).with_context(|| {
            format!("Failed to read migrations directory: {:?}", migrations_dir)
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // ディレクトリ名から version と description を抽出
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow!("Invalid directory name: {:?}", path))?;

            // フォーマット: {version}_{description}
            let parts: Vec<&str> = dir_name.splitn(2, '_').collect();
            if parts.len() != 2 {
                continue;
            }

            let version = parts[0].to_string();
            let description = parts[1].to_string();

            // メタデータファイルからチェックサムを読み込む
            let meta_path = path.join(".meta.yaml");
            let checksum = if meta_path.exists() {
                let meta_content = fs::read_to_string(&meta_path)?;
                self.extract_checksum_from_meta(&meta_content)?
            } else {
                "unknown".to_string()
            };

            let migration = Migration::new(version, description, checksum);
            migrations.push(migration);
        }

        // バージョン順にソート
        migrations.sort_by(|a, b| a.version.cmp(&b.version));

        Ok(migrations)
    }

    /// メタデータファイルからチェックサムを抽出
    fn extract_checksum_from_meta(&self, meta_content: &str) -> Result<String> {
        for line in meta_content.lines() {
            if line.starts_with("checksum:") {
                let checksum = line
                    .trim_start_matches("checksum:")
                    .trim()
                    .trim_matches('"')
                    .to_string();
                return Ok(checksum);
            }
        }
        Err(anyhow!("Checksum not found in metadata file"))
    }

    /// マイグレーション状態のリストを構築
    fn build_migration_status<'a>(
        &self,
        local_migrations: &'a [Migration],
        applied_migrations: &[MigrationRecord],
    ) -> Vec<(&'a str, &'a str, String)> {
        let applied_map: HashMap<&str, &MigrationRecord> = applied_migrations
            .iter()
            .map(|m| (m.version.as_str(), m))
            .collect();

        local_migrations
            .iter()
            .map(|local| {
                let status = if let Some(applied) = applied_map.get(local.version.as_str()) {
                    if applied.checksum == local.checksum {
                        "Applied".to_string()
                    } else {
                        "Applied (checksum mismatch)".to_string()
                    }
                } else {
                    "Pending".to_string()
                };

                (local.version.as_str(), local.description.as_str(), status)
            })
            .collect()
    }

    /// マイグレーションが存在しない場合のメッセージ
    fn format_no_migrations(&self) -> String {
        let mut output = String::new();
        output.push_str("=== Migration Status ===\n\n");
        output.push_str("No migrations found.\n");
        output.push_str("\nUse the `generate` command to create migrations.\n");
        output
    }

    /// マイグレーション状態をフォーマット
    pub fn format_migration_status(
        &self,
        status_list: &[(&str, &str, &str)],
        applied_count: usize,
        pending_count: usize,
    ) -> String {
        let mut output = String::new();

        output.push_str("=== Migration Status ===\n\n");

        // テーブルヘッダー
        output.push_str(&format!(
            "{:<20} {:<40} {:<30}\n",
            "Version", "Description", "Status"
        ));
        output.push_str(&format!("{}\n", "-".repeat(90)));

        // 各マイグレーションの状態
        for (version, description, status) in status_list {
            let status_display = if status.contains("checksum mismatch") {
                "⚠️  Applied (checksum mismatch)"
            } else if status.contains("Applied") {
                "✓ Applied"
            } else {
                "  Pending"
            };

            output.push_str(&format!(
                "{:<20} {:<40} {:<30}\n",
                version, description, status_display
            ));
        }

        // サマリー
        output.push_str(&format!("\n{}\n", "-".repeat(90)));
        output.push_str(&format!(
            "Total: {} (Applied: {}, Pending: {})\n",
            status_list.len(),
            applied_count,
            pending_count
        ));

        // チェックサム不一致の警告
        if status_list
            .iter()
            .any(|(_, _, s)| s.contains("checksum mismatch"))
        {
            output.push_str("\n⚠️  Warning: Some migrations have mismatched checksums.\n");
            output.push_str("   Migration files may have been modified after being applied.\n");
        }

        output
    }
}

impl Default for StatusCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_handler() {
        let handler = StatusCommandHandler::new();
        assert!(format!("{:?}", handler).contains("StatusCommandHandler"));
    }

    #[test]
    fn test_extract_checksum_from_meta() {
        let handler = StatusCommandHandler::new();

        let meta_content = r#"version: "20260121120000"
description: "create_users"
checksum: "test_checksum_123"
"#;

        let checksum = handler.extract_checksum_from_meta(meta_content).unwrap();
        assert_eq!(checksum, "test_checksum_123");
    }

    #[test]
    fn test_build_migration_status() {
        let handler = StatusCommandHandler::new();

        let local_migrations = vec![
            Migration::new(
                "20260121120000".to_string(),
                "create_users".to_string(),
                "checksum1".to_string(),
            ),
            Migration::new(
                "20260121120001".to_string(),
                "create_posts".to_string(),
                "checksum2".to_string(),
            ),
        ];

        let applied_migrations = vec![MigrationRecord::new(
            "20260121120000".to_string(),
            "create_users".to_string(),
            "checksum1".to_string(),
        )];

        let status_list = handler.build_migration_status(&local_migrations, &applied_migrations);

        assert_eq!(status_list.len(), 2);
        assert_eq!(status_list[0].2, "Applied");
        assert_eq!(status_list[1].2, "Pending");
    }

    #[test]
    fn test_build_migration_status_with_checksum_mismatch() {
        let handler = StatusCommandHandler::new();

        let local_migrations = vec![Migration::new(
            "20260121120000".to_string(),
            "create_users".to_string(),
            "checksum_new".to_string(),
        )];

        let applied_migrations = vec![MigrationRecord::new(
            "20260121120000".to_string(),
            "create_users".to_string(),
            "checksum_old".to_string(),
        )];

        let status_list = handler.build_migration_status(&local_migrations, &applied_migrations);

        assert_eq!(status_list.len(), 1);
        assert_eq!(status_list[0].2, "Applied (checksum mismatch)");
    }

    #[test]
    fn test_format_migration_status() {
        let handler = StatusCommandHandler::new();

        let status_list = vec![
            ("20260121120000", "create_users", "Applied"),
            ("20260121120001", "create_posts", "Pending"),
        ];

        let summary = handler.format_migration_status(&status_list, 1, 1);

        assert!(summary.contains("Migration Status"));
        assert!(summary.contains("20260121120000"));
        assert!(summary.contains("create_users"));
        assert!(summary.contains("✓ Applied"));
        assert!(summary.contains("Pending"));
        assert!(summary.contains("Total: 2"));
        assert!(summary.contains("Applied: 1"));
        assert!(summary.contains("Pending: 1"));
    }

    #[test]
    fn test_format_no_migrations() {
        let handler = StatusCommandHandler::new();
        let output = handler.format_no_migrations();

        assert!(output.contains("Migration Status"));
        assert!(output.contains("No migrations found"));
    }
}
