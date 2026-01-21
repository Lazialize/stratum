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
    use stratum::services::schema_parser::SchemaParserService;

    /// テスト用の一時ディレクトリとスキーマファイルを作成
    fn setup_test_schema_dir() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let schema_dir = temp_dir.path();

        // users.yaml を作成
        let users_yaml = r#"
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
        fs::write(schema_dir.join("users.yaml"), users_yaml).unwrap();

        // posts.yaml を作成
        let posts_yaml = r#"
version: "1.0"
tables:
  posts:
    name: posts
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: true
      - name: user_id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: null
      - name: title
        type:
          kind: VARCHAR
          length: 200
        nullable: false
        default_value: null
        auto_increment: null
    indexes: []
    constraints:
      - type: PRIMARY_KEY
        columns:
          - id
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
        assert!(error_message.contains("ファイルが見つかりません") || error_message.contains("No such file"));
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

        // 複数のスキーマファイルを作成
        let schema1 = r#"
version: "1.0"
tables:
  table1:
    name: table1
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: true
    indexes: []
    constraints: []
"#;
        fs::write(schema_dir.join("schema1.yaml"), schema1).unwrap();

        let schema2 = r#"
version: "1.0"
tables:
  table2:
    name: table2
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: true
    indexes: []
    constraints: []
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

        // .yaml ファイル
        let schema_yaml = r#"
version: "1.0"
tables:
  users:
    name: users
    columns: []
    indexes: []
    constraints: []
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
    name: users
    columns: []
    indexes: []
    constraints: []
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
    name: table{}
    columns: []
    indexes: []
    constraints: []
"#,
                i, i
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
}
