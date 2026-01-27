// マイグレーションドメインモデル
//
// データベースマイグレーションの定義と履歴管理を表現する型システム。
// MigrationFile, Migration, MigrationRecord, MigrationHistory などの構造体を提供します。

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::core::config::Dialect;
use crate::core::destructive_change_report::DestructiveChangeReport;

/// マイグレーションファイル
///
/// マイグレーションファイル（.meta.yaml）に保存される情報を表現します。
/// up.sqlとdown.sqlの内容、チェックサム、対象データベース方言などを含みます。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationFile {
    /// マイグレーションバージョン（タイムスタンプ: YYYYMMDDHHmmss）
    pub version: String,

    /// マイグレーションの説明
    pub description: String,

    /// 対象データベース方言
    pub dialect: Dialect,

    /// アップグレードSQL（スキーマ適用）
    pub up_sql: String,

    /// ダウングレードSQL（スキーマロールバック）
    pub down_sql: String,

    /// マイグレーションファイルのパス
    pub file_path: PathBuf,

    /// マイグレーションファイルのチェックサム（SHA-256）
    pub checksum: String,
}

/// マイグレーションメタデータ
///
/// .meta.yaml に保存される情報を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationMetadata {
    /// マイグレーションバージョン
    pub version: String,

    /// マイグレーションの説明
    pub description: String,

    /// 対象データベース方言
    pub dialect: Dialect,

    /// マイグレーションファイルのチェックサム
    pub checksum: String,

    /// 破壊的変更の検出結果
    #[serde(default)]
    pub destructive_changes: Option<DestructiveChangeReport>,
}

/// 破壊的変更の判定結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestructiveChangeStatus {
    /// 破壊的変更なし
    None,
    /// 破壊的変更あり
    Present,
    /// 旧メタデータ（判定不能のため破壊的扱い）
    Legacy,
}

impl MigrationMetadata {
    /// 破壊的変更の有無を判定
    pub fn destructive_change_status(&self) -> DestructiveChangeStatus {
        match &self.destructive_changes {
            None => DestructiveChangeStatus::Legacy,
            Some(report) => {
                if report.has_destructive_changes() {
                    DestructiveChangeStatus::Present
                } else {
                    DestructiveChangeStatus::None
                }
            }
        }
    }
}

impl MigrationFile {
    /// 新しいマイグレーションファイルを作成
    pub fn new(
        version: String,
        description: String,
        dialect: Dialect,
        up_sql: String,
        down_sql: String,
        file_path: PathBuf,
        checksum: String,
    ) -> Self {
        Self {
            version,
            description,
            dialect,
            up_sql,
            down_sql,
            file_path,
            checksum,
        }
    }

    /// バージョン形式が有効かどうかを検証
    pub fn validate_version(&self) -> bool {
        // YYYYMMDDHHmmss形式（14桁の数字）かどうかをチェック
        self.version.len() == 14 && self.version.chars().all(|c| c.is_ascii_digit())
    }
}

/// マイグレーション
///
/// マイグレーションの基本情報を表現します。
/// バージョン、説明、チェックサム、タイムスタンプを含みます。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Migration {
    /// マイグレーションバージョン
    pub version: String,

    /// マイグレーションの説明
    pub description: String,

    /// マイグレーションファイルのチェックサム
    pub checksum: String,

    /// マイグレーションが作成された日時
    pub timestamp: DateTime<Utc>,
}

impl Migration {
    /// 新しいマイグレーションを作成
    pub fn new(version: String, description: String, checksum: String) -> Self {
        Self {
            version,
            description,
            checksum,
            timestamp: Utc::now(),
        }
    }
}

/// マイグレーション記録
///
/// データベースに記録された適用済みマイグレーションの情報を表現します。
/// schema_migrationsテーブルに保存されるレコードに対応します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationRecord {
    /// マイグレーションバージョン
    pub version: String,

    /// マイグレーションの説明
    pub description: String,

    /// マイグレーションが適用された日時
    pub applied_at: DateTime<Utc>,

    /// マイグレーションファイルのチェックサム
    pub checksum: String,
}

impl MigrationRecord {
    /// 新しいマイグレーション記録を作成
    pub fn new(version: String, description: String, checksum: String) -> Self {
        Self {
            version,
            description,
            applied_at: Utc::now(),
            checksum,
        }
    }

    /// チェックサムが一致するか確認
    pub fn verify_checksum(&self, expected_checksum: &str) -> bool {
        self.checksum == expected_checksum
    }
}

/// 適用済みマイグレーション
///
/// マイグレーション適用時の実行情報を表現します。
/// 適用日時、実行時間などを含みます。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppliedMigration {
    /// マイグレーションバージョン
    pub version: String,

    /// マイグレーションの説明
    pub description: String,

    /// マイグレーションが適用された日時
    pub applied_at: DateTime<Utc>,

    /// マイグレーション適用にかかった時間
    #[serde(with = "duration_serde")]
    pub duration: Duration,
}

impl AppliedMigration {
    /// 新しい適用済みマイグレーションを作成
    pub fn new(
        version: String,
        description: String,
        applied_at: DateTime<Utc>,
        duration: Duration,
    ) -> Self {
        Self {
            version,
            description,
            applied_at,
            duration,
        }
    }
}

// chronoのDurationをシリアライズ/デシリアライズするためのヘルパー
mod duration_serde {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(duration.num_seconds())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds = i64::deserialize(deserializer)?;
        Ok(Duration::seconds(seconds))
    }
}

/// マイグレーションステータス
///
/// マイグレーションの状態を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum MigrationStatus {
    /// 未適用
    Pending,

    /// 適用済み
    Applied,

    /// 失敗
    Failed {
        /// エラーメッセージ
        error_message: String,
    },
}

impl MigrationStatus {
    /// ステータスの種類を文字列で取得
    pub fn kind(&self) -> &'static str {
        match self {
            MigrationStatus::Pending => "Pending",
            MigrationStatus::Applied => "Applied",
            MigrationStatus::Failed { .. } => "Failed",
        }
    }

    /// 失敗状態かどうか
    pub fn is_failed(&self) -> bool {
        matches!(self, MigrationStatus::Failed { .. })
    }

    /// 適用済み状態かどうか
    pub fn is_applied(&self) -> bool {
        matches!(self, MigrationStatus::Applied)
    }
}

/// マイグレーション履歴
///
/// データベースに適用されたマイグレーションの履歴を表現します。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationHistory {
    /// マイグレーション記録のリスト
    pub records: Vec<MigrationRecord>,
}

impl MigrationHistory {
    /// 新しいマイグレーション履歴を作成
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// マイグレーション記録を追加
    pub fn add_record(&mut self, record: MigrationRecord) {
        self.records.push(record);
    }

    /// 最新のマイグレーションバージョンを取得
    pub fn get_latest_version(&self) -> Option<&str> {
        self.records.last().map(|r| r.version.as_str())
    }

    /// 指定されたバージョンの記録を取得
    pub fn get_record(&self, version: &str) -> Option<&MigrationRecord> {
        self.records.iter().find(|r| r.version == version)
    }

    /// マイグレーション記録の数を取得
    pub fn count(&self) -> usize {
        self.records.len()
    }

    /// 指定されたバージョンが適用済みか確認
    pub fn is_applied(&self, version: &str) -> bool {
        self.records.iter().any(|r| r.version == version)
    }
}

impl Default for MigrationHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::destructive_change_report::DestructiveChangeReport;

    #[test]
    fn test_migration_file_new() {
        let migration = MigrationFile::new(
            "20260121120000".to_string(),
            "Initial schema".to_string(),
            Dialect::PostgreSQL,
            "CREATE TABLE users (id INTEGER PRIMARY KEY);".to_string(),
            "DROP TABLE users;".to_string(),
            PathBuf::from("migrations/20260121120000_initial_schema.sql"),
            "abc123def456".to_string(),
        );

        assert_eq!(migration.version, "20260121120000");
        assert_eq!(migration.description, "Initial schema");
        assert!(migration.validate_version());
    }

    #[test]
    fn test_migration_new() {
        let migration = Migration::new(
            "20260121120000".to_string(),
            "Initial schema".to_string(),
            "abc123def456".to_string(),
        );

        assert_eq!(migration.version, "20260121120000");
        assert_eq!(migration.description, "Initial schema");
        assert_eq!(migration.checksum, "abc123def456");
    }

    #[test]
    fn test_migration_record_new() {
        let record = MigrationRecord::new(
            "20260121120000".to_string(),
            "Initial schema".to_string(),
            "abc123def456".to_string(),
        );

        assert_eq!(record.version, "20260121120000");
        assert!(record.verify_checksum("abc123def456"));
        assert!(!record.verify_checksum("wrong_checksum"));
    }

    #[test]
    fn test_migration_status_kind() {
        assert_eq!(MigrationStatus::Pending.kind(), "Pending");
        assert_eq!(MigrationStatus::Applied.kind(), "Applied");
        assert_eq!(
            MigrationStatus::Failed {
                error_message: "Error".to_string()
            }
            .kind(),
            "Failed"
        );
    }

    #[test]
    fn test_migration_history_operations() {
        let mut history = MigrationHistory::new();
        assert_eq!(history.count(), 0);
        assert_eq!(history.get_latest_version(), None);

        let record = MigrationRecord::new(
            "20260121120000".to_string(),
            "Initial schema".to_string(),
            "abc123def456".to_string(),
        );

        history.add_record(record);
        assert_eq!(history.count(), 1);
        assert_eq!(history.get_latest_version(), Some("20260121120000"));
        assert!(history.is_applied("20260121120000"));
        assert!(!history.is_applied("20260121120001"));
    }

    #[test]
    fn test_metadata_missing_destructive_changes_is_legacy() {
        let yaml = r#"version: "20260125120000"
description: "legacy"
dialect: postgresql
checksum: "abc123"
"#;

        let metadata: MigrationMetadata =
            serde_saphyr::from_str(yaml).expect("Failed to deserialize metadata");

        assert_eq!(metadata.destructive_changes, None);
        assert_eq!(
            metadata.destructive_change_status(),
            DestructiveChangeStatus::Legacy
        );
    }

    #[test]
    fn test_metadata_empty_destructive_changes_is_none() {
        let yaml = r#"version: "20260125120000"
description: "safe"
dialect: postgresql
checksum: "abc123"
destructive_changes: {}
"#;

        let metadata: MigrationMetadata =
            serde_saphyr::from_str(yaml).expect("Failed to deserialize metadata");

        assert_eq!(metadata.destructive_changes, Some(DestructiveChangeReport::new()));
        assert_eq!(
            metadata.destructive_change_status(),
            DestructiveChangeStatus::None
        );
    }

    #[test]
    fn test_metadata_destructive_changes_present() {
        let yaml = r#"version: "20260125120000"
description: "drop"
dialect: postgresql
checksum: "abc123"
destructive_changes:
  tables_dropped:
    - "users"
"#;

        let metadata: MigrationMetadata =
            serde_saphyr::from_str(yaml).expect("Failed to deserialize metadata");

        assert!(metadata
            .destructive_changes
            .as_ref()
            .expect("report should exist")
            .has_destructive_changes());
        assert_eq!(
            metadata.destructive_change_status(),
            DestructiveChangeStatus::Present
        );
    }
}
