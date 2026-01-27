/// generateコマンドハンドラーのテスト
///
/// スキーマ差分検出とマイグレーションファイル生成機能が正しく動作することを確認します。
#[cfg(test)]
mod generate_command_tests {
    use std::fs;
    use strata::cli::commands::generate::{GenerateCommand, GenerateCommandHandler};
    use strata::core::config::Dialect;
    use tempfile::TempDir;

    /// コマンドハンドラーの作成テスト
    #[test]
    fn test_new_command_handler() {
        let handler = GenerateCommandHandler::new();
        assert!(format!("{:?}", handler).contains("GenerateCommandHandler"));
    }

    /// スキーマディレクトリが存在しない場合のエラー
    #[test]
    fn test_execute_no_schema_directory() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            description: Some("test migration".to_string()),
            dry_run: false,
            allow_destructive: false,
        };

        let result = handler.execute(&command);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        eprintln!("Error message: {}", err_msg);
        // Config file missing, should get config-related error
        assert!(err_msg.contains("Config"));
    }

    /// 設定ファイルが存在しない場合のエラー
    #[test]
    fn test_execute_no_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // schema/ディレクトリだけ作成
        fs::create_dir_all(project_path.join("schema")).unwrap();

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            description: Some("test migration".to_string()),
            dry_run: false,
            allow_destructive: false,
        };

        let result = handler.execute(&command);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Config"));
    }

    /// 空のスキーマディレクトリの場合は差分なし
    #[test]
    fn test_execute_empty_schema_directory() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // プロジェクトをセットアップ
        setup_test_project(project_path, Dialect::PostgreSQL);

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            description: Some("initial migration".to_string()),
            dry_run: false,
            allow_destructive: false,
        };

        let result = handler.execute(&command);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("changes") || err_msg.contains("schema"));
    }

    /// 新規テーブル追加のマイグレーション生成
    #[test]
    fn test_execute_add_table_migration() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // プロジェクトをセットアップ
        setup_test_project(project_path, Dialect::PostgreSQL);

        // スキーマファイルを作成
        create_simple_schema_file(project_path, "users", &["id", "name"]);

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            description: Some("create users table".to_string()),
            dry_run: false,
            allow_destructive: false,
        };

        let result = handler.execute(&command);
        if let Err(err) = &result {
            eprintln!("Error: {:?}", err);
        }
        assert!(result.is_ok());

        let migration_file = result.unwrap();
        eprintln!("Migration file: {}", migration_file);
        assert!(!migration_file.is_empty());

        // マイグレーションファイルが生成されているか確認
        let migrations_dir = project_path.join("migrations");
        let entries: Vec<_> = fs::read_dir(&migrations_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();

        // up.sql, down.sql, .meta.yamlの3ファイルが含まれるディレクトリが作成されている
        assert!(!entries.is_empty());
    }

    /// descriptionが指定されていない場合の自動生成
    #[test]
    fn test_execute_auto_description() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        setup_test_project(project_path, Dialect::PostgreSQL);
        create_simple_schema_file(project_path, "products", &["id", "name", "price"]);

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            description: None, // descriptionなし
            dry_run: false,
            allow_destructive: false,
        };

        let result = handler.execute(&command);
        assert!(result.is_ok());

        let migration_file = result.unwrap();
        assert!(!migration_file.is_empty());
    }

    /// マイグレーションファイルの内容検証
    #[test]
    fn test_migration_file_contents() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        setup_test_project(project_path, Dialect::PostgreSQL);
        create_simple_schema_file(project_path, "orders", &["id", "user_id", "total"]);

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            description: Some("create orders table".to_string()),
            dry_run: false,
            allow_destructive: false,
        };

        handler.execute(&command).unwrap();

        // 生成されたマイグレーションディレクトリを探す
        let migrations_dir = project_path.join("migrations");
        let migration_dirs: Vec<_> = fs::read_dir(&migrations_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        assert!(!migration_dirs.is_empty());

        let migration_dir = &migration_dirs[0].path();

        // up.sqlの内容を確認
        let up_sql_path = migration_dir.join("up.sql");
        assert!(up_sql_path.exists());
        let up_sql = fs::read_to_string(&up_sql_path).unwrap();
        assert!(up_sql.contains("CREATE TABLE"));
        assert!(up_sql.contains("orders"));

        // down.sqlの内容を確認
        let down_sql_path = migration_dir.join("down.sql");
        assert!(down_sql_path.exists());
        let down_sql = fs::read_to_string(&down_sql_path).unwrap();
        assert!(down_sql.contains("DROP TABLE"));

        // .meta.yamlの内容を確認
        let meta_path = migration_dir.join(".meta.yaml");
        assert!(meta_path.exists());
        let meta = fs::read_to_string(&meta_path).unwrap();
        assert!(meta.contains("version:"));
        assert!(meta.contains("description:"));
    }

    /// 複数の変更を含むマイグレーション
    #[test]
    fn test_execute_multiple_changes() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        setup_test_project(project_path, Dialect::PostgreSQL);

        // 複数のテーブルを作成
        create_simple_schema_file(project_path, "users", &["id", "name"]);
        create_simple_schema_file(project_path, "posts", &["id", "title", "user_id"]);

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            description: Some("initial schema".to_string()),
            dry_run: false,
            allow_destructive: false,
        };

        let result = handler.execute(&command);
        assert!(result.is_ok());

        // up.sqlに両方のテーブルが含まれていることを確認
        let migrations_dir = project_path.join("migrations");
        let migration_dirs: Vec<_> = fs::read_dir(&migrations_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        let migration_dir = &migration_dirs[0].path();
        let up_sql = fs::read_to_string(migration_dir.join("up.sql")).unwrap();

        assert!(up_sql.contains("users"));
        assert!(up_sql.contains("posts"));
    }

    /// MySQL方言でのマイグレーション生成
    #[test]
    fn test_execute_with_mysql_dialect() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        setup_test_project(project_path, Dialect::MySQL);
        create_simple_schema_file(project_path, "customers", &["id", "email"]);

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            description: Some("create customers".to_string()),
            dry_run: false,
            allow_destructive: false,
        };

        let result = handler.execute(&command);
        assert!(result.is_ok());

        // MySQLのSQLが生成されていることを確認
        let migrations_dir = project_path.join("migrations");
        let migration_dirs: Vec<_> = fs::read_dir(&migrations_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        let migration_dir = &migration_dirs[0].path();
        let meta = fs::read_to_string(migration_dir.join(".meta.yaml")).unwrap();
        assert!(meta.contains("MySQL") || meta.contains("mysql"));
    }

    // ヘルパー関数

    /// テストプロジェクトをセットアップ
    fn setup_test_project(project_path: &std::path::Path, dialect: Dialect) {
        // ディレクトリを作成
        fs::create_dir_all(project_path.join("schema")).unwrap();
        fs::create_dir_all(project_path.join("migrations")).unwrap();

        // 設定ファイルを作成
        let config_content = format!(
            r#"version: "1.0"
dialect: {}
schema_dir: schema
migrations_dir: migrations
environments:
  development:
    host: localhost
    port: {}
    database: testdb
    user: testuser
    password: testpass
    timeout: 30
"#,
            dialect,
            match dialect {
                Dialect::PostgreSQL => 5432,
                Dialect::MySQL => 3306,
                Dialect::SQLite => 0,
            }
        );

        fs::write(project_path.join(".strata.yaml"), config_content).unwrap();
    }

    /// 簡単なスキーマファイルを作成（新構文）
    fn create_simple_schema_file(
        project_path: &std::path::Path,
        table_name: &str,
        columns: &[&str],
    ) {
        let mut column_defs = Vec::new();
        for col in columns {
            if *col == "id" {
                column_defs.push(format!(
                    r#"      - name: {}
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true"#,
                    col
                ));
            } else {
                column_defs.push(format!(
                    r#"      - name: {}
        type:
          kind: VARCHAR
          length: 255
        nullable: false"#,
                    col
                ));
            }
        }

        // 新構文: name フィールドなし、primary_key は独立フィールド
        let schema_content = format!(
            r#"version: "1.0"
tables:
  {}:
    columns:
{}
    primary_key:
      - id
"#,
            table_name,
            column_defs.join("\n")
        );

        fs::write(
            project_path
                .join("schema")
                .join(format!("{}.yaml", table_name)),
            schema_content,
        )
        .unwrap();
    }
}
