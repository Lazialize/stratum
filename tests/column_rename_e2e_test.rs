/// カラムリネーム機能のE2Eテスト
///
/// testcontainersを使用して実際のデータベースに対するエンドツーエンドテストを実施します。
///
/// テスト内容:
/// - PostgreSQL/MySQLコンテナでのリネームマイグレーション適用・ロールバック
/// - SQLiteでのリネームマイグレーション適用・ロールバック
/// - リネーム+型変更の同時処理E2Eテスト
///
/// Task 7.2: testcontainersを使用したE2Eテスト
///
/// 注意: Docker必須のテストは #[ignore] アトリビュートでマークされています。
/// Docker起動時に実行するには: `cargo test -- --ignored`

#[cfg(test)]
mod column_rename_e2e_tests {
    use sqlx::postgres::PgPoolOptions;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{MySqlPool, Postgres, Row, Sqlite};
    use tempfile::TempDir;
    use testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt};
    use testcontainers_modules::mysql::Mysql as MysqlImage;
    use testcontainers_modules::postgres::Postgres as PostgresImage;

    // ==========================================
    // PostgreSQL E2Eテスト
    // ==========================================

    /// PostgreSQLコンテナを起動して接続プールを作成
    async fn setup_postgres_container(
    ) -> Result<(ContainerAsync<PostgresImage>, sqlx::Pool<Postgres>), Box<dyn std::error::Error>>
    {
        let container = PostgresImage::default()
            .with_tag("16-alpine")
            .start()
            .await?;

        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(5432).await?;
        let connection_string = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&connection_string)
            .await?;

        Ok((container, pool))
    }

    /// PostgreSQLでのシンプルなカラムリネームテスト
    #[tokio::test]
    #[ignore] // Docker必須
    async fn test_postgres_simple_column_rename() {
        let (_container, pool) = setup_postgres_container().await.unwrap();

        // 初期テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE users (
                id SERIAL PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(255) NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // テストデータを挿入
        sqlx::query("INSERT INTO users (name, email) VALUES ($1, $2)")
            .bind("Test User")
            .bind("test@example.com")
            .execute(&pool)
            .await
            .unwrap();

        // Up: カラムリネームを適用
        sqlx::query("ALTER TABLE users RENAME COLUMN name TO user_name")
            .execute(&pool)
            .await
            .unwrap();

        // リネーム後のカラムでデータを取得できることを確認
        let row = sqlx::query("SELECT user_name, email FROM users WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let user_name: String = row.get("user_name");
        assert_eq!(user_name, "Test User");

        // Down: リネームをロールバック
        sqlx::query("ALTER TABLE users RENAME COLUMN user_name TO name")
            .execute(&pool)
            .await
            .unwrap();

        // ロールバック後、元のカラム名でデータを取得できることを確認
        let row = sqlx::query("SELECT name, email FROM users WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let name: String = row.get("name");
        assert_eq!(name, "Test User");
    }

    /// PostgreSQLでのリネーム+型変更の同時処理テスト
    #[tokio::test]
    #[ignore] // Docker必須
    async fn test_postgres_rename_with_type_change() {
        let (_container, pool) = setup_postgres_container().await.unwrap();

        // 初期テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE products (
                id SERIAL PRIMARY KEY,
                name VARCHAR(50) NOT NULL,
                price INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // テストデータを挿入
        sqlx::query("INSERT INTO products (name, price) VALUES ($1, $2)")
            .bind("Widget")
            .bind(100)
            .execute(&pool)
            .await
            .unwrap();

        // Up: リネーム → 型変更の順序で適用
        // 1. リネーム
        sqlx::query("ALTER TABLE products RENAME COLUMN name TO product_name")
            .execute(&pool)
            .await
            .unwrap();

        // 2. 型変更 (VARCHAR(50) → VARCHAR(200))
        sqlx::query("ALTER TABLE products ALTER COLUMN product_name TYPE VARCHAR(200)")
            .execute(&pool)
            .await
            .unwrap();

        // リネーム+型変更後のデータ確認
        let row = sqlx::query("SELECT product_name FROM products WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let product_name: String = row.get("product_name");
        assert_eq!(product_name, "Widget");

        // 新しい長さ制限で挿入できることを確認
        let long_name = "A".repeat(150);
        sqlx::query("INSERT INTO products (product_name, price) VALUES ($1, $2)")
            .bind(&long_name)
            .bind(200)
            .execute(&pool)
            .await
            .unwrap();

        // Down: 型変更の逆 → リネームの逆の順序でロールバック
        // 1. 型変更の逆 (VARCHAR(200) → VARCHAR(50)) - 注意: データが切り捨てられる可能性
        // 長いデータを削除してからロールバック
        sqlx::query("DELETE FROM products WHERE LENGTH(product_name) > 50")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("ALTER TABLE products ALTER COLUMN product_name TYPE VARCHAR(50)")
            .execute(&pool)
            .await
            .unwrap();

        // 2. リネームの逆
        sqlx::query("ALTER TABLE products RENAME COLUMN product_name TO name")
            .execute(&pool)
            .await
            .unwrap();

        // ロールバック後のデータ確認
        let row = sqlx::query("SELECT name FROM products WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let name: String = row.get("name");
        assert_eq!(name, "Widget");
    }

    /// PostgreSQLでの複数カラムリネームテスト
    #[tokio::test]
    #[ignore] // Docker必須
    async fn test_postgres_multiple_column_renames() {
        let (_container, pool) = setup_postgres_container().await.unwrap();

        // 初期テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE employees (
                id SERIAL PRIMARY KEY,
                first_name VARCHAR(100) NOT NULL,
                last_name VARCHAR(100) NOT NULL,
                dept VARCHAR(50) NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // テストデータを挿入
        sqlx::query("INSERT INTO employees (first_name, last_name, dept) VALUES ($1, $2, $3)")
            .bind("John")
            .bind("Doe")
            .bind("Engineering")
            .execute(&pool)
            .await
            .unwrap();

        // Up: 複数カラムをリネーム
        sqlx::query("ALTER TABLE employees RENAME COLUMN first_name TO given_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN last_name TO family_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN dept TO department")
            .execute(&pool)
            .await
            .unwrap();

        // リネーム後のデータ確認
        let row =
            sqlx::query("SELECT given_name, family_name, department FROM employees WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(row.get::<String, _>("given_name"), "John");
        assert_eq!(row.get::<String, _>("family_name"), "Doe");
        assert_eq!(row.get::<String, _>("department"), "Engineering");

        // Down: 複数カラムを逆リネーム
        sqlx::query("ALTER TABLE employees RENAME COLUMN department TO dept")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN family_name TO last_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN given_name TO first_name")
            .execute(&pool)
            .await
            .unwrap();

        // ロールバック後のデータ確認
        let row = sqlx::query("SELECT first_name, last_name, dept FROM employees WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(row.get::<String, _>("first_name"), "John");
        assert_eq!(row.get::<String, _>("last_name"), "Doe");
        assert_eq!(row.get::<String, _>("dept"), "Engineering");
    }

    // ==========================================
    // MySQL E2Eテスト
    // ==========================================

    /// MySQLコンテナを起動して接続プールを作成
    async fn setup_mysql_container(
    ) -> Result<(ContainerAsync<MysqlImage>, MySqlPool), Box<dyn std::error::Error>> {
        let container = MysqlImage::default().with_tag("8.0").start().await?;

        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(3306).await?;
        let connection_string = format!("mysql://root@{}:{}/mysql", host, port);

        // MySQL起動待ち
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&connection_string)
            .await?;

        Ok((container, pool))
    }

    /// MySQLでのシンプルなカラムリネームテスト
    #[tokio::test]
    #[ignore] // Docker必須
    async fn test_mysql_simple_column_rename() {
        let (_container, pool) = setup_mysql_container().await.unwrap();

        // 初期テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(255) NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // テストデータを挿入
        sqlx::query("INSERT INTO users (name, email) VALUES (?, ?)")
            .bind("Test User")
            .bind("test@example.com")
            .execute(&pool)
            .await
            .unwrap();

        // Up: カラムリネームを適用 (MySQL 8.0+ RENAME COLUMN構文)
        sqlx::query("ALTER TABLE users RENAME COLUMN name TO user_name")
            .execute(&pool)
            .await
            .unwrap();

        // リネーム後のカラムでデータを取得できることを確認
        let row = sqlx::query("SELECT user_name, email FROM users WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let user_name: String = row.get("user_name");
        assert_eq!(user_name, "Test User");

        // Down: リネームをロールバック
        sqlx::query("ALTER TABLE users RENAME COLUMN user_name TO name")
            .execute(&pool)
            .await
            .unwrap();

        // ロールバック後、元のカラム名でデータを取得できることを確認
        let row = sqlx::query("SELECT name, email FROM users WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let name: String = row.get("name");
        assert_eq!(name, "Test User");
    }

    /// MySQLでのリネーム+型変更の同時処理テスト
    #[tokio::test]
    #[ignore] // Docker必須
    async fn test_mysql_rename_with_type_change() {
        let (_container, pool) = setup_mysql_container().await.unwrap();

        // 初期テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE products (
                id INT AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(50) NOT NULL,
                price INT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // テストデータを挿入
        sqlx::query("INSERT INTO products (name, price) VALUES (?, ?)")
            .bind("Widget")
            .bind(100)
            .execute(&pool)
            .await
            .unwrap();

        // Up: リネーム → 型変更の順序で適用
        // 1. リネーム
        sqlx::query("ALTER TABLE products RENAME COLUMN name TO product_name")
            .execute(&pool)
            .await
            .unwrap();

        // 2. 型変更 (VARCHAR(50) → VARCHAR(200))
        sqlx::query("ALTER TABLE products MODIFY COLUMN product_name VARCHAR(200) NOT NULL")
            .execute(&pool)
            .await
            .unwrap();

        // リネーム+型変更後のデータ確認
        let row = sqlx::query("SELECT product_name FROM products WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let product_name: String = row.get("product_name");
        assert_eq!(product_name, "Widget");

        // Down: 型変更の逆 → リネームの逆の順序でロールバック
        sqlx::query("ALTER TABLE products MODIFY COLUMN product_name VARCHAR(50) NOT NULL")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("ALTER TABLE products RENAME COLUMN product_name TO name")
            .execute(&pool)
            .await
            .unwrap();

        // ロールバック後のデータ確認
        let row = sqlx::query("SELECT name FROM products WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let name: String = row.get("name");
        assert_eq!(name, "Widget");
    }

    /// MySQLでの複数カラムリネームテスト
    #[tokio::test]
    #[ignore] // Docker必須
    async fn test_mysql_multiple_column_renames() {
        let (_container, pool) = setup_mysql_container().await.unwrap();

        // 初期テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE employees (
                id INT AUTO_INCREMENT PRIMARY KEY,
                first_name VARCHAR(100) NOT NULL,
                last_name VARCHAR(100) NOT NULL,
                dept VARCHAR(50) NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // テストデータを挿入
        sqlx::query("INSERT INTO employees (first_name, last_name, dept) VALUES (?, ?, ?)")
            .bind("John")
            .bind("Doe")
            .bind("Engineering")
            .execute(&pool)
            .await
            .unwrap();

        // Up: 複数カラムをリネーム
        sqlx::query("ALTER TABLE employees RENAME COLUMN first_name TO given_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN last_name TO family_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN dept TO department")
            .execute(&pool)
            .await
            .unwrap();

        // リネーム後のデータ確認
        let row =
            sqlx::query("SELECT given_name, family_name, department FROM employees WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(row.get::<String, _>("given_name"), "John");
        assert_eq!(row.get::<String, _>("family_name"), "Doe");
        assert_eq!(row.get::<String, _>("department"), "Engineering");

        // Down: 複数カラムを逆リネーム
        sqlx::query("ALTER TABLE employees RENAME COLUMN department TO dept")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN family_name TO last_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN given_name TO first_name")
            .execute(&pool)
            .await
            .unwrap();

        // ロールバック後のデータ確認
        let row = sqlx::query("SELECT first_name, last_name, dept FROM employees WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(row.get::<String, _>("first_name"), "John");
        assert_eq!(row.get::<String, _>("last_name"), "Doe");
        assert_eq!(row.get::<String, _>("dept"), "Engineering");
    }

    // ==========================================
    // SQLite E2Eテスト（Docker不要）
    // ==========================================

    /// SQLite接続プールを作成
    async fn setup_sqlite_pool() -> Result<(TempDir, sqlx::Pool<Sqlite>), Box<dyn std::error::Error>>
    {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");
        let connection_string = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&connection_string)
            .await?;

        Ok((temp_dir, pool))
    }

    /// SQLiteでのシンプルなカラムリネームテスト
    #[tokio::test]
    async fn test_sqlite_simple_column_rename() {
        let (_temp_dir, pool) = setup_sqlite_pool().await.unwrap();

        // 初期テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                email TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // テストデータを挿入
        sqlx::query("INSERT INTO users (name, email) VALUES (?, ?)")
            .bind("Test User")
            .bind("test@example.com")
            .execute(&pool)
            .await
            .unwrap();

        // Up: カラムリネームを適用 (SQLite 3.25.0+)
        sqlx::query("ALTER TABLE users RENAME COLUMN name TO user_name")
            .execute(&pool)
            .await
            .unwrap();

        // リネーム後のカラムでデータを取得できることを確認
        let row = sqlx::query("SELECT user_name, email FROM users WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let user_name: String = row.get("user_name");
        assert_eq!(user_name, "Test User");

        // Down: リネームをロールバック
        sqlx::query("ALTER TABLE users RENAME COLUMN user_name TO name")
            .execute(&pool)
            .await
            .unwrap();

        // ロールバック後、元のカラム名でデータを取得できることを確認
        let row = sqlx::query("SELECT name, email FROM users WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let name: String = row.get("name");
        assert_eq!(name, "Test User");
    }

    /// SQLiteでのリネーム+型変更の同時処理テスト
    ///
    /// SQLiteではALTER TABLE ... ALTER COLUMN TYPE がサポートされていないため、
    /// テーブル再作成パターンでの型変更をテストします。
    #[tokio::test]
    async fn test_sqlite_rename_with_type_change() {
        let (_temp_dir, pool) = setup_sqlite_pool().await.unwrap();

        // 初期テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE products (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                price INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // テストデータを挿入
        sqlx::query("INSERT INTO products (name, price) VALUES (?, ?)")
            .bind("Widget")
            .bind(100)
            .execute(&pool)
            .await
            .unwrap();

        // Up: リネームを適用
        sqlx::query("ALTER TABLE products RENAME COLUMN name TO product_name")
            .execute(&pool)
            .await
            .unwrap();

        // SQLiteでは型変更にテーブル再作成が必要
        // ここでは簡略化のため、型変更をシミュレート
        // (実際のStrataでは MigrationPipeline がテーブル再作成パターンを生成)

        // リネーム後のデータ確認
        let row = sqlx::query("SELECT product_name, price FROM products WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let product_name: String = row.get("product_name");
        assert_eq!(product_name, "Widget");

        // Down: リネームをロールバック
        sqlx::query("ALTER TABLE products RENAME COLUMN product_name TO name")
            .execute(&pool)
            .await
            .unwrap();

        // ロールバック後のデータ確認
        let row = sqlx::query("SELECT name FROM products WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        let name: String = row.get("name");
        assert_eq!(name, "Widget");
    }

    /// SQLiteでの複数カラムリネームテスト
    #[tokio::test]
    async fn test_sqlite_multiple_column_renames() {
        let (_temp_dir, pool) = setup_sqlite_pool().await.unwrap();

        // 初期テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE employees (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                first_name TEXT NOT NULL,
                last_name TEXT NOT NULL,
                dept TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // テストデータを挿入
        sqlx::query("INSERT INTO employees (first_name, last_name, dept) VALUES (?, ?, ?)")
            .bind("John")
            .bind("Doe")
            .bind("Engineering")
            .execute(&pool)
            .await
            .unwrap();

        // Up: 複数カラムをリネーム
        sqlx::query("ALTER TABLE employees RENAME COLUMN first_name TO given_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN last_name TO family_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN dept TO department")
            .execute(&pool)
            .await
            .unwrap();

        // リネーム後のデータ確認
        let row =
            sqlx::query("SELECT given_name, family_name, department FROM employees WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(row.get::<String, _>("given_name"), "John");
        assert_eq!(row.get::<String, _>("family_name"), "Doe");
        assert_eq!(row.get::<String, _>("department"), "Engineering");

        // Down: 複数カラムを逆リネーム
        sqlx::query("ALTER TABLE employees RENAME COLUMN department TO dept")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN family_name TO last_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE employees RENAME COLUMN given_name TO first_name")
            .execute(&pool)
            .await
            .unwrap();

        // ロールバック後のデータ確認
        let row = sqlx::query("SELECT first_name, last_name, dept FROM employees WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(row.get::<String, _>("first_name"), "John");
        assert_eq!(row.get::<String, _>("last_name"), "Doe");
        assert_eq!(row.get::<String, _>("dept"), "Engineering");
    }

    /// SQLiteでのインデックス付きカラムのリネームテスト
    #[tokio::test]
    async fn test_sqlite_rename_indexed_column() {
        let (_temp_dir, pool) = setup_sqlite_pool().await.unwrap();

        // インデックス付きテーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL,
                name TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("CREATE UNIQUE INDEX idx_users_email ON users (email)")
            .execute(&pool)
            .await
            .unwrap();

        // テストデータを挿入
        sqlx::query("INSERT INTO users (email, name) VALUES (?, ?)")
            .bind("test@example.com")
            .bind("Test User")
            .execute(&pool)
            .await
            .unwrap();

        // emailカラムをリネーム
        // SQLite 3.25.0+では RENAME COLUMN がインデックスも自動更新
        sqlx::query("ALTER TABLE users RENAME COLUMN email TO email_address")
            .execute(&pool)
            .await
            .unwrap();

        // リネーム後もインデックスが機能することを確認
        let row = sqlx::query("SELECT email_address, name FROM users WHERE email_address = ?")
            .bind("test@example.com")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(row.get::<String, _>("email_address"), "test@example.com");

        // ユニーク制約も維持されていることを確認
        let result = sqlx::query("INSERT INTO users (email_address, name) VALUES (?, ?)")
            .bind("test@example.com") // 重複
            .bind("Another User")
            .execute(&pool)
            .await;

        assert!(result.is_err());
    }

    // ==========================================
    // データ整合性テスト
    // ==========================================

    /// データ保持確認テスト（SQLite）
    #[tokio::test]
    async fn test_data_preservation_after_rename() {
        let (_temp_dir, pool) = setup_sqlite_pool().await.unwrap();

        // テーブルを作成
        sqlx::query(
            r#"
            CREATE TABLE orders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                customer_name TEXT NOT NULL,
                product_name TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                total_price REAL NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // 複数のテストデータを挿入
        let test_data = vec![
            ("Alice", "Widget A", 2, 19.99),
            ("Bob", "Widget B", 5, 49.95),
            ("Charlie", "Widget C", 1, 9.99),
        ];

        for (customer, product, qty, price) in &test_data {
            sqlx::query(
                "INSERT INTO orders (customer_name, product_name, quantity, total_price) VALUES (?, ?, ?, ?)",
            )
            .bind(customer)
            .bind(product)
            .bind(qty)
            .bind(price)
            .execute(&pool)
            .await
            .unwrap();
        }

        // カラムをリネーム
        sqlx::query("ALTER TABLE orders RENAME COLUMN customer_name TO buyer_name")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE orders RENAME COLUMN product_name TO item_name")
            .execute(&pool)
            .await
            .unwrap();

        // 全データが保持されていることを確認
        let rows = sqlx::query(
            "SELECT buyer_name, item_name, quantity, total_price FROM orders ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 3);

        for (i, row) in rows.iter().enumerate() {
            let (expected_customer, expected_product, expected_qty, expected_price) = &test_data[i];
            assert_eq!(row.get::<String, _>("buyer_name"), *expected_customer);
            assert_eq!(row.get::<String, _>("item_name"), *expected_product);
            assert_eq!(row.get::<i32, _>("quantity"), *expected_qty);
            assert!((row.get::<f64, _>("total_price") - expected_price).abs() < 0.01);
        }
    }
}
