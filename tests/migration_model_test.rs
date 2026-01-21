/// Migrationドメインモデルのテスト
///
/// マイグレーションファイル、マイグレーション履歴、適用済みマイグレーションなどの
/// ドメインモデルが正しく動作することを確認します。

use chrono::{Duration, Utc};
use std::path::PathBuf;

#[cfg(test)]
mod migration_model_tests {
    use super::*;

    /// MigrationFile構造体のデシリアライゼーションのテスト
    #[test]
    fn test_migration_file_deserialization() {
        use stratum::core::migration::MigrationFile;
        use stratum::core::config::Dialect;

        let yaml = r#"
version: "20260121120000"
description: "Initial schema"
dialect: postgresql
up_sql: "CREATE TABLE users (id INTEGER PRIMARY KEY);"
down_sql: "DROP TABLE users;"
file_path: "migrations/20260121120000_initial_schema.sql"
checksum: "abc123def456"
"#;

        let migration: MigrationFile =
            serde_saphyr::from_str(yaml).expect("Failed to deserialize MigrationFile");

        assert_eq!(migration.version, "20260121120000");
        assert_eq!(migration.description, "Initial schema");
        assert_eq!(migration.dialect, Dialect::PostgreSQL);
        assert!(migration.up_sql.contains("CREATE TABLE"));
        assert!(migration.down_sql.contains("DROP TABLE"));
        assert_eq!(migration.checksum, "abc123def456");
    }

    /// MigrationFile構造体のシリアライゼーションのテスト
    #[test]
    fn test_migration_file_serialization() {
        use stratum::core::migration::MigrationFile;
        use stratum::core::config::Dialect;

        let migration = MigrationFile {
            version: "20260121120000".to_string(),
            description: "Initial schema".to_string(),
            dialect: Dialect::PostgreSQL,
            up_sql: "CREATE TABLE users (id INTEGER PRIMARY KEY);".to_string(),
            down_sql: "DROP TABLE users;".to_string(),
            file_path: PathBuf::from("migrations/20260121120000_initial_schema.sql"),
            checksum: "abc123def456".to_string(),
        };

        let yaml = serde_saphyr::to_string(&migration).expect("Failed to serialize MigrationFile");
        assert!(yaml.contains("version"));
        assert!(yaml.contains("20260121120000"));
        assert!(yaml.contains("Initial schema"));
    }

    /// MigrationRecord構造体のデシリアライゼーションのテスト
    #[test]
    fn test_migration_record_deserialization() {
        use stratum::core::migration::MigrationRecord;

        let yaml = r#"
version: "20260121120000"
description: "Initial schema"
applied_at: "2026-01-21T12:00:00Z"
checksum: "abc123def456"
"#;

        let record: MigrationRecord =
            serde_saphyr::from_str(yaml).expect("Failed to deserialize MigrationRecord");

        assert_eq!(record.version, "20260121120000");
        assert_eq!(record.description, "Initial schema");
        assert_eq!(record.checksum, "abc123def456");
    }

    /// AppliedMigration構造体の作成テスト
    #[test]
    fn test_applied_migration_creation() {
        use stratum::core::migration::AppliedMigration;

        let now = Utc::now();
        let duration = Duration::seconds(5);

        let applied = AppliedMigration {
            version: "20260121120000".to_string(),
            description: "Initial schema".to_string(),
            applied_at: now,
            duration,
        };

        assert_eq!(applied.version, "20260121120000");
        assert_eq!(applied.description, "Initial schema");
        assert_eq!(applied.duration, duration);
    }

    /// MigrationStatus列挙型のテスト
    #[test]
    fn test_migration_status_variants() {
        use stratum::core::migration::MigrationStatus;

        let pending = MigrationStatus::Pending;
        let applied = MigrationStatus::Applied;
        let failed = MigrationStatus::Failed {
            error_message: "Connection timeout".to_string(),
        };

        assert!(matches!(pending, MigrationStatus::Pending));
        assert!(matches!(applied, MigrationStatus::Applied));
        assert!(matches!(failed, MigrationStatus::Failed { .. }));

        if let MigrationStatus::Failed { error_message } = failed {
            assert_eq!(error_message, "Connection timeout");
        }
    }

    /// Migration構造体の作成と検証のテスト
    #[test]
    fn test_migration_struct() {
        use stratum::core::migration::Migration;

        let migration = Migration {
            version: "20260121120000".to_string(),
            description: "Initial schema".to_string(),
            checksum: "abc123def456".to_string(),
            timestamp: Utc::now(),
        };

        assert_eq!(migration.version, "20260121120000");
        assert_eq!(migration.description, "Initial schema");
        assert_eq!(migration.checksum, "abc123def456");
    }

    /// MigrationHistory構造体のテスト
    #[test]
    fn test_migration_history() {
        use stratum::core::migration::{MigrationHistory, MigrationRecord};

        let mut history = MigrationHistory::new();
        assert_eq!(history.records.len(), 0);

        let record = MigrationRecord {
            version: "20260121120000".to_string(),
            description: "Initial schema".to_string(),
            applied_at: Utc::now(),
            checksum: "abc123def456".to_string(),
        };

        history.add_record(record.clone());
        assert_eq!(history.records.len(), 1);

        let latest = history.get_latest_version();
        assert_eq!(latest, Some("20260121120000"));
    }

    /// MigrationFileのバージョン形式検証のテスト
    #[test]
    fn test_migration_file_version_format() {
        use stratum::core::migration::MigrationFile;

        let migration = MigrationFile {
            version: "20260121120000".to_string(),
            description: "Test".to_string(),
            dialect: stratum::core::config::Dialect::PostgreSQL,
            up_sql: "".to_string(),
            down_sql: "".to_string(),
            file_path: PathBuf::from("test.sql"),
            checksum: "".to_string(),
        };

        // バージョンが14桁のタイムスタンプ形式であることを確認
        assert_eq!(migration.version.len(), 14);
        assert!(migration.version.chars().all(|c| c.is_ascii_digit()));
    }
}
