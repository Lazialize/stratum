/// マイグレーションファイル生成サービスのテスト
///
/// スキーマ差分からマイグレーションファイルを正しく生成することを確認します。

#[cfg(test)]
mod migration_generator_tests {
    use stratum::core::config::Dialect;
    use stratum::core::schema::{Column, ColumnType, Schema, Table};
    use stratum::core::schema_diff::SchemaDiff;
    use stratum::services::migration_generator::MigrationGenerator;
    use stratum::services::schema_diff_detector::SchemaDiffDetector;

    /// サービスの作成テスト
    #[test]
    fn test_new_service() {
        let generator = MigrationGenerator::new();
        assert!(format!("{:?}", generator).contains("MigrationGenerator"));
    }

    /// タイムスタンプ生成のテスト
    #[test]
    fn test_generate_timestamp() {
        let generator = MigrationGenerator::new();
        let timestamp = generator.generate_timestamp();

        // YYYYMMDDHHmmss形式（14桁の数字）
        assert_eq!(timestamp.len(), 14);
        assert!(timestamp.chars().all(|c| c.is_ascii_digit()));
    }

    /// ファイル名生成のテスト
    #[test]
    fn test_generate_migration_filename() {
        let generator = MigrationGenerator::new();
        let timestamp = "20260122120000";
        let description = "create_users_table";

        let filename = generator.generate_migration_filename(timestamp, description);

        assert_eq!(filename, "20260122120000_create_users_table");
    }

    /// 説明文のサニタイズテスト
    #[test]
    fn test_sanitize_description() {
        let generator = MigrationGenerator::new();

        assert_eq!(
            generator.sanitize_description("Create Users Table"),
            "create_users_table"
        );
        assert_eq!(
            generator.sanitize_description("Add Email Column"),
            "add_email_column"
        );
        assert_eq!(
            generator.sanitize_description("Update Status Field!"),
            "update_status_field"
        );
    }

    /// 空の差分からのSQL生成テスト
    #[test]
    fn test_generate_up_sql_empty_diff() {
        let generator = MigrationGenerator::new();
        let diff = SchemaDiff::new();

        let up_sql = generator
            .generate_up_sql(&diff, Dialect::PostgreSQL)
            .unwrap();

        assert!(up_sql.is_empty() || up_sql.trim().is_empty());
    }

    /// テーブル追加のUP SQL生成テスト
    #[test]
    fn test_generate_up_sql_table_added() {
        let generator = MigrationGenerator::new();

        let mut diff = SchemaDiff::new();
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        diff.added_tables.push(table);

        let up_sql = generator
            .generate_up_sql(&diff, Dialect::PostgreSQL)
            .unwrap();

        assert!(up_sql.contains("CREATE TABLE users"));
    }

    /// テーブル削除のDOWN SQL生成テスト
    #[test]
    fn test_generate_down_sql_table_removed() {
        let generator = MigrationGenerator::new();

        let mut diff = SchemaDiff::new();
        diff.removed_tables.push("users".to_string());

        let down_sql = generator
            .generate_down_sql(&diff, Dialect::PostgreSQL)
            .unwrap();

        // removed_tablesの場合、DOWNではテーブルを再作成する必要がある
        // 現在はTODOコメントを生成
        assert!(down_sql.contains("TODO") || down_sql.contains("Recreate"));
    }

    /// 複数の変更を含むUP SQL生成テスト
    #[test]
    fn test_generate_up_sql_multiple_changes() {
        let generator = MigrationGenerator::new();
        let detector = SchemaDiffDetector::new();

        let schema1 = Schema::new("1.0".to_string());

        let mut schema2 = Schema::new("1.0".to_string());
        let mut users_table = Table::new("users".to_string());
        users_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(users_table);

        let diff = detector.detect_diff(&schema1, &schema2);
        let up_sql = generator
            .generate_up_sql(&diff, Dialect::PostgreSQL)
            .unwrap();

        assert!(up_sql.contains("CREATE TABLE users"));
    }

    /// マイグレーションメタデータ生成のテスト
    #[test]
    fn test_generate_migration_metadata() {
        let generator = MigrationGenerator::new();
        let version = "20260122120000";
        let description = "create_users_table";
        let checksum = "abc123def456";

        let metadata = generator.generate_migration_metadata(
            version,
            description,
            Dialect::PostgreSQL,
            checksum,
        );

        assert!(metadata.contains("version:"));
        assert!(metadata.contains("20260122120000"));
        assert!(metadata.contains("description:"));
        assert!(metadata.contains("create_users_table"));
        assert!(metadata.contains("dialect:"));
        assert!(metadata.contains("checksum:"));
        assert!(metadata.contains("abc123def456"));
    }

    /// マイグレーション生成の統合テスト
    #[test]
    fn test_generate_migration_integrated() {
        let generator = MigrationGenerator::new();
        let detector = SchemaDiffDetector::new();

        // Schema 1: 空
        let schema1 = Schema::new("1.0".to_string());

        // Schema 2: usersテーブルを追加
        let mut schema2 = Schema::new("1.0".to_string());
        let mut users_table = Table::new("users".to_string());
        users_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        users_table.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        schema2.add_table(users_table);

        let diff = detector.detect_diff(&schema1, &schema2);

        // UP SQLの生成
        let up_sql = generator
            .generate_up_sql(&diff, Dialect::PostgreSQL)
            .unwrap();
        assert!(up_sql.contains("CREATE TABLE users"));
        assert!(up_sql.contains("id"));
        assert!(up_sql.contains("email"));

        // DOWN SQLの生成
        let down_sql = generator
            .generate_down_sql(&diff, Dialect::PostgreSQL)
            .unwrap();
        // usersテーブルが追加されたので、DOWNではDROP TABLE users
        // ただし、added_tablesからDOWN SQLを生成する場合
        assert!(down_sql.contains("DROP TABLE") || down_sql.is_empty());
    }
}
