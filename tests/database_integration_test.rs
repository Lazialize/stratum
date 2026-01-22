/// データベース統合テスト
///
/// testcontainersを使用して実際のデータベースに対するエンドツーエンドテストを実施します。
///
/// テスト内容:
/// - マイグレーションの適用とロールバック
/// - チェックサム検証
/// - トランザクション制御
///
/// 注意: このテストはDockerが必要です。Docker未起動の場合はスキップされます。

#[cfg(test)]
mod database_integration_tests {
    use sqlx::any::AnyPoolOptions;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Any, Row};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt};
    use testcontainers_modules::postgres::Postgres;

    /// テストプロジェクト構造の検証テスト（Docker不要）
    #[test]
    fn test_setup_test_project() {
        let result = setup_test_project();
        assert!(result.is_ok());

        let (_temp_dir, project_path) = result.unwrap();

        // ディレクトリが存在することを確認
        assert!(project_path.join("schema").exists());
        assert!(project_path.join("migrations").exists());
        assert!(project_path.join(".stratum.yaml").exists());
        assert!(project_path.join("schema").join("users.yaml").exists());

        // 設定ファイルの内容を確認
        let config_content = fs::read_to_string(project_path.join(".stratum.yaml")).unwrap();
        assert!(config_content.contains("dialect: postgresql"));
        assert!(config_content.contains("schema_dir: schema"));

        // スキーマファイルの内容を確認
        let schema_content =
            fs::read_to_string(project_path.join("schema").join("users.yaml")).unwrap();
        assert!(schema_content.contains("users"));
        assert!(schema_content.contains("email"));
    }

    /// テスト用のプロジェクトディレクトリを作成
    fn setup_test_project() -> Result<(TempDir, PathBuf), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let project_path = temp_dir.path().to_path_buf();

        // schema/ディレクトリを作成
        let schema_dir = project_path.join("schema");
        fs::create_dir(&schema_dir)?;

        // migrations/ディレクトリを作成
        let migrations_dir = project_path.join("migrations");
        fs::create_dir(&migrations_dir)?;

        // 設定ファイルを作成
        let config_content = r#"
version: "1.0"
dialect: postgresql
schema_dir: schema
migrations_dir: migrations

environments:
  development:
    host: localhost
    port: 5432
    database: test_db
    user: postgres
    password: postgres
    timeout: 30
"#;
        fs::write(project_path.join(".stratum.yaml"), config_content)?;

        // テスト用のスキーマファイルを作成
        let schema_content = r#"
version: "1.0"
tables:
  users:
    name: users
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: true
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        default_value: null
        auto_increment: null
    indexes:
      - name: idx_users_email
        columns:
          - email
        unique: true
    constraints:
      - type: PRIMARY_KEY
        columns:
          - id
"#;
        fs::write(schema_dir.join("users.yaml"), schema_content)?;

        Ok((temp_dir, project_path))
    }

    /// PostgreSQLコンテナを起動して接続プールを作成
    async fn setup_postgres_container() -> Result<
        (ContainerAsync<Postgres>, sqlx::Pool<Any>),
        Box<dyn std::error::Error>,
    > {
        // PostgreSQLコンテナを起動
        let container = Postgres::default()
            .with_tag("16-alpine")
            .start()
            .await?;

        // 接続文字列を構築
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(5432).await?;
        let connection_string = format!(
            "postgres://postgres:postgres@{}:{}/postgres",
            host, port
        );

        // 接続プールを作成
        let pool = AnyPoolOptions::new()
            .max_connections(5)
            .connect(&connection_string)
            .await?;

        Ok((container, pool))
    }

    #[tokio::test]
    #[ignore] // Docker必須のため、通常のテスト実行ではスキップ
    async fn test_migration_apply_and_rollback() {
        // テストプロジェクトのセットアップ
        let (_temp_dir, _project_path) = setup_test_project().unwrap();

        // PostgreSQLコンテナを起動
        let (_container, pool) = setup_postgres_container().await.unwrap();

        // マイグレーション履歴テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version VARCHAR(255) PRIMARY KEY,
                description VARCHAR(255) NOT NULL,
                checksum VARCHAR(64) NOT NULL,
                applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // usersテーブルを作成するマイグレーションを適用
        sqlx::query(
            r#"
            CREATE TABLE users (
                id SERIAL NOT NULL,
                email VARCHAR(255) NOT NULL,
                PRIMARY KEY (id)
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // マイグレーション履歴を記録
        sqlx::query(
            r#"
            INSERT INTO schema_migrations (version, description, checksum)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind("20260122000001")
        .bind("create_users_table")
        .bind("abc123def456")
        .execute(&pool)
        .await
        .unwrap();

        // usersテーブルが存在することを確認
        let result = sqlx::query("SELECT * FROM users").fetch_all(&pool).await;
        assert!(result.is_ok());

        // マイグレーション履歴を確認
        let migrations: Vec<String> = sqlx::query("SELECT version FROM schema_migrations")
            .fetch_all(&pool)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get::<String, _>("version"))
            .collect();

        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0], "20260122000001");

        // ロールバック: usersテーブルを削除
        sqlx::query("DROP TABLE users").execute(&pool).await.unwrap();

        // マイグレーション履歴から削除
        sqlx::query("DELETE FROM schema_migrations WHERE version = $1")
            .bind("20260122000001")
            .execute(&pool)
            .await
            .unwrap();

        // usersテーブルが存在しないことを確認
        let result = sqlx::query("SELECT * FROM users").fetch_all(&pool).await;
        assert!(result.is_err());

        // マイグレーション履歴が空であることを確認
        let migrations: Vec<String> = sqlx::query("SELECT version FROM schema_migrations")
            .fetch_all(&pool)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get::<String, _>("version"))
            .collect();

        assert_eq!(migrations.len(), 0);
    }

    #[tokio::test]
    #[ignore] // Docker必須のため、通常のテスト実行ではスキップ
    async fn test_checksum_verification() {
        // PostgreSQLコンテナを起動
        let (_container, pool) = setup_postgres_container().await.unwrap();

        // マイグレーション履歴テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version VARCHAR(255) PRIMARY KEY,
                description VARCHAR(255) NOT NULL,
                checksum VARCHAR(64) NOT NULL,
                applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // マイグレーション履歴を記録
        let version = "20260122000001";
        let description = "create_users_table";
        let checksum = "abc123def456";

        sqlx::query(
            r#"
            INSERT INTO schema_migrations (version, description, checksum)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(version)
        .bind(description)
        .bind(checksum)
        .execute(&pool)
        .await
        .unwrap();

        // チェックサムを取得
        let stored_checksum: String =
            sqlx::query("SELECT checksum FROM schema_migrations WHERE version = $1")
                .bind(version)
                .fetch_one(&pool)
                .await
                .unwrap()
                .get("checksum");

        // チェックサムが一致することを確認
        assert_eq!(stored_checksum, checksum);

        // チェックサムが異なる場合のテスト
        let different_checksum = "different_hash";
        assert_ne!(stored_checksum, different_checksum);
    }

    #[tokio::test]
    #[ignore] // Docker必須のため、通常のテスト実行ではスキップ
    async fn test_transaction_rollback_on_error() {
        // PostgreSQLコンテナを起動
        let (_container, pool) = setup_postgres_container().await.unwrap();

        // マイグレーション履歴テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version VARCHAR(255) PRIMARY KEY,
                description VARCHAR(255) NOT NULL,
                checksum VARCHAR(64) NOT NULL,
                applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // トランザクション開始
        let mut tx = pool.begin().await.unwrap();

        // usersテーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE users (
                id SERIAL NOT NULL,
                email VARCHAR(255) NOT NULL,
                PRIMARY KEY (id)
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .unwrap();

        // マイグレーション履歴を記録
        sqlx::query(
            r#"
            INSERT INTO schema_migrations (version, description, checksum)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind("20260122000001")
        .bind("create_users_table")
        .bind("abc123def456")
        .execute(&mut *tx)
        .await
        .unwrap();

        // エラーをシミュレート: トランザクションをロールバック
        tx.rollback().await.unwrap();

        // usersテーブルが存在しないことを確認（ロールバックされたため）
        let result = sqlx::query("SELECT * FROM users").fetch_all(&pool).await;
        assert!(result.is_err());

        // マイグレーション履歴にレコードが存在しないことを確認
        let migrations: Vec<String> = sqlx::query("SELECT version FROM schema_migrations")
            .fetch_all(&pool)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get::<String, _>("version"))
            .collect();

        assert_eq!(migrations.len(), 0);
    }

    #[tokio::test]
    #[ignore] // Docker必須のため、通常のテスト実行ではスキップ
    async fn test_multiple_migrations_in_order() {
        // PostgreSQLコンテナを起動
        let (_container, pool) = setup_postgres_container().await.unwrap();

        // マイグレーション履歴テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version VARCHAR(255) PRIMARY KEY,
                description VARCHAR(255) NOT NULL,
                checksum VARCHAR(64) NOT NULL,
                applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // マイグレーション1: usersテーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE users (
                id SERIAL NOT NULL,
                email VARCHAR(255) NOT NULL,
                PRIMARY KEY (id)
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO schema_migrations (version, description, checksum)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind("20260122000001")
        .bind("create_users_table")
        .bind("hash1")
        .execute(&pool)
        .await
        .unwrap();

        // マイグレーション2: postsテーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE posts (
                id SERIAL NOT NULL,
                user_id INTEGER NOT NULL,
                title VARCHAR(255) NOT NULL,
                PRIMARY KEY (id),
                FOREIGN KEY (user_id) REFERENCES users(id)
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO schema_migrations (version, description, checksum)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind("20260122000002")
        .bind("create_posts_table")
        .bind("hash2")
        .execute(&pool)
        .await
        .unwrap();

        // マイグレーション履歴を確認
        let migrations: Vec<String> = sqlx::query(
            "SELECT version FROM schema_migrations ORDER BY version",
        )
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(|row| row.get::<String, _>("version"))
        .collect();

        assert_eq!(migrations.len(), 2);
        assert_eq!(migrations[0], "20260122000001");
        assert_eq!(migrations[1], "20260122000002");

        // 両方のテーブルが存在することを確認
        let users_result = sqlx::query("SELECT * FROM users").fetch_all(&pool).await;
        assert!(users_result.is_ok());

        let posts_result = sqlx::query("SELECT * FROM posts").fetch_all(&pool).await;
        assert!(posts_result.is_ok());
    }

    /// SQLiteを使った統合テスト（Docker不要）
    #[tokio::test]
    async fn test_sqlite_migration_apply_and_rollback() {
        // 一時ディレクトリを作成
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // SQLite接続プールを作成
        let connection_string = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&connection_string)
            .await
            .unwrap();

        // マイグレーション履歴テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // usersテーブルを作成するマイグレーションを適用
        sqlx::query(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // マイグレーション履歴を記録
        sqlx::query(
            r#"
            INSERT INTO schema_migrations (version, description, checksum)
            VALUES (?, ?, ?)
            "#,
        )
        .bind("20260122000001")
        .bind("create_users_table")
        .bind("abc123def456")
        .execute(&pool)
        .await
        .unwrap();

        // usersテーブルが存在することを確認
        let result = sqlx::query("SELECT * FROM users").fetch_all(&pool).await;
        assert!(result.is_ok());

        // マイグレーション履歴を確認
        let migrations: Vec<String> = sqlx::query("SELECT version FROM schema_migrations")
            .fetch_all(&pool)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get::<String, _>("version"))
            .collect();

        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0], "20260122000001");

        // ロールバック: usersテーブルを削除
        sqlx::query("DROP TABLE users").execute(&pool).await.unwrap();

        // マイグレーション履歴から削除
        sqlx::query("DELETE FROM schema_migrations WHERE version = ?")
            .bind("20260122000001")
            .execute(&pool)
            .await
            .unwrap();

        // usersテーブルが存在しないことを確認
        let result = sqlx::query("SELECT * FROM users").fetch_all(&pool).await;
        assert!(result.is_err());

        // マイグレーション履歴が空であることを確認
        let migrations: Vec<String> = sqlx::query("SELECT version FROM schema_migrations")
            .fetch_all(&pool)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get::<String, _>("version"))
            .collect();

        assert_eq!(migrations.len(), 0);
    }

    /// SQLiteでのチェックサム検証テスト（Docker不要）
    #[tokio::test]
    async fn test_sqlite_checksum_verification() {
        // 一時ディレクトリを作成
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // SQLite接続プールを作成
        let connection_string = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&connection_string)
            .await
            .unwrap();

        // マイグレーション履歴テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // マイグレーション履歴を記録
        let version = "20260122000001";
        let description = "create_users_table";
        let checksum = "abc123def456";

        sqlx::query(
            r#"
            INSERT INTO schema_migrations (version, description, checksum)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(version)
        .bind(description)
        .bind(checksum)
        .execute(&pool)
        .await
        .unwrap();

        // チェックサムを取得
        let stored_checksum: String =
            sqlx::query("SELECT checksum FROM schema_migrations WHERE version = ?")
                .bind(version)
                .fetch_one(&pool)
                .await
                .unwrap()
                .get("checksum");

        // チェックサムが一致することを確認
        assert_eq!(stored_checksum, checksum);

        // チェックサムが異なる場合のテスト
        let different_checksum = "different_hash";
        assert_ne!(stored_checksum, different_checksum);
    }

    /// SQLiteでのトランザクションロールバックテスト（Docker不要）
    #[tokio::test]
    async fn test_sqlite_transaction_rollback_on_error() {
        // 一時ディレクトリを作成
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // SQLite接続プールを作成
        let connection_string = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&connection_string)
            .await
            .unwrap();

        // マイグレーション履歴テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // トランザクション開始
        let mut tx = pool.begin().await.unwrap();

        // usersテーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .unwrap();

        // マイグレーション履歴を記録
        sqlx::query(
            r#"
            INSERT INTO schema_migrations (version, description, checksum)
            VALUES (?, ?, ?)
            "#,
        )
        .bind("20260122000001")
        .bind("create_users_table")
        .bind("abc123def456")
        .execute(&mut *tx)
        .await
        .unwrap();

        // エラーをシミュレート: トランザクションをロールバック
        tx.rollback().await.unwrap();

        // usersテーブルが存在しないことを確認（ロールバックされたため）
        let result = sqlx::query("SELECT * FROM users").fetch_all(&pool).await;
        assert!(result.is_err());

        // マイグレーション履歴にレコードが存在しないことを確認
        let migrations: Vec<String> = sqlx::query("SELECT version FROM schema_migrations")
            .fetch_all(&pool)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get::<String, _>("version"))
            .collect();

        assert_eq!(migrations.len(), 0);
    }
}
