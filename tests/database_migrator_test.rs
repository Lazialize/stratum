/// データベースマイグレーターサービスのテスト
///
/// マイグレーション履歴の管理とトランザクション制御が正しく動作することを確認します。

#[cfg(test)]
mod database_migrator_tests {
    use strata::adapters::database_migrator::DatabaseMigratorService;
    use strata::core::config::Dialect;
    use strata::core::migration::Migration;

    /// サービスの作成テスト
    #[test]
    fn test_new_service() {
        let service = DatabaseMigratorService::new();
        assert!(format!("{:?}", service).contains("DatabaseMigratorService"));
    }

    /// PostgreSQL用のマイグレーションテーブル作成SQL生成テスト
    #[test]
    fn test_generate_create_migration_table_sql_postgres() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_create_migration_table_sql(Dialect::PostgreSQL);

        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("schema_migrations"));
        assert!(sql.contains("version"));
        assert!(sql.contains("applied_at"));
        assert!(sql.contains("checksum"));
        assert!(sql.contains("IF NOT EXISTS"));
    }

    /// MySQL用のマイグレーションテーブル作成SQL生成テスト
    #[test]
    fn test_generate_create_migration_table_sql_mysql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_create_migration_table_sql(Dialect::MySQL);

        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("schema_migrations"));
        assert!(sql.contains("version"));
        assert!(sql.contains("applied_at"));
        assert!(sql.contains("checksum"));
        assert!(sql.contains("IF NOT EXISTS"));
    }

    /// SQLite用のマイグレーションテーブル作成SQL生成テスト
    #[test]
    fn test_generate_create_migration_table_sql_sqlite() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_create_migration_table_sql(Dialect::SQLite);

        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("schema_migrations"));
        assert!(sql.contains("version"));
        assert!(sql.contains("applied_at"));
        assert!(sql.contains("checksum"));
        assert!(sql.contains("IF NOT EXISTS"));
    }

    /// マイグレーション記録クエリ生成テスト（パラメータバインド対応）
    #[test]
    fn test_generate_record_migration_query() {
        let service = DatabaseMigratorService::new();
        let migration = Migration::new(
            "20240101120000".to_string(),
            "create_users_table".to_string(),
            "abc123def456".to_string(),
        );

        let (sql, params) =
            service.generate_record_migration_query(&migration, Dialect::PostgreSQL);

        assert!(sql.contains("INSERT INTO schema_migrations"));
        assert!(sql.contains("$1"));
        assert!(!sql.contains("20240101120000")); // 値が直接埋め込まれていない
        assert_eq!(params.len(), 4);
        assert_eq!(params[0], "20240101120000");
        assert_eq!(params[1], "create_users_table");
    }

    /// マイグレーション削除クエリ生成テスト（パラメータバインド対応）
    #[test]
    fn test_generate_remove_migration_query() {
        let service = DatabaseMigratorService::new();
        let version = "20240101_120000";

        let (sql, params) = service.generate_remove_migration_query(version, Dialect::PostgreSQL);

        assert!(sql.contains("DELETE FROM schema_migrations"));
        assert!(sql.contains("WHERE version ="));
        assert!(sql.contains("$1"));
        assert!(!sql.contains("20240101_120000")); // 値が直接埋め込まれていない
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "20240101_120000");
    }

    /// マイグレーション履歴取得のSELECT SQL生成テスト
    #[test]
    fn test_generate_get_migrations_sql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_get_migrations_sql(Dialect::PostgreSQL);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM schema_migrations"));
        assert!(sql.contains("ORDER BY"));
        assert!(sql.contains("version"));
    }

    /// トランザクション開始SQL生成テスト
    #[test]
    fn test_generate_begin_transaction_sql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_begin_transaction_sql();

        assert_eq!(sql, "BEGIN");
    }

    /// トランザクションコミットSQL生成テスト
    #[test]
    fn test_generate_commit_transaction_sql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_commit_transaction_sql();

        assert_eq!(sql, "COMMIT");
    }

    /// トランザクションロールバックSQL生成テスト
    #[test]
    fn test_generate_rollback_transaction_sql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_rollback_transaction_sql();

        assert_eq!(sql, "ROLLBACK");
    }

    /// 特定マイグレーションの取得クエリ生成テスト（パラメータバインド対応）
    #[test]
    fn test_generate_get_migration_by_version_query() {
        let service = DatabaseMigratorService::new();
        let version = "20240101_120000";

        let (sql, params) =
            service.generate_get_migration_by_version_query(Dialect::PostgreSQL, version);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM schema_migrations"));
        assert!(sql.contains("WHERE version ="));
        assert!(sql.contains("$1"));
        assert!(!sql.contains("20240101_120000")); // 値が直接埋め込まれていない
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "20240101_120000");
    }

    /// マイグレーションテーブル存在確認SQL生成テスト（PostgreSQL）
    #[test]
    fn test_generate_check_migration_table_exists_sql_postgres() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_check_migration_table_exists_sql(Dialect::PostgreSQL);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("information_schema.tables"));
        assert!(sql.contains("table_name"));
        assert!(sql.contains("schema_migrations"));
    }

    /// マイグレーションテーブル存在確認SQL生成テスト（MySQL）
    #[test]
    fn test_generate_check_migration_table_exists_sql_mysql() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_check_migration_table_exists_sql(Dialect::MySQL);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("information_schema.tables"));
        assert!(sql.contains("table_name"));
        assert!(sql.contains("schema_migrations"));
    }

    /// マイグレーションテーブル存在確認SQL生成テスト（SQLite）
    #[test]
    fn test_generate_check_migration_table_exists_sql_sqlite() {
        let service = DatabaseMigratorService::new();
        let sql = service.generate_check_migration_table_exists_sql(Dialect::SQLite);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("sqlite_master"));
        assert!(sql.contains("type = 'table'"));
        assert!(sql.contains("name"));
        assert!(sql.contains("schema_migrations"));
    }
}
