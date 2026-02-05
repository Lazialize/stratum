/// 方言固有カラム型の統合テスト
///
/// このテストは実際のデータベースに対して方言固有型を使用したテーブル作成を実行し、
/// SQL生成と実行が正しく動作することを検証します。
///
/// ## テスト環境
/// - Docker環境が必要（testcontainersを使用）
/// - PostgreSQL, MySQLコンテナを起動して実行
/// - 実行時は `cargo test -- --ignored` を使用
use sqlx::{Connection, MySqlConnection, PgConnection, Row};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mysql::Mysql;
use testcontainers_modules::postgres::Postgres;

/// PostgreSQL: SERIAL型を使用したテーブル作成の統合テスト
#[tokio::test]
#[ignore]
async fn test_postgres_serial_type_table_creation() {
    let container = Postgres::default()
        .start()
        .await
        .expect("Failed to start PostgreSQL container");

    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get container port");

    let connection_string = format!(
        "postgres://postgres:postgres@127.0.0.1:{}/postgres",
        host_port
    );

    let mut conn = PgConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to PostgreSQL");

    // SERIAL型を使用したテーブル作成SQL
    let create_table_sql = r#"
        CREATE TABLE test_serial_table (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL
        )
    "#;

    sqlx::query(create_table_sql)
        .execute(&mut conn)
        .await
        .expect("Failed to create table with SERIAL type");

    // テーブルが正しく作成されたことを確認
    let result = sqlx::query("SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'test_serial_table' ORDER BY ordinal_position")
        .fetch_all(&mut conn)
        .await
        .expect("Failed to query table schema");

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].get::<String, _>("column_name"), "id");
    assert_eq!(result[0].get::<String, _>("data_type"), "integer");
    assert_eq!(result[1].get::<String, _>("column_name"), "name");
    assert_eq!(result[1].get::<String, _>("data_type"), "character varying");
}

/// PostgreSQL: INET型を使用したテーブル作成の統合テスト
#[tokio::test]
#[ignore]
async fn test_postgres_inet_type_table_creation() {
    let container = Postgres::default()
        .start()
        .await
        .expect("Failed to start PostgreSQL container");

    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get container port");

    let connection_string = format!(
        "postgres://postgres:postgres@127.0.0.1:{}/postgres",
        host_port
    );

    let mut conn = PgConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to PostgreSQL");

    // INET型を使用したテーブル作成SQL
    let create_table_sql = r#"
        CREATE TABLE test_inet_table (
            id SERIAL PRIMARY KEY,
            ip_address INET NOT NULL
        )
    "#;

    sqlx::query(create_table_sql)
        .execute(&mut conn)
        .await
        .expect("Failed to create table with INET type");

    // データを挿入してINET型が正しく動作することを確認
    sqlx::query("INSERT INTO test_inet_table (ip_address) VALUES ($1::inet)")
        .bind("192.168.1.1")
        .execute(&mut conn)
        .await
        .expect("Failed to insert INET data");

    let result = sqlx::query("SELECT ip_address::text AS ip_address FROM test_inet_table")
        .fetch_one(&mut conn)
        .await
        .expect("Failed to fetch INET data");

    let ip: String = result.get("ip_address");
    assert_eq!(ip, "192.168.1.1/32");
}

/// PostgreSQL: ARRAY型を使用したテーブル作成の統合テスト
#[tokio::test]
#[ignore]
async fn test_postgres_array_type_table_creation() {
    let container = Postgres::default()
        .start()
        .await
        .expect("Failed to start PostgreSQL container");

    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get container port");

    let connection_string = format!(
        "postgres://postgres:postgres@127.0.0.1:{}/postgres",
        host_port
    );

    let mut conn = PgConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to PostgreSQL");

    // ARRAY型を使用したテーブル作成SQL
    let create_table_sql = r#"
        CREATE TABLE test_array_table (
            id SERIAL PRIMARY KEY,
            tags TEXT[]
        )
    "#;

    sqlx::query(create_table_sql)
        .execute(&mut conn)
        .await
        .expect("Failed to create table with ARRAY type");

    // データを挿入してARRAY型が正しく動作することを確認
    sqlx::query("INSERT INTO test_array_table (tags) VALUES ($1)")
        .bind(vec!["rust", "postgresql", "strata"])
        .execute(&mut conn)
        .await
        .expect("Failed to insert ARRAY data");

    let result = sqlx::query("SELECT tags FROM test_array_table")
        .fetch_one(&mut conn)
        .await
        .expect("Failed to fetch ARRAY data");

    let tags: Vec<String> = result.get("tags");
    assert_eq!(tags, vec!["rust", "postgresql", "strata"]);
}

/// PostgreSQL: 共通型と方言固有型の混在スキーマの統合テスト
#[tokio::test]
#[ignore]
async fn test_postgres_mixed_common_and_dialect_specific_types() {
    let container = Postgres::default()
        .start()
        .await
        .expect("Failed to start PostgreSQL container");

    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get container port");

    let connection_string = format!(
        "postgres://postgres:postgres@127.0.0.1:{}/postgres",
        host_port
    );

    let mut conn = PgConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to PostgreSQL");

    // 共通型（VARCHAR, DECIMAL）と方言固有型（SERIAL）の混在
    let create_table_sql = r#"
        CREATE TABLE test_mixed_table (
            id SERIAL PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            price NUMERIC(10, 2) NOT NULL
        )
    "#;

    sqlx::query(create_table_sql)
        .execute(&mut conn)
        .await
        .expect("Failed to create table with mixed types");

    // データを挿入して動作確認
    sqlx::query("INSERT INTO test_mixed_table (name, price) VALUES ($1, $2)")
        .bind("Test Product")
        .bind(99.99)
        .execute(&mut conn)
        .await
        .expect("Failed to insert data");

    let result = sqlx::query("SELECT id, name, price FROM test_mixed_table")
        .fetch_one(&mut conn)
        .await
        .expect("Failed to fetch data");

    let id: i32 = result.get("id");
    let name: String = result.get("name");

    assert_eq!(id, 1); // SERIAL auto-increment
    assert_eq!(name, "Test Product");
}

/// MySQL: ENUM型を使用したテーブル作成の統合テスト
#[tokio::test]
#[ignore]
async fn test_mysql_enum_type_table_creation() {
    let container = Mysql::default()
        .start()
        .await
        .expect("Failed to start MySQL container");

    let host_port = container
        .get_host_port_ipv4(3306)
        .await
        .expect("Failed to get container port");

    let connection_string = format!("mysql://root@127.0.0.1:{}/mysql", host_port);

    let mut conn = MySqlConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to MySQL");

    // ENUM型を使用したテーブル作成SQL
    let create_table_sql = r#"
        CREATE TABLE test_enum_table (
            id INT AUTO_INCREMENT PRIMARY KEY,
            status ENUM('active', 'inactive', 'pending') NOT NULL
        )
    "#;

    sqlx::query(create_table_sql)
        .execute(&mut conn)
        .await
        .expect("Failed to create table with ENUM type");

    // データを挿入してENUM型が正しく動作することを確認
    sqlx::query("INSERT INTO test_enum_table (status) VALUES (?)")
        .bind("active")
        .execute(&mut conn)
        .await
        .expect("Failed to insert ENUM data");

    let result = sqlx::query("SELECT status FROM test_enum_table")
        .fetch_one(&mut conn)
        .await
        .expect("Failed to fetch ENUM data");

    let status: String = result.get("status");
    assert_eq!(status, "active");
}

/// MySQL: TINYINT型を使用したテーブル作成の統合テスト
#[tokio::test]
#[ignore]
async fn test_mysql_tinyint_type_table_creation() {
    let container = Mysql::default()
        .start()
        .await
        .expect("Failed to start MySQL container");

    let host_port = container
        .get_host_port_ipv4(3306)
        .await
        .expect("Failed to get container port");

    let connection_string = format!("mysql://root@127.0.0.1:{}/mysql", host_port);

    let mut conn = MySqlConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to MySQL");

    // TINYINT型を使用したテーブル作成SQL
    let create_table_sql = r#"
        CREATE TABLE test_tinyint_table (
            id INT AUTO_INCREMENT PRIMARY KEY,
            age TINYINT UNSIGNED NOT NULL
        )
    "#;

    sqlx::query(create_table_sql)
        .execute(&mut conn)
        .await
        .expect("Failed to create table with TINYINT type");

    // データを挿入してTINYINT型が正しく動作することを確認
    sqlx::query("INSERT INTO test_tinyint_table (age) VALUES (?)")
        .bind(25)
        .execute(&mut conn)
        .await
        .expect("Failed to insert TINYINT data");

    let result = sqlx::query("SELECT age FROM test_tinyint_table")
        .fetch_one(&mut conn)
        .await
        .expect("Failed to fetch TINYINT data");

    let age: u8 = result.get("age");
    assert_eq!(age, 25);
}

/// MySQL: SET型を使用したテーブル作成の統合テスト
#[tokio::test]
#[ignore]
async fn test_mysql_set_type_table_creation() {
    let container = Mysql::default()
        .start()
        .await
        .expect("Failed to start MySQL container");

    let host_port = container
        .get_host_port_ipv4(3306)
        .await
        .expect("Failed to get container port");

    let connection_string = format!("mysql://root@127.0.0.1:{}/mysql", host_port);

    let mut conn = MySqlConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to MySQL");

    // SET型を使用したテーブル作成SQL
    let create_table_sql = r#"
        CREATE TABLE test_set_table (
            id INT AUTO_INCREMENT PRIMARY KEY,
            permissions SET('read', 'write', 'execute', 'delete') NOT NULL
        )
    "#;

    sqlx::query(create_table_sql)
        .execute(&mut conn)
        .await
        .expect("Failed to create table with SET type");

    // データを挿入してSET型が正しく動作することを確認
    sqlx::query("INSERT INTO test_set_table (permissions) VALUES (?)")
        .bind("read,write")
        .execute(&mut conn)
        .await
        .expect("Failed to insert SET data");

    let result = sqlx::query("SELECT permissions FROM test_set_table")
        .fetch_one(&mut conn)
        .await
        .expect("Failed to fetch SET data");

    let permissions: String = result.get("permissions");
    assert_eq!(permissions, "read,write");
}

/// MySQL: 共通型と方言固有型の混在スキーマの統合テスト
#[tokio::test]
#[ignore]
async fn test_mysql_mixed_common_and_dialect_specific_types() {
    let container = Mysql::default()
        .start()
        .await
        .expect("Failed to start MySQL container");

    let host_port = container
        .get_host_port_ipv4(3306)
        .await
        .expect("Failed to get container port");

    let connection_string = format!("mysql://root@127.0.0.1:{}/mysql", host_port);

    let mut conn = MySqlConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to MySQL");

    // 共通型（VARCHAR, TEXT）と方言固有型（ENUM, TINYINT）の混在
    let create_table_sql = r#"
        CREATE TABLE test_mixed_table (
            id INT AUTO_INCREMENT PRIMARY KEY,
            username VARCHAR(50) NOT NULL,
            status ENUM('active', 'inactive') NOT NULL,
            age TINYINT UNSIGNED,
            description TEXT
        )
    "#;

    sqlx::query(create_table_sql)
        .execute(&mut conn)
        .await
        .expect("Failed to create table with mixed types");

    // データを挿入して動作確認
    sqlx::query(
        "INSERT INTO test_mixed_table (username, status, age, description) VALUES (?, ?, ?, ?)",
    )
    .bind("testuser")
    .bind("active")
    .bind(30)
    .bind("Test description")
    .execute(&mut conn)
    .await
    .expect("Failed to insert data");

    let result = sqlx::query("SELECT id, username, status, age, description FROM test_mixed_table")
        .fetch_one(&mut conn)
        .await
        .expect("Failed to fetch data");

    let id: i32 = result.get("id");
    let username: String = result.get("username");
    let status: String = result.get("status");
    let age: Option<u8> = result.get("age");
    let description: Option<String> = result.get("description");

    assert_eq!(id, 1); // AUTO_INCREMENT
    assert_eq!(username, "testuser");
    assert_eq!(status, "active");
    assert_eq!(age, Some(30));
    assert_eq!(description, Some("Test description".to_string()));
}

/// MySQL: AUTO_INCREMENT属性がイントロスペクトされることを確認する回帰テスト
///
/// Issue #11: MySQL `export` loses `auto_increment` attribute on columns
/// この問題を修正するため、information_schema.columnsのEXTRAカラムから
/// auto_increment属性を検出するようにした。
#[tokio::test]
#[ignore]
async fn test_mysql_auto_increment_introspection() {
    use strata::adapters::database_introspector::create_introspector;
    use strata::core::config::Dialect;

    let container = Mysql::default()
        .start()
        .await
        .expect("Failed to start MySQL container");

    let host_port = container
        .get_host_port_ipv4(3306)
        .await
        .expect("Failed to get container port");

    let connection_string = format!("mysql://root@127.0.0.1:{}/mysql", host_port);

    let mut conn = MySqlConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to MySQL");

    // AUTO_INCREMENTを持つテーブルを作成
    let create_table_sql = r#"
        CREATE TABLE test_auto_increment (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            counter INT NOT NULL
        )
    "#;

    sqlx::query(create_table_sql)
        .execute(&mut conn)
        .await
        .expect("Failed to create table with AUTO_INCREMENT");

    // AnyPoolを使用してイントロスペクターをテスト
    sqlx::any::install_default_drivers();
    let any_connection_string = format!("mysql://root@127.0.0.1:{}/mysql", host_port);
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect(&any_connection_string)
        .await
        .expect("Failed to create AnyPool");

    // MySQLイントロスペクターを使用してカラム情報を取得
    let introspector = create_introspector(Dialect::MySQL);
    let columns = introspector
        .get_columns(&pool, "test_auto_increment")
        .await
        .expect("Failed to get columns");

    // カラムが3つあることを確認
    assert_eq!(columns.len(), 3);

    // idカラムのauto_incrementがSome(true)であることを確認
    let id_column = columns.iter().find(|c| c.name == "id").unwrap();
    assert_eq!(
        id_column.auto_increment,
        Some(true),
        "id column should have auto_increment = Some(true)"
    );

    // nameカラムのauto_incrementがNone/Some(false)であることを確認
    let name_column = columns.iter().find(|c| c.name == "name").unwrap();
    assert!(
        name_column.auto_increment.is_none() || name_column.auto_increment == Some(false),
        "name column should not have auto_increment"
    );

    // counterカラムのauto_incrementがNone/Some(false)であることを確認
    let counter_column = columns.iter().find(|c| c.name == "counter").unwrap();
    assert!(
        counter_column.auto_increment.is_none() || counter_column.auto_increment == Some(false),
        "counter column should not have auto_increment"
    );
}
