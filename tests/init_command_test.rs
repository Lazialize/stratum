/// initコマンドハンドラーのテスト
///
/// プロジェクト初期化機能が正しく動作することを確認します。

#[cfg(test)]
mod init_command_tests {
    use std::fs;
    use std::path::PathBuf;
    use strata::cli::commands::init::{ConfigFileParams, InitCommand, InitCommandHandler};
    use strata::core::config::Dialect;
    use strata::services::config_loader::ConfigLoader;
    use tempfile::TempDir;

    /// コマンドハンドラーの作成テスト
    #[test]
    fn test_new_command_handler() {
        let handler = InitCommandHandler::new();
        assert!(format!("{:?}", handler).contains("InitCommandHandler"));
    }

    /// ディレクトリ構造の作成テスト
    #[test]
    fn test_create_directory_structure() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let handler = InitCommandHandler::new();
        handler
            .create_directory_structure(project_path)
            .expect("Failed to create directory structure");

        // schema/とmigrations/が作成されているか確認
        assert!(project_path.join("schema").exists());
        assert!(project_path.join("schema").is_dir());
        assert!(project_path.join("migrations").exists());
        assert!(project_path.join("migrations").is_dir());
    }

    /// 設定ファイル生成テスト - PostgreSQL
    #[test]
    fn test_generate_config_file_postgresql() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();
        let config_path = project_path.join(".strata.yaml");

        let handler = InitCommandHandler::new();
        let params = ConfigFileParams {
            dialect: Dialect::PostgreSQL,
            database_name: "mydb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            user: Some("user".to_string()),
            password: Some("pass".to_string()),
        };
        handler
            .generate_config_file(project_path, params)
            .expect("Failed to generate config file");

        // 設定ファイルが作成されているか確認
        assert!(config_path.exists());
        assert!(config_path.is_file());

        // 設定ファイルの内容を検証
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("version:"));
        assert!(content.contains("dialect: postgresql"));
        assert!(content.contains("schema_dir: schema"));
        assert!(content.contains("migrations_dir: migrations"));
        assert!(content.contains("development:"));
        assert!(content.contains("database: mydb"));
    }

    /// 設定ファイル生成テスト - MySQL
    #[test]
    fn test_generate_config_file_mysql() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();
        let config_path = project_path.join(".strata.yaml");

        let handler = InitCommandHandler::new();
        let params = ConfigFileParams {
            dialect: Dialect::MySQL,
            database_name: "mydb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            user: Some("root".to_string()),
            password: Some("pass".to_string()),
        };
        handler
            .generate_config_file(project_path, params)
            .expect("Failed to generate config file");

        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("dialect: mysql"));
        assert!(content.contains("port: 3306"));
    }

    /// 設定ファイル生成テスト - SQLite
    #[test]
    fn test_generate_config_file_sqlite() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();
        let config_path = project_path.join(".strata.yaml");

        let handler = InitCommandHandler::new();
        let params = ConfigFileParams {
            dialect: Dialect::SQLite,
            database_name: "db.sqlite".to_string(),
            host: None,
            port: None,
            user: None,
            password: None,
        };
        handler
            .generate_config_file(project_path, params)
            .expect("Failed to generate config file");

        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("dialect: sqlite"));
        assert!(content.contains("database: db.sqlite"));
    }

    /// 初期化済みプロジェクトの検出テスト
    #[test]
    fn test_is_already_initialized() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let handler = InitCommandHandler::new();

        // 初期状態では初期化されていない
        assert!(!handler.is_already_initialized(project_path));

        // 設定ファイルを作成
        fs::write(
            project_path.join(".strata.yaml"),
            "version: 1.0\ndialect: postgresql\n",
        )
        .unwrap();

        // 初期化済みと判定される
        assert!(handler.is_already_initialized(project_path));
    }

    /// force=falseで初期化済みプロジェクトを初期化しようとするとエラー
    #[test]
    fn test_execute_already_initialized_without_force() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // 事前に初期化
        let handler = InitCommandHandler::new();
        handler.create_directory_structure(project_path).unwrap();
        let params = ConfigFileParams {
            dialect: Dialect::PostgreSQL,
            database_name: "testdb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            user: Some("user".to_string()),
            password: Some("pass".to_string()),
        };
        handler.generate_config_file(project_path, params).unwrap();

        // 再初期化を試みる（force=false）
        let command = InitCommand {
            project_path: project_path.to_path_buf(),
            dialect: Dialect::PostgreSQL,
            force: false,
            database_name: "testdb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            user: Some("user".to_string()),
            password: Some("pass".to_string()),
        };

        let result = handler.execute(&command);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("already initialized"));
    }

    /// Re-initialize with force=true
    #[test]
    fn test_execute_already_initialized_with_force() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // 事前に初期化
        let handler = InitCommandHandler::new();
        handler.create_directory_structure(project_path).unwrap();
        let params = ConfigFileParams {
            dialect: Dialect::PostgreSQL,
            database_name: "testdb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            user: Some("user".to_string()),
            password: Some("pass".to_string()),
        };
        handler.generate_config_file(project_path, params).unwrap();

        // 再初期化を試みる（force=true）
        let command = InitCommand {
            project_path: project_path.to_path_buf(),
            dialect: Dialect::MySQL,
            force: true,
            database_name: "newdb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            user: Some("root".to_string()),
            password: Some("newpass".to_string()),
        };

        let result = handler.execute(&command);
        assert!(result.is_ok());

        // 設定ファイルが更新されているか確認
        let config_path = project_path.join(".strata.yaml");
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("dialect: mysql"));
        assert!(content.contains("database: newdb"));
    }

    /// 新規プロジェクトの初期化テスト
    #[test]
    fn test_execute_new_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let handler = InitCommandHandler::new();
        let command = InitCommand {
            project_path: project_path.to_path_buf(),
            dialect: Dialect::PostgreSQL,
            force: false,
            database_name: "myapp".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            user: Some("postgres".to_string()),
            password: Some("secret".to_string()),
        };

        let result = handler.execute(&command);
        assert!(result.is_ok());

        // ディレクトリが作成されているか
        assert!(project_path.join("schema").exists());
        assert!(project_path.join("migrations").exists());

        // 設定ファイルが作成されているか
        let config_path = project_path.join(".strata.yaml");
        assert!(config_path.exists());

        // 設定ファイルが正しくパースできるか
        let config = ConfigLoader::from_file(&config_path).unwrap();
        assert_eq!(config.dialect, Dialect::PostgreSQL);
        assert_eq!(config.schema_dir, PathBuf::from("schema"));
        assert_eq!(config.migrations_dir, PathBuf::from("migrations"));

        let dev_config = config.get_database_config("development").unwrap();
        assert_eq!(dev_config.database, "myapp");
    }

    /// 設定ファイルのバリデーションテスト
    #[test]
    fn test_generated_config_is_valid() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let handler = InitCommandHandler::new();
        let params = ConfigFileParams {
            dialect: Dialect::PostgreSQL,
            database_name: "testdb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            user: Some("user".to_string()),
            password: Some("pass".to_string()),
        };
        handler.generate_config_file(project_path, params).unwrap();

        let config_path = project_path.join(".strata.yaml");
        let config = ConfigLoader::from_file(&config_path).unwrap();

        // バリデーションが通ることを確認
        assert!(config.validate().is_ok());
    }

    /// 相対パスでの初期化テスト
    #[test]
    fn test_execute_with_relative_path() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let handler = InitCommandHandler::new();
        let command = InitCommand {
            project_path: project_path.to_path_buf(),
            dialect: Dialect::SQLite,
            force: false,
            database_name: "app.db".to_string(),
            host: None,
            port: None,
            user: None,
            password: None,
        };

        let result = handler.execute(&command);
        assert!(result.is_ok());

        // schema_dirとmigrations_dirが相対パスであることを確認
        let config_path = project_path.join(".strata.yaml");
        let config = ConfigLoader::from_file(&config_path).unwrap();
        assert!(config.schema_dir.is_relative());
        assert!(config.migrations_dir.is_relative());
    }
}
