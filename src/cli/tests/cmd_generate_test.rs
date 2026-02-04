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
            config_path: None,
            schema_dir: None,
            description: Some("test migration".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
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
            config_path: None,
            schema_dir: None,
            description: Some("test migration".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
        };

        let result = handler.execute(&command);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Config"));
    }

    /// 空のスキーマディレクトリの場合は差分なし
    /// 2.5: 「変更なし」は正常終了（Ok）として扱う
    #[test]
    fn test_execute_empty_schema_directory() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // プロジェクトをセットアップ
        setup_test_project(project_path, Dialect::PostgreSQL);

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            config_path: None,
            schema_dir: None,
            description: Some("initial migration".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
        };

        let result = handler.execute(&command);
        assert!(result.is_ok());
        let msg = result.unwrap();
        assert!(msg.contains("No schema changes"));
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
            config_path: None,
            schema_dir: None,
            description: Some("create users table".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
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
            config_path: None,
            schema_dir: None,
            description: None, // descriptionなし
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
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
            config_path: None,
            schema_dir: None,
            description: Some("create orders table".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
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
            config_path: None,
            schema_dir: None,
            description: Some("initial schema".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
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
            config_path: None,
            schema_dir: None,
            description: Some("create customers".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
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

    /// generate saves per-migration schema snapshot inside migration directory
    #[test]
    fn test_generate_saves_per_migration_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        setup_test_project(project_path, Dialect::PostgreSQL);
        create_simple_schema_file(project_path, "users", &["id", "name"]);

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            config_path: None,
            schema_dir: None,
            description: Some("create users table".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
        };

        handler.execute(&command).unwrap();

        // Find the migration directory
        let migrations_dir = project_path.join("migrations");
        let migration_dirs: Vec<_> = fs::read_dir(&migrations_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        assert_eq!(migration_dirs.len(), 1);
        let migration_dir = migration_dirs[0].path();

        // Per-migration snapshot should exist inside the migration directory
        let per_migration_snapshot = migration_dir.join(".schema_snapshot.yaml");
        assert!(
            per_migration_snapshot.exists(),
            "Per-migration .schema_snapshot.yaml should exist in {:?}",
            migration_dir
        );

        // It should contain the users table
        let content = fs::read_to_string(&per_migration_snapshot).unwrap();
        assert!(
            content.contains("users"),
            "Per-migration snapshot should contain 'users' table"
        );
    }

    /// Deleting a failed migration directory causes the next generate to produce
    /// correct CREATE TABLE instead of ALTER TABLE (issue #9 regression test)
    #[test]
    fn test_generate_after_deleting_failed_migration_produces_create_table() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        setup_test_project(project_path, Dialect::PostgreSQL);

        // Step 1: Create and generate the first table (users) - this is the "applied" baseline
        create_simple_schema_file(project_path, "users", &["id", "name"]);

        let handler = GenerateCommandHandler::new();
        let command = GenerateCommand {
            project_path: project_path.to_path_buf(),
            config_path: None,
            schema_dir: None,
            description: Some("create users".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
        };
        handler.execute(&command).unwrap();

        // Wait to ensure different timestamp for next migration
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Step 2: Add a new table (metadata) and generate migration
        create_simple_schema_file(project_path, "metadata", &["id", "name"]);

        let command2 = GenerateCommand {
            project_path: project_path.to_path_buf(),
            config_path: None,
            schema_dir: None,
            description: Some("create metadata".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
        };
        handler.execute(&command2).unwrap();

        // Verify we now have 2 migration directories
        let migrations_dir = project_path.join("migrations");
        let mut migration_dirs: Vec<_> = fs::read_dir(&migrations_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect();
        migration_dirs.sort();
        assert_eq!(migration_dirs.len(), 2);

        // Step 3: Simulate failed apply by deleting the second migration directory
        fs::remove_dir_all(&migration_dirs[1]).unwrap();

        // Wait to ensure different timestamp for next migration
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Step 4: Run generate again - should produce CREATE TABLE for metadata, not ALTER TABLE
        let command3 = GenerateCommand {
            project_path: project_path.to_path_buf(),
            config_path: None,
            schema_dir: None,
            description: Some("recreate metadata".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
        };
        handler.execute(&command3).unwrap();

        // Find the newly generated migration directory
        let mut migration_dirs_after: Vec<_> = fs::read_dir(&migrations_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect();
        migration_dirs_after.sort();
        assert_eq!(migration_dirs_after.len(), 2);

        // Read the new migration's up.sql
        let new_migration_dir = &migration_dirs_after[1];
        let up_sql = fs::read_to_string(new_migration_dir.join("up.sql")).unwrap();

        // Should contain CREATE TABLE, not ALTER TABLE
        assert!(
            up_sql.contains("CREATE TABLE"),
            "Expected CREATE TABLE in up.sql after deleting failed migration, got:\n{}",
            up_sql
        );
        assert!(
            !up_sql.contains("ALTER TABLE"),
            "Should NOT contain ALTER TABLE after deleting failed migration, got:\n{}",
            up_sql
        );
    }

    /// Multiple generates without apply work correctly with per-migration snapshots
    #[test]
    fn test_multiple_generates_without_apply() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        setup_test_project(project_path, Dialect::PostgreSQL);

        let handler = GenerateCommandHandler::new();

        // Step 1: Generate first migration (users table)
        create_simple_schema_file(project_path, "users", &["id", "name"]);
        let command1 = GenerateCommand {
            project_path: project_path.to_path_buf(),
            config_path: None,
            schema_dir: None,
            description: Some("create users".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
        };
        handler.execute(&command1).unwrap();

        // Wait to ensure different timestamp for next migration
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Step 2: Generate second migration (posts table) - without applying first
        create_simple_schema_file(project_path, "posts", &["id", "name"]);
        let command2 = GenerateCommand {
            project_path: project_path.to_path_buf(),
            config_path: None,
            schema_dir: None,
            description: Some("create posts".to_string()),
            dry_run: false,
            allow_destructive: false,
            verbose: false,
            format: strata::cli::OutputFormat::Text,
        };
        handler.execute(&command2).unwrap();

        // Find migration directories
        let migrations_dir = project_path.join("migrations");
        let mut migration_dirs: Vec<_> = fs::read_dir(&migrations_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect();
        migration_dirs.sort();
        assert_eq!(migration_dirs.len(), 2);

        // First migration should CREATE users only
        let up1 = fs::read_to_string(migration_dirs[0].join("up.sql")).unwrap();
        assert!(up1.contains("users"), "First migration should contain users table");
        assert!(!up1.contains("posts"), "First migration should NOT contain posts table");

        // Second migration should CREATE posts only (not duplicate users)
        let up2 = fs::read_to_string(migration_dirs[1].join("up.sql")).unwrap();
        assert!(up2.contains("posts"), "Second migration should contain posts table");
        assert!(!up2.contains("users"), "Second migration should NOT duplicate users table");
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
