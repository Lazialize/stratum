/// CLI統合テスト
///
/// 実際のユーザーシナリオに基づいたCLIコマンドの統合テストを実施します。
///
/// テスト内容:
/// - 複数マイグレーションの一括適用/ロールバック
/// - apply/rollbackの往復サイクル
/// - テーブルの段階的進化（カラム追加、リネーム等）
/// - E2Eフルフロー（init → generate → apply → rollback）
/// - エッジケース（変更なし、ロールバック境界等）
///
/// 注意: Docker必須のテストは #[ignore] アトリビュートでマークされています。
/// Docker起動時に実行するには: `cargo test cli_integration -- --ignored`
#[cfg(test)]
#[allow(dead_code)]
mod cli_integration_tests {
    use sqlx::any::install_default_drivers;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt};
    use testcontainers_modules::mysql::Mysql as MysqlImage;
    use testcontainers_modules::postgres::Postgres as PostgresImage;

    use strata::cli::commands::apply::{ApplyCommand, ApplyCommandHandler};
    use strata::cli::commands::generate::{GenerateCommand, GenerateCommandHandler};
    use strata::cli::commands::rollback::{RollbackCommand, RollbackCommandHandler};
    use strata::cli::commands::status::{StatusCommand, StatusCommandHandler};
    use strata::core::config::{Config, DatabaseConfig, Dialect};
    use strata::services::config_serializer::ConfigSerializer;

    // ==========================================
    // テストヘルパー
    // ==========================================

    /// テスト用プロジェクトを管理する構造体
    struct TestProject {
        pub temp_dir: TempDir,
        pub project_path: PathBuf,
        pub dialect: Dialect,
        pub db_path: Option<PathBuf>,
    }

    impl TestProject {
        /// SQLiteプロジェクトを作成
        fn sqlite() -> Self {
            let temp_dir = TempDir::new().unwrap();
            let project_path = temp_dir.path().to_path_buf();
            let db_path = project_path.join("test.db");

            // データベースファイルを作成
            fs::File::create(&db_path).unwrap();

            Self {
                temp_dir,
                project_path,
                dialect: Dialect::SQLite,
                db_path: Some(db_path),
            }
        }

        /// プロジェクトを初期化（init相当）
        fn init(&self) {
            // ディレクトリを作成
            fs::create_dir_all(self.project_path.join("schema")).unwrap();
            fs::create_dir_all(self.project_path.join("migrations")).unwrap();

            // 設定ファイルを作成
            let db_config = if let Some(ref db_path) = self.db_path {
                DatabaseConfig {
                    host: String::new(),
                    database: db_path.to_string_lossy().to_string(),
                    ..Default::default()
                }
            } else {
                DatabaseConfig {
                    port: Some(5432),
                    database: "testdb".to_string(),
                    user: Some("postgres".to_string()),
                    password: Some("postgres".to_string()),
                    ..Default::default()
                }
            };

            let mut environments = HashMap::new();
            environments.insert("development".to_string(), db_config);

            let config = Config {
                version: "1.0".to_string(),
                dialect: self.dialect,
                schema_dir: PathBuf::from("schema"),
                migrations_dir: PathBuf::from("migrations"),
                environments,
            };

            let config_yaml = ConfigSerializer::to_yaml(&config).unwrap();
            fs::write(self.project_path.join(".strata.yaml"), config_yaml).unwrap();
        }

        /// PostgreSQL用の設定でプロジェクトを初期化
        fn init_with_postgres_config(&self, host: &str, port: u16, database: &str) {
            fs::create_dir_all(self.project_path.join("schema")).unwrap();
            fs::create_dir_all(self.project_path.join("migrations")).unwrap();

            let db_config = DatabaseConfig {
                host: host.to_string(),
                port: Some(port),
                database: database.to_string(),
                user: Some("postgres".to_string()),
                password: Some("postgres".to_string()),
                timeout: Some(30),
                ..Default::default()
            };

            let mut environments = HashMap::new();
            environments.insert("development".to_string(), db_config);

            let config = Config {
                version: "1.0".to_string(),
                dialect: Dialect::PostgreSQL,
                schema_dir: PathBuf::from("schema"),
                migrations_dir: PathBuf::from("migrations"),
                environments,
            };

            let config_yaml = ConfigSerializer::to_yaml(&config).unwrap();
            fs::write(self.project_path.join(".strata.yaml"), config_yaml).unwrap();
        }

        /// MySQL用の設定でプロジェクトを初期化
        fn init_with_mysql_config(&self, host: &str, port: u16, database: &str) {
            fs::create_dir_all(self.project_path.join("schema")).unwrap();
            fs::create_dir_all(self.project_path.join("migrations")).unwrap();

            let db_config = DatabaseConfig {
                host: host.to_string(),
                port: Some(port),
                database: database.to_string(),
                user: Some("root".to_string()),
                timeout: Some(30),
                ..Default::default()
            };

            let mut environments = HashMap::new();
            environments.insert("development".to_string(), db_config);

            let config = Config {
                version: "1.0".to_string(),
                dialect: Dialect::MySQL,
                schema_dir: PathBuf::from("schema"),
                migrations_dir: PathBuf::from("migrations"),
                environments,
            };

            let config_yaml = ConfigSerializer::to_yaml(&config).unwrap();
            fs::write(self.project_path.join(".strata.yaml"), config_yaml).unwrap();
        }

        /// スキーマYAMLを追加（テーブル作成）
        fn add_table(&self, table_name: &str, columns: &[(&str, &str)]) {
            let mut column_defs = Vec::new();
            for (name, col_type) in columns {
                let type_def = match *col_type {
                    "INTEGER" | "INT" => {
                        if *name == "id" {
                            format!(
                                r#"      - name: {}
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true"#,
                                name
                            )
                        } else {
                            format!(
                                r#"      - name: {}
        type:
          kind: INTEGER
        nullable: false"#,
                                name
                            )
                        }
                    }
                    "TEXT" | "VARCHAR" => format!(
                        r#"      - name: {}
        type:
          kind: VARCHAR
          length: 255
        nullable: false"#,
                        name
                    ),
                    "BOOLEAN" => format!(
                        r#"      - name: {}
        type:
          kind: BOOLEAN
        nullable: false"#,
                        name
                    ),
                    _ => format!(
                        r#"      - name: {}
        type:
          kind: VARCHAR
          length: 255
        nullable: true"#,
                        name
                    ),
                };
                column_defs.push(type_def);
            }

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
                self.project_path
                    .join("schema")
                    .join(format!("{}.yaml", table_name)),
                schema_content,
            )
            .unwrap();
        }

        /// スキーマYAMLを更新（カラム追加）
        fn add_column(&self, table_name: &str, columns: &[(&str, &str)], new_column: (&str, &str)) {
            let mut all_columns: Vec<(&str, &str)> = columns.to_vec();
            all_columns.push(new_column);
            self.add_table(table_name, &all_columns);
        }

        /// スキーマYAMLを更新（カラムリネーム）
        fn rename_column(
            &self,
            table_name: &str,
            columns: &[(&str, &str)],
            old_name: &str,
            new_name: &str,
        ) {
            let mut column_defs = Vec::new();
            for (name, col_type) in columns {
                let actual_name = if *name == old_name { new_name } else { *name };
                let renamed_from = if *name == old_name {
                    format!("\n        renamed_from: {}", old_name)
                } else {
                    String::new()
                };

                let type_def = match *col_type {
                    "INTEGER" | "INT" => {
                        if *name == "id" {
                            format!(
                                r#"      - name: {}
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true{}"#,
                                actual_name, renamed_from
                            )
                        } else {
                            format!(
                                r#"      - name: {}
        type:
          kind: INTEGER
        nullable: false{}"#,
                                actual_name, renamed_from
                            )
                        }
                    }
                    "TEXT" | "VARCHAR" => format!(
                        r#"      - name: {}
        type:
          kind: VARCHAR
          length: 255
        nullable: false{}"#,
                        actual_name, renamed_from
                    ),
                    _ => format!(
                        r#"      - name: {}
        type:
          kind: VARCHAR
          length: 255
        nullable: true{}"#,
                        actual_name, renamed_from
                    ),
                };
                column_defs.push(type_def);
            }

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
                self.project_path
                    .join("schema")
                    .join(format!("{}.yaml", table_name)),
                schema_content,
            )
            .unwrap();
        }

        /// generateコマンドを実行
        fn generate(&self, description: &str) -> Result<String, String> {
            self.generate_with_allow(description, false)
        }

        /// generateコマンドを実行（破壊的変更許可）
        fn generate_allow_destructive(&self, description: &str) -> Result<String, String> {
            self.generate_with_allow(description, true)
        }

        fn generate_with_allow(
            &self,
            description: &str,
            allow_destructive: bool,
        ) -> Result<String, String> {
            let handler = GenerateCommandHandler::new();
            let command = GenerateCommand {
                project_path: self.project_path.clone(),
                config_path: None,
                description: Some(description.to_string()),
                dry_run: false,
                allow_destructive,
            };

            handler.execute(&command).map_err(|e| e.to_string())
        }

        /// applyコマンドを実行
        async fn apply(&self) -> Result<String, String> {
            self.apply_with_allow(false).await
        }

        /// applyコマンドを実行（破壊的変更許可）
        async fn apply_allow_destructive(&self) -> Result<String, String> {
            self.apply_with_allow(true).await
        }

        async fn apply_with_allow(&self, allow_destructive: bool) -> Result<String, String> {
            let handler = ApplyCommandHandler::new();
            let command = ApplyCommand {
                project_path: self.project_path.clone(),
                config_path: None,
                dry_run: false,
                env: "development".to_string(),
                timeout: None,
                allow_destructive,
            };

            handler.execute(&command).await.map_err(|e| e.to_string())
        }

        /// apply --dry-runを実行
        async fn apply_dry_run(&self) -> Result<String, String> {
            let handler = ApplyCommandHandler::new();
            let command = ApplyCommand {
                project_path: self.project_path.clone(),
                config_path: None,
                dry_run: true,
                env: "development".to_string(),
                timeout: None,
                allow_destructive: false,
            };

            handler.execute(&command).await.map_err(|e| e.to_string())
        }

        /// rollbackコマンドを実行
        async fn rollback(&self, steps: u32) -> Result<String, String> {
            let handler = RollbackCommandHandler::new();
            let command = RollbackCommand {
                project_path: self.project_path.clone(),
                config_path: None,
                steps: Some(steps),
                env: "development".to_string(),
                dry_run: false,
                allow_destructive: true, // down.sql may contain DROP TABLE
            };

            handler.execute(&command).await.map_err(|e| e.to_string())
        }

        /// statusコマンドを実行
        async fn status(&self) -> Result<String, String> {
            let handler = StatusCommandHandler::new();
            let command = StatusCommand {
                project_path: self.project_path.clone(),
                config_path: None,
                env: "development".to_string(),
            };

            handler.execute(&command).await.map_err(|e| e.to_string())
        }

        /// マイグレーションディレクトリの数を取得
        fn migration_count(&self) -> usize {
            let migrations_dir = self.project_path.join("migrations");
            if !migrations_dir.exists() {
                return 0;
            }

            fs::read_dir(&migrations_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| !n.starts_with('.'))
                        .unwrap_or(false)
                })
                .count()
        }

        /// マイグレーションディレクトリのリストを取得（ソート済み）
        fn migration_dirs(&self) -> Vec<PathBuf> {
            let migrations_dir = self.project_path.join("migrations");
            if !migrations_dir.exists() {
                return Vec::new();
            }

            let mut dirs: Vec<PathBuf> = fs::read_dir(&migrations_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| !n.starts_with('.'))
                        .unwrap_or(false)
                })
                .map(|e| e.path())
                .collect();

            dirs.sort();
            dirs
        }
    }

    /// PostgreSQLプロジェクト（testcontainers用）
    struct PostgresTestProject {
        pub project: TestProject,
        pub container: ContainerAsync<PostgresImage>,
        pub pool: sqlx::AnyPool,
    }

    impl PostgresTestProject {
        async fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let container = PostgresImage::default()
                .with_tag("16-alpine")
                .start()
                .await?;

            let host = container.get_host().await?;
            let port = container.get_host_port_ipv4(5432).await?;
            let connection_string =
                format!("postgresql://postgres:postgres@{}:{}/postgres", host, port);

            let pool = sqlx::any::AnyPoolOptions::new()
                .max_connections(5)
                .connect(&connection_string)
                .await?;

            let temp_dir = TempDir::new()?;
            let project_path = temp_dir.path().to_path_buf();

            let project = TestProject {
                temp_dir,
                project_path,
                dialect: Dialect::PostgreSQL,
                db_path: None,
            };

            project.init_with_postgres_config(&host.to_string(), port, "postgres");

            Ok(Self {
                project,
                container,
                pool,
            })
        }

        /// テーブルが存在するか確認
        async fn table_exists(&self, table_name: &str) -> bool {
            let result = sqlx::query(
                "SELECT 1 FROM information_schema.tables WHERE table_schema = ANY(current_schemas(false)) AND table_name = $1",
            )
            .bind(table_name)
            .fetch_optional(&self.pool)
            .await;

            match result {
                Ok(row) => row.is_some(),
                Err(_) => false,
            }
        }

        /// カラムが存在するか確認
        async fn column_exists(&self, table_name: &str, column_name: &str) -> bool {
            let result = sqlx::query(
                "SELECT 1 FROM information_schema.columns WHERE table_schema = ANY(current_schemas(false)) AND table_name = $1 AND column_name = $2",
            )
            .bind(table_name)
            .bind(column_name)
            .fetch_optional(&self.pool)
            .await;

            match result {
                Ok(row) => row.is_some(),
                Err(_) => false,
            }
        }
    }

    /// MySQLプロジェクト（testcontainers用）
    struct MySqlTestProject {
        pub project: TestProject,
        pub container: ContainerAsync<MysqlImage>,
        pub pool: sqlx::AnyPool,
    }

    impl MySqlTestProject {
        async fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let container = MysqlImage::default().with_tag("8.0").start().await?;

            let host = container.get_host().await?;
            let port = container.get_host_port_ipv4(3306).await?;

            // MySQL起動待ち
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            let connection_string = format!("mysql://root@{}:{}/mysql", host, port);

            let pool = sqlx::any::AnyPoolOptions::new()
                .max_connections(5)
                .connect(&connection_string)
                .await?;

            let temp_dir = TempDir::new()?;
            let project_path = temp_dir.path().to_path_buf();

            let project = TestProject {
                temp_dir,
                project_path,
                dialect: Dialect::MySQL,
                db_path: None,
            };

            project.init_with_mysql_config(&host.to_string(), port, "mysql");

            Ok(Self {
                project,
                container,
                pool,
            })
        }

        /// テーブルが存在するか確認
        async fn table_exists(&self, table_name: &str) -> bool {
            let result = sqlx::query(
                "SELECT 1 FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = ?",
            )
            .bind(table_name)
            .fetch_optional(&self.pool)
            .await;

            match result {
                Ok(row) => row.is_some(),
                Err(_) => false,
            }
        }
    }

    // ==========================================
    // バッチ操作テスト: 複数マイグレーション一括適用
    // ==========================================

    /// 複数のマイグレーションを生成し、一括でapplyするテスト
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_batch_apply_multiple_migrations() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        // デバッグ: 設定ファイルの内容を確認
        let config_content = fs::read_to_string(project.project_path.join(".strata.yaml")).unwrap();
        println!("=== Config file ===\n{}", config_content);

        // Step 1: usersテーブルを追加してgenerate
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        let result = project.generate("create_users");
        assert!(result.is_ok(), "Failed to generate: {:?}", result);

        // タイムスタンプが秒単位なので、次のgenerate前に1秒待機
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Step 2: postsテーブルを追加してgenerate
        project.add_table("posts", &[("id", "INTEGER"), ("title", "VARCHAR")]);
        let result = project.generate("create_posts");
        assert!(result.is_ok(), "Failed to generate: {:?}", result);

        // タイムスタンプが秒単位なので、次のgenerate前に1秒待機
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Step 3: commentsテーブルを追加してgenerate
        project.add_table(
            "comments",
            &[
                ("id", "INTEGER"),
                ("content", "VARCHAR"),
                ("post_id", "INTEGER"),
            ],
        );
        let result = project.generate("create_comments");
        assert!(result.is_ok(), "Failed to generate: {:?}", result);

        // 3つのマイグレーションが生成されていることを確認
        assert_eq!(project.migration_count(), 3);

        // Step 4: 一括apply
        let result = project.apply().await;
        assert!(result.is_ok(), "Failed to apply: {:?}", result);

        let summary = result.unwrap();
        assert!(summary.contains("3"), "Expected 3 migrations applied");

        // Step 5: status確認
        let status = project.status().await;
        assert!(status.is_ok());
        let status_output = status.unwrap();
        assert!(
            status_output.contains("Applied") || status_output.contains("applied"),
            "Status should show applied migrations"
        );
    }

    /// 5つのマイグレーションを一括適用するテスト
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_batch_apply_five_migrations() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        // 5つのテーブルを順番に追加
        let tables = ["users", "posts", "comments", "tags", "categories"];
        for (i, table) in tables.iter().enumerate() {
            project.add_table(table, &[("id", "INTEGER"), ("name", "VARCHAR")]);
            let result = project.generate(&format!("create_{}", table));
            assert!(result.is_ok(), "Failed to generate {}: {:?}", table, result);
            // 最後以外は1秒待機（タイムスタンプ衝突回避）
            if i < tables.len() - 1 {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }

        assert_eq!(project.migration_count(), 5);

        // 一括apply
        let result = project.apply().await;
        assert!(result.is_ok(), "Failed to apply: {:?}", result);

        let summary = result.unwrap();
        assert!(summary.contains("5"), "Expected 5 migrations applied");
    }

    // ==========================================
    // バッチ操作テスト: 複数マイグレーション一括ロールバック
    // ==========================================

    /// 複数のマイグレーションを一括でrollbackするテスト
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_batch_rollback_multiple_migrations() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        // 3つのマイグレーションを生成
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("posts", &[("id", "INTEGER"), ("title", "VARCHAR")]);
        project.generate("create_posts").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("comments", &[("id", "INTEGER"), ("content", "VARCHAR")]);
        project.generate("create_comments").unwrap();

        // 一括apply
        project.apply().await.unwrap();

        // --steps 2 でrollback
        let result = project.rollback(2).await;
        assert!(result.is_ok(), "Failed to rollback: {:?}", result);

        let summary = result.unwrap();
        assert!(
            summary.contains("2") || summary.contains("rollback"),
            "Expected 2 migrations rolled back"
        );

        // status確認: 1つだけApplied、2つはPending
        let status = project.status().await.unwrap();
        println!("Status after rollback 2: {}", status);
    }

    /// 適用数を超えるstepsでrollbackするテスト（境界条件）
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_rollback_steps_exceeds_applied() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        // 3つのマイグレーションを生成してapply
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("posts", &[("id", "INTEGER"), ("title", "VARCHAR")]);
        project.generate("create_posts").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("comments", &[("id", "INTEGER"), ("content", "VARCHAR")]);
        project.generate("create_comments").unwrap();

        project.apply().await.unwrap();

        // --steps 5 でrollback（適用数3を超える）
        let result = project.rollback(5).await;
        // 適用数を超えるstepsの場合、適用済み全て（3つ）がロールバックされるべき
        assert!(
            result.is_ok(),
            "Rollback should succeed even with steps exceeding applied count"
        );
        let output = result.unwrap();
        assert!(
            output.contains("3 migration(s) rolled back"),
            "All 3 migrations should be rolled back, got: {}",
            output
        );
    }

    // ==========================================
    // apply/rollbackサイクルテスト
    // ==========================================

    /// apply → rollback → apply の往復テスト
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_apply_rollback_apply_cycle() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        // マイグレーション生成
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("posts", &[("id", "INTEGER"), ("title", "VARCHAR")]);
        project.generate("create_posts").unwrap();

        // Cycle 1: apply
        let result = project.apply().await;
        assert!(result.is_ok(), "Cycle 1 apply failed: {:?}", result);

        // Cycle 2: rollback 1
        let result = project.rollback(1).await;
        assert!(result.is_ok(), "Cycle 2 rollback failed: {:?}", result);

        // Cycle 3: apply (postsを再適用)
        let result = project.apply().await;
        assert!(result.is_ok(), "Cycle 3 apply failed: {:?}", result);

        // Cycle 4: rollback 2
        let result = project.rollback(2).await;
        assert!(result.is_ok(), "Cycle 4 rollback failed: {:?}", result);

        // Cycle 5: apply (両方再適用)
        let result = project.apply().await;
        assert!(result.is_ok(), "Cycle 5 apply failed: {:?}", result);
    }

    /// 複数サイクルの往復テスト
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_multiple_apply_rollback_cycles() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        // 3つのマイグレーションを生成
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("posts", &[("id", "INTEGER"), ("title", "VARCHAR")]);
        project.generate("create_posts").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("comments", &[("id", "INTEGER"), ("content", "VARCHAR")]);
        project.generate("create_comments").unwrap();

        // Cycle 1: apply all → rollback all
        project.apply().await.unwrap();
        project.rollback(3).await.unwrap();

        // Cycle 2: apply all → rollback 1 → apply 1
        project.apply().await.unwrap();
        project.rollback(1).await.unwrap();
        project.apply().await.unwrap();

        // Cycle 3: rollback 2 → apply 2
        project.rollback(2).await.unwrap();
        project.apply().await.unwrap();

        // 最終状態: 3つ全て適用済み
        let status = project.status().await.unwrap();
        println!("Final status after multiple cycles: {}", status);
    }

    /// 部分的なrollback後に新しいマイグレーションを追加するテスト
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_rollback_then_add_new_migration() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        // 2つのマイグレーションを生成してapply
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("posts", &[("id", "INTEGER"), ("title", "VARCHAR")]);
        project.generate("create_posts").unwrap();

        project.apply().await.unwrap();

        // rollback 1 (postsを戻す)
        project.rollback(1).await.unwrap();

        // 新しいテーブルを追加してgenerate（ロールバックされたマイグレーションと異なるタイムスタンプを確保）
        std::thread::sleep(std::time::Duration::from_secs(1));
        project.add_table("comments", &[("id", "INTEGER"), ("content", "VARCHAR")]);
        let result = project.generate("create_comments");
        assert!(
            result.is_ok(),
            "Failed to generate after rollback: {:?}",
            result
        );

        // apply (posts + comments)
        let result = project.apply().await;
        assert!(
            result.is_ok(),
            "Failed to apply after adding new migration: {:?}",
            result
        );

        // 最終的に3つのマイグレーションが存在
        assert_eq!(project.migration_count(), 3);
    }

    // ==========================================
    // テーブル進化テスト（連続マイグレーション）
    // ==========================================

    /// 単一テーブルの段階的進化テスト
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_table_evolution() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        // Step 1: users(id, name) 作成
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();
        project.apply().await.unwrap();

        // タイムスタンプ衝突を避けるためスリープ
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Step 2: email カラム追加
        project.add_column(
            "users",
            &[("id", "INTEGER"), ("name", "VARCHAR")],
            ("email", "VARCHAR"),
        );
        project.generate("add_email_to_users").unwrap();
        project.apply().await.unwrap();

        // タイムスタンプ衝突を避けるためスリープ
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Step 3: name → display_name リネーム
        project.rename_column(
            "users",
            &[("id", "INTEGER"), ("name", "VARCHAR"), ("email", "VARCHAR")],
            "name",
            "display_name",
        );
        project
            .generate_allow_destructive("rename_name_to_display_name")
            .unwrap();
        project.apply_allow_destructive().await.unwrap();

        // 3つのマイグレーションが生成されている
        assert_eq!(project.migration_count(), 3);
    }

    // ==========================================
    // エッジケーステスト
    // ==========================================

    /// 変更なしでgenerateするテスト
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_generate_with_no_changes() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        // 最初のマイグレーションを生成してapply
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();
        project.apply().await.unwrap();

        // 変更なしでgenerate
        let result = project.generate("no_changes");

        // 2.5: 「変更なし」は Ok で返されるようになった
        println!("Generate with no changes result: {:?}", result);
        assert!(result.is_ok(), "Expected Ok for no changes");
        let msg = result.unwrap().to_lowercase();
        assert!(
            msg.contains("no schema changes") || msg.contains("no changes"),
            "Expected 'no changes' message, got: {}",
            msg
        );

        // マイグレーション数は1のまま
        assert_eq!(project.migration_count(), 1);
    }

    /// 適用済みのマイグレーションに対して再applyするテスト
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_reapply_already_applied() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();

        // 1回目のapply
        project.apply().await.unwrap();

        // 2回目のapply（すでに適用済み）
        let result = project.apply().await;

        // エラーまたは「適用するマイグレーションなし」
        println!("Reapply result: {:?}", result);
        assert!(
            result.is_err()
                || result
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("no pending"),
            "Expected error or 'no pending migrations' message"
        );
    }

    /// 空の状態でrollbackするテスト
    #[tokio::test]
    #[ignore] // Requires SQLx Any driver
    async fn test_rollback_with_no_applied_migrations() {
        install_default_drivers();

        let project = TestProject::sqlite();
        project.init();

        // マイグレーションを生成するが適用しない
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();

        // rollback（適用済みなし）
        let result = project.rollback(1).await;

        // エラーになるべき（ロールバックするマイグレーションがない）
        assert!(
            result.is_err(),
            "Rollback should fail when no migrations are applied"
        );
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.to_lowercase().contains("no migrations")
                || err_msg.to_lowercase().contains("does not exist"),
            "Error should indicate no migrations to rollback, got: {}",
            err_msg
        );
    }

    // ==========================================
    // PostgreSQL E2Eテスト
    // ==========================================

    /// PostgreSQLでのフルフローテスト
    #[tokio::test]
    #[ignore] // Docker required
    async fn test_postgres_full_flow() {
        install_default_drivers();
        let test_project = PostgresTestProject::new().await.unwrap();
        let project = &test_project.project;

        // Step 1: テーブル追加
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        let result = project.generate("create_users");
        assert!(result.is_ok(), "Generate failed: {:?}", result);

        // Step 2: apply
        let result = project.apply().await;
        assert!(result.is_ok(), "Apply failed: {:?}", result);

        // Step 3: テーブル存在確認
        assert!(
            test_project.table_exists("users").await,
            "users table should exist"
        );

        // Step 4: カラム追加
        std::thread::sleep(std::time::Duration::from_secs(1)); // タイムスタンプ衝突回避
        project.add_column(
            "users",
            &[("id", "INTEGER"), ("name", "VARCHAR")],
            ("email", "VARCHAR"),
        );
        project.generate("add_email").unwrap();
        project.apply().await.unwrap();

        // Step 5: カラム存在確認
        assert!(
            test_project.column_exists("users", "email").await,
            "email column should exist"
        );

        // Step 6: rollback
        let result = project.rollback(1).await;
        assert!(result.is_ok(), "Rollback failed: {:?}", result);

        // Step 7: rollback後のカラム確認
        assert!(
            !test_project.column_exists("users", "email").await,
            "email column should not exist after rollback"
        );

        // Step 8: 再apply
        let result = project.apply().await;
        assert!(result.is_ok(), "Re-apply failed: {:?}", result);

        assert!(
            test_project.column_exists("users", "email").await,
            "email column should exist after re-apply"
        );
    }

    /// PostgreSQLでの複数サイクルテスト
    #[tokio::test]
    #[ignore] // Docker required
    async fn test_postgres_multiple_cycles() {
        install_default_drivers();
        let test_project = PostgresTestProject::new().await.unwrap();
        let project = &test_project.project;

        // 3つのテーブルを順次追加
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("posts", &[("id", "INTEGER"), ("title", "VARCHAR")]);
        project.generate("create_posts").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("comments", &[("id", "INTEGER"), ("content", "VARCHAR")]);
        project.generate("create_comments").unwrap();

        // Cycle 1: apply all
        project.apply().await.unwrap();
        assert!(test_project.table_exists("users").await);
        assert!(test_project.table_exists("posts").await);
        assert!(test_project.table_exists("comments").await);

        // Cycle 2: rollback 2
        project.rollback(2).await.unwrap();
        assert!(test_project.table_exists("users").await);
        assert!(!test_project.table_exists("posts").await);
        assert!(!test_project.table_exists("comments").await);

        // Cycle 3: apply remaining
        project.apply().await.unwrap();
        assert!(test_project.table_exists("posts").await);
        assert!(test_project.table_exists("comments").await);

        // Cycle 4: rollback all
        project.rollback(3).await.unwrap();
        assert!(!test_project.table_exists("users").await);
        assert!(!test_project.table_exists("posts").await);
        assert!(!test_project.table_exists("comments").await);

        // Cycle 5: apply all again
        project.apply().await.unwrap();
        assert!(test_project.table_exists("users").await);
        assert!(test_project.table_exists("posts").await);
        assert!(test_project.table_exists("comments").await);
    }

    // ==========================================
    // MySQL E2Eテスト
    // ==========================================

    /// MySQLでのフルフローテスト
    #[tokio::test]
    #[ignore] // Docker required
    async fn test_mysql_full_flow() {
        install_default_drivers();
        let test_project = MySqlTestProject::new().await.unwrap();
        let project = &test_project.project;

        // Step 1: テーブル追加
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        let result = project.generate("create_users");
        assert!(result.is_ok(), "Generate failed: {:?}", result);

        // Step 2: apply
        let result = project.apply().await;
        assert!(result.is_ok(), "Apply failed: {:?}", result);

        // Step 3: テーブル存在確認
        assert!(
            test_project.table_exists("users").await,
            "users table should exist"
        );

        // Step 4: カラム追加とapply
        std::thread::sleep(std::time::Duration::from_secs(1)); // タイムスタンプ衝突回避
        project.add_column(
            "users",
            &[("id", "INTEGER"), ("name", "VARCHAR")],
            ("email", "VARCHAR"),
        );
        project.generate("add_email").unwrap();
        project.apply().await.unwrap();

        // Step 5: rollback
        project.rollback(1).await.unwrap();

        // Step 6: 再apply
        project.apply().await.unwrap();
    }

    /// MySQLでの複数サイクルテスト
    #[tokio::test]
    #[ignore] // Docker required
    async fn test_mysql_multiple_cycles() {
        install_default_drivers();
        let test_project = MySqlTestProject::new().await.unwrap();
        let project = &test_project.project;

        // 3つのテーブルを順次追加
        project.add_table("users", &[("id", "INTEGER"), ("name", "VARCHAR")]);
        project.generate("create_users").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("posts", &[("id", "INTEGER"), ("title", "VARCHAR")]);
        project.generate("create_posts").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));

        project.add_table("comments", &[("id", "INTEGER"), ("content", "VARCHAR")]);
        project.generate("create_comments").unwrap();

        // apply → rollback → apply のサイクルを複数回
        project.apply().await.unwrap();
        project.rollback(2).await.unwrap();
        project.apply().await.unwrap();
        project.rollback(3).await.unwrap();
        project.apply().await.unwrap();

        // 最終確認
        assert!(test_project.table_exists("users").await);
        assert!(test_project.table_exists("posts").await);
        assert!(test_project.table_exists("comments").await);
    }
}
