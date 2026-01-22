/// DialectSpecific型のデータベース実行時エラーメッセージ伝達テスト
///
/// testcontainersを使用して実際のデータベースに対して無効な型を使用し、
/// データベースからのエラーメッセージが透過的に伝達されることを確認します。
///
/// 注意: このテストはDockerが必要です。Docker未起動の場合はスキップされます。

#[cfg(test)]
mod dialect_specific_database_error_tests {
    use sqlx::{Connection, PgConnection, Row};
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;

    /// PostgreSQLで無効な型名を使用した場合のエラーメッセージ伝達テスト
    ///
    /// 注意: このテストはDockerが必要です。
    /// 実行方法: `cargo test --test dialect_specific_database_error_test -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn test_postgres_invalid_type_error_message() {
        // testcontainersでPostgreSQLコンテナを起動
        let container = Postgres::default()
            .start()
            .await
            .expect("Failed to start PostgreSQL container");

        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get container port");

        let connection_string = format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            port
        );

        let mut conn = PgConnection::connect(&connection_string)
            .await
            .expect("Failed to connect to PostgreSQL");

        // 無効な型名を使用したCREATE TABLE文を実行
        // "SERIALS"は存在しない型（正しくは"SERIAL"）
        let sql = "CREATE TABLE test_table (id SERIALS);";

        let result = sqlx::query(sql).execute(&mut conn).await;

        // エラーが発生することを確認
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_message = error.to_string();

        // データベースからのエラーメッセージが含まれることを確認
        // PostgreSQLは "type \"SERIALS\" does not exist" のようなエラーを返す
        assert!(
            error_message.contains("SERIALS") || error_message.contains("type"),
            "Expected error message to contain type error, got: {}",
            error_message
        );

        // PostgreSQLは存在しない型に対してHINTを提供することがある
        // 例: HINT: Did you mean "SERIAL"?
        // (注: 全てのPostgreSQLバージョンでHINTが提供されるわけではない)
    }

    /// PostgreSQLで正しい型名（SERIAL）を使用した場合の成功テスト
    #[tokio::test]
    #[ignore]
    async fn test_postgres_valid_dialect_specific_type() {
        let container = Postgres::default()
            .start()
            .await
            .expect("Failed to start PostgreSQL container");

        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get container port");

        let connection_string = format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            port
        );

        let mut conn = PgConnection::connect(&connection_string)
            .await
            .expect("Failed to connect to PostgreSQL");

        // 正しいPostgreSQL方言固有型（SERIAL）を使用
        let sql = "CREATE TABLE test_table (id SERIAL PRIMARY KEY);";

        let result = sqlx::query(sql).execute(&mut conn).await;

        // 成功することを確認
        assert!(
            result.is_ok(),
            "Expected CREATE TABLE with SERIAL to succeed, got error: {:?}",
            result.err()
        );

        // テーブルが作成されたことを確認
        let check_sql = "SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'test_table' AND column_name = 'id';";
        let row = sqlx::query(check_sql)
            .fetch_one(&mut conn)
            .await
            .expect("Failed to query table metadata");

        let column_name: String = row.get("column_name");
        assert_eq!(column_name, "id");

        // SERIALはINTEGERとして実装される
        let data_type: String = row.get("data_type");
        assert_eq!(data_type, "integer");
    }

    /// PostgreSQLのINET型（ネットワークアドレス型）のテスト
    #[tokio::test]
    #[ignore]
    async fn test_postgres_inet_type() {
        let container = Postgres::default()
            .start()
            .await
            .expect("Failed to start PostgreSQL container");

        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get container port");

        let connection_string = format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            port
        );

        let mut conn = PgConnection::connect(&connection_string)
            .await
            .expect("Failed to connect to PostgreSQL");

        // INET型を使用したテーブル作成
        let sql = "CREATE TABLE network_log (id SERIAL PRIMARY KEY, ip_address INET);";

        let result = sqlx::query(sql).execute(&mut conn).await;

        assert!(
            result.is_ok(),
            "Expected CREATE TABLE with INET to succeed, got error: {:?}",
            result.err()
        );

        // INET型のカラムが作成されたことを確認
        let check_sql = "SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'network_log' AND column_name = 'ip_address';";
        let row = sqlx::query(check_sql)
            .fetch_one(&mut conn)
            .await
            .expect("Failed to query table metadata");

        let data_type: String = row.get("data_type");
        // PostgreSQLのINET型はUSER-DEFINED typeとして表示される
        assert!(data_type.contains("USER-DEFINED") || data_type.contains("inet"));
    }

    /// PostgreSQLで無効なパラメータを持つ型のエラーテスト
    #[tokio::test]
    #[ignore]
    async fn test_postgres_invalid_type_parameter() {
        let container = Postgres::default()
            .start()
            .await
            .expect("Failed to start PostgreSQL container");

        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get container port");

        let connection_string = format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            port
        );

        let mut conn = PgConnection::connect(&connection_string)
            .await
            .expect("Failed to connect to PostgreSQL");

        // VARCHARに無効な長さパラメータを指定
        // PostgreSQLのVARCHARの最大長は10485760
        let sql = "CREATE TABLE test_table (name VARCHAR(10485761));";

        let result = sqlx::query(sql).execute(&mut conn).await;

        // エラーが発生することを確認
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_message = error.to_string();

        // データベースからのパラメータエラーメッセージが含まれることを確認
        assert!(
            error_message.contains("length")
                || error_message.contains("invalid")
                || error_message.contains("at most"),
            "Expected error message to contain parameter error, got: {}",
            error_message
        );
    }

    // MySQLのテストはMySQL testcontainerが必要
    // 現在のプロジェクトではPostgreSQLのみを使用しているため、MySQLテストはコメントアウト
    /*
    #[tokio::test]
    async fn test_mysql_enum_type() {
        use testcontainers_modules::mysql::Mysql;

        let container = Mysql::default()
            .start()
            .await
            .expect("Failed to start MySQL container");

        let port = container
            .get_host_port_ipv4(3306)
            .await
            .expect("Failed to get container port");

        let connection_string = format!("mysql://root@127.0.0.1:{}/test", port);

        let mut conn = sqlx::MySqlConnection::connect(&connection_string)
            .await
            .expect("Failed to connect to MySQL");

        // ENUM型を使用したテーブル作成
        let sql = "CREATE TABLE users (id INT AUTO_INCREMENT PRIMARY KEY, status ENUM('active', 'inactive', 'pending'));";

        let result = sqlx::query(sql).execute(&mut conn).await;

        assert!(
            result.is_ok(),
            "Expected CREATE TABLE with ENUM to succeed, got error: {:?}",
            result.err()
        );
    }
    */
}
