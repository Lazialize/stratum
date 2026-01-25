/// SchemaParserServiceのテスト
///
/// スキーマディレクトリのスキャン、YAML解析、スキーマファイルのマージ処理が
/// 正しく動作することを確認します。
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[cfg(test)]
mod schema_parser_tests {
    use super::*;
    use strata::services::schema_parser::SchemaParserService;

    /// テスト用の一時ディレクトリとスキーマファイルを作成
    fn setup_test_schema_dir() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let schema_dir = temp_dir.path();

        // users.yaml を作成（新構文）
        let users_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_users_email
        columns:
          - email
        unique: true
"#;
        fs::write(schema_dir.join("users.yaml"), users_yaml).unwrap();

        // posts.yaml を作成（新構文）
        let posts_yaml = r#"
version: "1.0"
tables:
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: user_id
        type:
          kind: INTEGER
        nullable: false
      - name: title
        type:
          kind: VARCHAR
          length: 200
        nullable: false
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - user_id
        referenced_table: users
        referenced_columns:
          - id
"#;
        fs::write(schema_dir.join("posts.yaml"), posts_yaml).unwrap();

        temp_dir
    }

    /// 単一のスキーマファイル解析のテスト
    #[test]
    fn test_parse_single_schema_file() {
        let temp_dir = setup_test_schema_dir();
        let schema_file = temp_dir.path().join("users.yaml");

        let parser = SchemaParserService::new();
        let schema = parser
            .parse_schema_file(&schema_file)
            .expect("Failed to parse schema file");

        assert_eq!(schema.version, "1.0");
        assert_eq!(schema.tables.len(), 1);
        assert!(schema.tables.contains_key("users"));

        let users_table = schema.tables.get("users").unwrap();
        assert_eq!(users_table.name, "users");
        assert_eq!(users_table.columns.len(), 2);
        assert_eq!(users_table.indexes.len(), 1);
        assert_eq!(users_table.constraints.len(), 1);
    }

    /// スキーマディレクトリのスキャンとマージのテスト
    #[test]
    fn test_parse_schema_directory() {
        let temp_dir = setup_test_schema_dir();

        let parser = SchemaParserService::new();
        let schema = parser
            .parse_schema_directory(temp_dir.path())
            .expect("Failed to parse schema directory");

        // 2つのテーブルがマージされているはず
        assert_eq!(schema.tables.len(), 2);
        assert!(schema.tables.contains_key("users"));
        assert!(schema.tables.contains_key("posts"));

        // usersテーブルの検証
        let users_table = schema.tables.get("users").unwrap();
        assert_eq!(users_table.columns.len(), 2);

        // postsテーブルの検証
        let posts_table = schema.tables.get("posts").unwrap();
        assert_eq!(posts_table.columns.len(), 3);
        assert_eq!(posts_table.constraints.len(), 2);
    }

    /// YAMLファイルが存在しないディレクトリのテスト
    #[test]
    fn test_parse_empty_directory() {
        let temp_dir = TempDir::new().unwrap();

        let parser = SchemaParserService::new();
        let result = parser.parse_schema_directory(temp_dir.path());

        // 空のディレクトリでもエラーではなく、空のスキーマを返す
        assert!(result.is_ok());
        let schema = result.unwrap();
        assert_eq!(schema.tables.len(), 0);
    }

    /// 存在しないファイルの解析テスト
    #[test]
    fn test_parse_nonexistent_file() {
        let parser = SchemaParserService::new();
        let result = parser.parse_schema_file(&PathBuf::from("/nonexistent/path/schema.yaml"));

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("File not found") || error_message.contains("No such file"));
    }

    /// 不正なYAML構文のテスト
    #[test]
    fn test_parse_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let invalid_file = temp_dir.path().join("invalid.yaml");

        // 不正なYAML
        fs::write(&invalid_file, "invalid: [yaml: syntax}").unwrap();

        let parser = SchemaParserService::new();
        let result = parser.parse_schema_file(&invalid_file);

        assert!(result.is_err());
    }

    /// 複数ファイルからのテーブルマージのテスト
    #[test]
    fn test_merge_multiple_schema_files() {
        let temp_dir = TempDir::new().unwrap();
        let schema_dir = temp_dir.path();

        // 複数のスキーマファイルを作成（新構文）
        let schema1 = r#"
version: "1.0"
tables:
  table1:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
    primary_key:
      - id
"#;
        fs::write(schema_dir.join("schema1.yaml"), schema1).unwrap();

        let schema2 = r#"
version: "1.0"
tables:
  table2:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
    primary_key:
      - id
"#;
        fs::write(schema_dir.join("schema2.yaml"), schema2).unwrap();

        let parser = SchemaParserService::new();
        let schema = parser
            .parse_schema_directory(schema_dir)
            .expect("Failed to parse directory");

        assert_eq!(schema.tables.len(), 2);
        assert!(schema.tables.contains_key("table1"));
        assert!(schema.tables.contains_key("table2"));
    }

    /// YAMLファイルのみをスキャン（.yamlと.yml拡張子）
    #[test]
    fn test_scan_only_yaml_files() {
        let temp_dir = TempDir::new().unwrap();
        let schema_dir = temp_dir.path();

        // .yaml ファイル（新構文）
        let schema_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
"#;
        fs::write(schema_dir.join("users.yaml"), schema_yaml).unwrap();

        // .yml ファイル
        fs::write(schema_dir.join("posts.yml"), schema_yaml).unwrap();

        // .txt ファイル（無視されるべき）
        fs::write(schema_dir.join("readme.txt"), "This is not YAML").unwrap();

        let parser = SchemaParserService::new();
        let schema = parser
            .parse_schema_directory(schema_dir)
            .expect("Failed to parse directory");

        // .yaml と .yml のみが解析されるはず
        assert_eq!(schema.tables.len(), 1); // users テーブルのみ（postsも同じusersなので上書き）
    }

    /// スキーマバージョンの保持テスト
    #[test]
    fn test_preserve_schema_version() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        let schema_yaml = r#"
version: "2.5.1"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
"#;
        fs::write(&schema_file, schema_yaml).unwrap();

        let parser = SchemaParserService::new();
        let schema = parser
            .parse_schema_file(&schema_file)
            .expect("Failed to parse schema");

        assert_eq!(schema.version, "2.5.1");
    }

    /// ディレクトリスキャンの順序非依存性テスト
    #[test]
    fn test_directory_scan_order_independence() {
        let temp_dir = TempDir::new().unwrap();
        let schema_dir = temp_dir.path();

        // 複数のスキーマファイルを作成（ファイル名の順序が異なる）
        for i in 1..=5 {
            let content = format!(
                r#"
version: "1.0"
tables:
  table{}:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
"#,
                i
            );
            fs::write(schema_dir.join(format!("schema{}.yaml", i)), content).unwrap();
        }

        let parser = SchemaParserService::new();
        let schema = parser
            .parse_schema_directory(schema_dir)
            .expect("Failed to parse directory");

        // 順序に関わらず、すべてのテーブルがマージされるはず
        assert_eq!(schema.tables.len(), 5);
        for i in 1..=5 {
            assert!(schema.tables.contains_key(&format!("table{}", i)));
        }
    }

    // ======================================
    // Task 6.4: 統合テスト - バリデーション連携
    // ======================================

    /// 新構文でパースしたスキーマのバリデーション連携テスト
    #[test]
    fn test_parse_and_validate_new_syntax_valid_schema() {
        use strata::services::schema_validator::SchemaValidatorService;

        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // 有効な新構文スキーマ
        let valid_schema = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_users_email
        columns:
          - email
        unique: true
"#;
        fs::write(&schema_file, valid_schema).unwrap();

        let parser = SchemaParserService::new();
        let schema = parser.parse_schema_file(&schema_file).unwrap();

        // バリデーション実行
        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // 有効なスキーマはバリデーション成功
        assert!(result.is_valid(), "Validation errors: {:?}", result.errors);
    }

    /// PRIMARY_KEYに存在しないカラムを指定した場合のバリデーションエラー
    #[test]
    fn test_parse_and_validate_invalid_primary_key_column() {
        use strata::services::schema_validator::SchemaValidatorService;

        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // primary_keyに存在しないカラムを指定
        let invalid_schema = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - nonexistent_column
"#;
        fs::write(&schema_file, invalid_schema).unwrap();

        let parser = SchemaParserService::new();
        let schema = parser.parse_schema_file(&schema_file).unwrap();

        // バリデーション実行
        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // 存在しないカラムを参照するPRIMARY_KEYはエラー
        assert!(!result.is_valid());
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.to_string().contains("nonexistent_column")),
            "Expected error about nonexistent_column, got: {:?}",
            result.errors
        );
    }

    /// 外部キー制約のバリデーション連携テスト
    #[test]
    fn test_parse_and_validate_foreign_key_constraint() {
        use strata::services::schema_validator::SchemaValidatorService;

        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // 参照先テーブルが存在しない外部キー
        let invalid_schema = r#"
version: "1.0"
tables:
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: user_id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - user_id
        referenced_table: users
        referenced_columns:
          - id
"#;
        fs::write(&schema_file, invalid_schema).unwrap();

        let parser = SchemaParserService::new();
        let schema = parser.parse_schema_file(&schema_file).unwrap();

        // バリデーション実行
        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // 存在しないテーブルへの参照はエラー
        assert!(!result.is_valid());
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.to_string().contains("users")),
            "Expected error about missing users table, got: {:?}",
            result.errors
        );
    }

    /// 往復テスト: パース→シリアライズ→パース
    #[test]
    fn test_round_trip_parse_serialize_parse() {
        use strata::services::schema_serializer::SchemaSerializerService;

        let temp_dir = TempDir::new().unwrap();
        let original_file = temp_dir.path().join("original.yaml");
        let serialized_file = temp_dir.path().join("serialized.yaml");

        // 複雑なスキーマを作成
        let original_schema = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
      - name: age
        type:
          kind: INTEGER
        nullable: true
    primary_key:
      - id
    indexes:
      - name: idx_users_email
        columns:
          - email
        unique: true
    constraints:
      - type: UNIQUE
        columns:
          - email
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: user_id
        type:
          kind: INTEGER
        nullable: false
      - name: title
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - user_id
        referenced_table: users
        referenced_columns:
          - id
"#;
        fs::write(&original_file, original_schema).unwrap();

        // パース
        let parser = SchemaParserService::new();
        let schema = parser.parse_schema_file(&original_file).unwrap();

        // シリアライズ
        let serializer = SchemaSerializerService::new();
        serializer
            .serialize_to_file(&schema, &serialized_file)
            .unwrap();

        // 再パース
        let reparsed_schema = parser.parse_schema_file(&serialized_file).unwrap();

        // 比較
        assert_eq!(schema.version, reparsed_schema.version);
        assert_eq!(schema.tables.len(), reparsed_schema.tables.len());

        // usersテーブル
        let original_users = schema.get_table("users").unwrap();
        let reparsed_users = reparsed_schema.get_table("users").unwrap();
        assert_eq!(original_users.columns.len(), reparsed_users.columns.len());
        assert_eq!(
            original_users.get_primary_key_columns(),
            reparsed_users.get_primary_key_columns()
        );
        assert_eq!(original_users.indexes.len(), reparsed_users.indexes.len());
        assert_eq!(
            original_users.constraints.len(),
            reparsed_users.constraints.len()
        );

        // postsテーブル
        let original_posts = schema.get_table("posts").unwrap();
        let reparsed_posts = reparsed_schema.get_table("posts").unwrap();
        assert_eq!(original_posts.columns.len(), reparsed_posts.columns.len());
        assert_eq!(
            original_posts.constraints.len(),
            reparsed_posts.constraints.len()
        );
    }
}
