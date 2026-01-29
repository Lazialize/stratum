// statusコマンドハンドラー
//
// マイグレーション状態の確認機能を実装します。
// - データベース接続と履歴テーブルの読み込み
// - ローカルマイグレーションファイルとの照合
// - 適用済み/未適用の状態表示（テーブル形式）
// - チェックサム不一致の検出と警告

use crate::cli::command_context::CommandContext;
use crate::cli::commands::migration_loader;
use crate::core::migration::{Migration, MigrationMetadata, MigrationRecord};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// statusコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct StatusCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// カスタム設定ファイルパス
    pub config_path: Option<PathBuf>,
    /// 環境名
    pub env: String,
}

/// statusコマンドハンドラー
#[derive(Debug, Default)]
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
        let context = CommandContext::load_with_config(
            command.project_path.clone(),
            command.config_path.clone(),
        )?;

        // マイグレーションディレクトリのパスを解決
        let migrations_dir = context.require_migrations_dir()?;

        // ローカルマイグレーションファイルを読み込む
        let local_migrations = self.load_local_migrations(&migrations_dir)?;

        // マイグレーションが存在しない場合
        if local_migrations.is_empty() {
            return Ok(self.format_no_migrations());
        }

        // データベースに接続し、マイグレーション履歴を取得
        let (_pool, applied_migrations) = context.connect_and_load_migrations(&command.env).await?;

        // マイグレーション状態を生成
        let status_list = self.build_migration_status(&local_migrations, &applied_migrations);

        // 各ステータスの数を計算
        let applied_count = status_list
            .iter()
            .filter(|(_, _, status)| status.starts_with("Applied"))
            .count();
        let pending_count = status_list
            .iter()
            .filter(|(_, _, status)| status == "Pending")
            .count();
        let orphaned_count = status_list
            .iter()
            .filter(|(_, _, status)| status == "Orphaned")
            .count();

        // フォーマット用に参照のベクタを作成
        let status_list_refs: Vec<(&str, &str, &str)> = status_list
            .iter()
            .map(|(v, d, s)| (v.as_str(), d.as_str(), s.as_str()))
            .collect();

        // フォーマットして返す
        Ok(self.format_migration_status(&status_list_refs, applied_count, pending_count, orphaned_count))
    }

    /// ローカルマイグレーションファイルを読み込む
    fn load_local_migrations(&self, migrations_dir: &Path) -> Result<Vec<Migration>> {
        let available = migration_loader::load_available_migrations(migrations_dir)?;

        let mut migrations = Vec::new();
        for (version, description, path) in available {
            // メタデータファイルからチェックサムを読み込む
            let meta_path = path.join(".meta.yaml");
            let checksum = if meta_path.exists() {
                let meta_content = fs::read_to_string(&meta_path)?;
                self.extract_checksum_from_meta(&meta_content)?
            } else {
                "unknown".to_string()
            };

            migrations.push(Migration::new(version, description, checksum));
        }

        Ok(migrations)
    }

    /// メタデータファイルからチェックサムを抽出
    fn extract_checksum_from_meta(&self, meta_content: &str) -> Result<String> {
        let metadata: MigrationMetadata =
            serde_saphyr::from_str(meta_content).with_context(|| "Failed to parse metadata YAML")?;
        Ok(metadata.checksum)
    }

    /// マイグレーション状態のリストを構築
    ///
    /// ローカルマイグレーションと適用済みマイグレーションを照合し、
    /// ローカルに存在しないがDBに記録されているマイグレーションを "Orphaned" として検出する。
    fn build_migration_status(
        &self,
        local_migrations: &[Migration],
        applied_migrations: &[MigrationRecord],
    ) -> Vec<(String, String, String)> {
        let applied_map: HashMap<&str, &MigrationRecord> = applied_migrations
            .iter()
            .map(|m| (m.version.as_str(), m))
            .collect();

        let local_version_set: std::collections::HashSet<&str> = local_migrations
            .iter()
            .map(|m| m.version.as_str())
            .collect();

        // ローカルマイグレーションの状態
        let mut status_list: Vec<(String, String, String)> = local_migrations
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

                (
                    local.version.clone(),
                    local.description.clone(),
                    status,
                )
            })
            .collect();

        // 孤立マイグレーション（DBに記録があるがローカルに存在しない）を追加
        for record in applied_migrations {
            if !local_version_set.contains(record.version.as_str()) {
                status_list.push((
                    record.version.clone(),
                    record.description.clone(),
                    "Orphaned".to_string(),
                ));
            }
        }

        // バージョン順にソート
        status_list.sort_by(|a, b| a.0.cmp(&b.0));

        status_list
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
        orphaned_count: usize,
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
            } else if *status == "Orphaned" {
                "⚠️  Orphaned"
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
        if orphaned_count > 0 {
            output.push_str(&format!(
                "Total: {} (Applied: {}, Pending: {}, Orphaned: {})\n",
                status_list.len(),
                applied_count,
                pending_count,
                orphaned_count
            ));
        } else {
            output.push_str(&format!(
                "Total: {} (Applied: {}, Pending: {})\n",
                status_list.len(),
                applied_count,
                pending_count
            ));
        }

        // チェックサム不一致の警告
        if status_list
            .iter()
            .any(|(_, _, s)| s.contains("checksum mismatch"))
        {
            output.push_str("\n⚠️  Warning: Some migrations have mismatched checksums.\n");
            output.push_str("   Migration files may have been modified after being applied.\n");
        }

        // 孤立マイグレーションの警告
        if orphaned_count > 0 {
            output.push_str("\n⚠️  Warning: Orphaned migrations detected.\n");
            output.push_str("   These migrations exist in the database but their local files are missing.\n");
        }

        output
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
dialect: sqlite
checksum: "test_checksum_123"
destructive_changes: {}
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
    fn test_build_migration_status_with_orphaned() {
        let handler = StatusCommandHandler::new();

        let local_migrations = vec![Migration::new(
            "20260121120001".to_string(),
            "create_posts".to_string(),
            "checksum2".to_string(),
        )];

        let applied_migrations = vec![
            MigrationRecord::new(
                "20260121120000".to_string(),
                "create_users".to_string(),
                "checksum1".to_string(),
            ),
            MigrationRecord::new(
                "20260121120001".to_string(),
                "create_posts".to_string(),
                "checksum2".to_string(),
            ),
        ];

        let status_list = handler.build_migration_status(&local_migrations, &applied_migrations);

        assert_eq!(status_list.len(), 2);
        assert_eq!(status_list[0].0, "20260121120000");
        assert_eq!(status_list[0].2, "Orphaned");
        assert_eq!(status_list[1].0, "20260121120001");
        assert_eq!(status_list[1].2, "Applied");
    }

    #[test]
    fn test_format_migration_status() {
        let handler = StatusCommandHandler::new();

        let status_list = vec![
            ("20260121120000", "create_users", "Applied"),
            ("20260121120001", "create_posts", "Pending"),
        ];

        let summary = handler.format_migration_status(&status_list, 1, 1, 0);

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
    fn test_format_migration_status_with_orphaned() {
        let handler = StatusCommandHandler::new();

        let status_list = vec![
            ("20260121120000", "create_users", "Orphaned"),
            ("20260121120001", "create_posts", "Applied"),
        ];

        let summary = handler.format_migration_status(&status_list, 1, 0, 1);

        assert!(summary.contains("Orphaned: 1"));
        assert!(summary.contains("⚠️  Orphaned"));
        assert!(summary.contains("Orphaned migrations detected"));
    }

    #[test]
    fn test_format_no_migrations() {
        let handler = StatusCommandHandler::new();
        let output = handler.format_no_migrations();

        assert!(output.contains("Migration Status"));
        assert!(output.contains("No migrations found"));
    }
}
