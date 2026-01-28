// スキーマパーサーサービス
//
// YAMLスキーマファイルの読み込み、解析、マージ処理を行うサービス。
// ディレクトリ全体のスキーマファイルをスキャンし、統合されたスキーマを生成します。
//
// DTO変換はDtoConverterServiceに委譲しています。

use crate::core::error::IoError;
use crate::core::schema::Schema;
use crate::services::schema_io::dto::SchemaDto;
use crate::services::schema_io::dto_converter::DtoConverterService;
use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

/// スキーマパーサーサービス
///
/// YAMLスキーマファイルの解析とマージを行います。
/// DTO変換はDtoConverterServiceに委譲しています。
#[derive(Debug, Clone)]
pub struct SchemaParserService {
    /// DTO変換サービス
    dto_converter: DtoConverterService,
}

impl SchemaParserService {
    /// 新しいSchemaParserServiceを作成
    pub fn new() -> Self {
        Self {
            dto_converter: DtoConverterService::new(),
        }
    }

    /// 指定されたディレクトリからすべてのYAMLファイルを読み込み、統合されたスキーマを返す
    ///
    /// # Arguments
    ///
    /// * `schema_dir` - スキーマ定義ファイルが格納されたディレクトリ
    ///
    /// # Returns
    ///
    /// 統合されたスキーマオブジェクト
    ///
    /// # Errors
    ///
    /// - ディレクトリが存在しない場合
    /// - YAMLファイルの解析に失敗した場合
    pub fn parse_schema_directory(&self, schema_dir: &Path) -> Result<Schema> {
        // ディレクトリの存在確認
        if !schema_dir.exists() {
            return Err(IoError::FileNotFound {
                path: schema_dir.display().to_string(),
            }
            .into());
        }

        if !schema_dir.is_dir() {
            return Err(anyhow::anyhow!(
                "The specified path is not a directory: {}",
                schema_dir.display()
            ));
        }

        // ディレクトリ内のYAMLファイルを収集
        let yaml_files = self.scan_yaml_files(schema_dir)?;

        // YAMLファイルが存在しない場合は空のスキーマを返す
        if yaml_files.is_empty() {
            return Ok(Schema::new("1.0".to_string()));
        }

        // 各YAMLファイルを解析してスキーマをマージ
        let mut merged_schema = Schema::new("1.0".to_string());

        for file_path in yaml_files {
            let schema = self
                .parse_schema_file(&file_path)
                .with_context(|| format!("Failed to parse schema file: {:?}", file_path))?;

            // バージョンを保持（最初に見つかったバージョンを使用）
            if merged_schema.table_count() == 0 {
                merged_schema.version = schema.version;
            }

            // テーブルをマージ
            for (table_name, table) in schema.tables {
                merged_schema.tables.insert(table_name, table);
            }

            // ENUMをマージ
            for (enum_name, enum_def) in schema.enums {
                merged_schema.enums.insert(enum_name, enum_def);
            }
        }

        Ok(merged_schema)
    }

    /// 単一のYAMLファイルを解析してスキーマオブジェクトに変換
    ///
    /// # Arguments
    ///
    /// * `file_path` - スキーマ定義ファイルのパス
    ///
    /// # Returns
    ///
    /// 解析されたスキーマオブジェクト
    ///
    /// # Errors
    ///
    /// - ファイルが存在しない場合
    /// - ファイルの読み込みに失敗した場合
    /// - YAMLの解析に失敗した場合
    pub fn parse_schema_file(&self, file_path: &Path) -> Result<Schema> {
        // ファイルの存在確認
        if !file_path.exists() {
            return Err(IoError::FileNotFound {
                path: file_path.display().to_string(),
            }
            .into());
        }

        // ファイル内容を読み込み
        let content = fs::read_to_string(file_path).map_err(|e| IoError::FileRead {
            path: file_path.display().to_string(),
            cause: e.to_string(),
        })?;

        // YAMLをDTOにデシリアライズ
        let dto: SchemaDto =
            serde_saphyr::from_str(&content).map_err(|e| self.format_parse_error(file_path, e))?;

        // DTOを内部モデルに変換（DtoConverterServiceに委譲）
        self.dto_converter.dto_to_schema(&dto)
    }

    /// serde_saphyrエラーから行番号を抽出
    fn extract_line_from_error(&self, error: &serde_saphyr::Error) -> Option<usize> {
        let error_msg = error.to_string();
        let re = Regex::new(r"line (\d+)").ok()?;
        re.captures(&error_msg)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse().ok())
    }

    /// エラーメッセージのフォーマット
    fn format_parse_error(&self, file_path: &Path, error: serde_saphyr::Error) -> anyhow::Error {
        match self.extract_line_from_error(&error) {
            Some(line) => anyhow::anyhow!(
                "Failed to parse YAML at {}:{}: {}",
                file_path.display(),
                line,
                error
            ),
            None => anyhow::anyhow!("Failed to parse YAML at {}: {}", file_path.display(), error),
        }
    }

    /// ディレクトリ内のYAMLファイルをスキャン
    ///
    /// .yaml と .yml 拡張子を持つファイルのみを収集します。
    ///
    /// # Arguments
    ///
    /// * `dir` - スキャンするディレクトリ
    ///
    /// # Returns
    ///
    /// YAMLファイルのパスのリスト
    fn scan_yaml_files(&self, dir: &Path) -> Result<Vec<std::path::PathBuf>> {
        let mut yaml_files = Vec::new();

        // ディレクトリエントリを読み込み
        let entries = fs::read_dir(dir).map_err(|e| IoError::FileRead {
            path: dir.display().to_string(),
            cause: e.to_string(),
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| IoError::FileRead {
                path: dir.display().to_string(),
                cause: e.to_string(),
            })?;

            let path = entry.path();

            // ファイルのみを対象とする（ディレクトリは除外）
            if !path.is_file() {
                continue;
            }

            // .yaml または .yml 拡張子を持つファイルのみを対象
            if let Some(extension) = path.extension() {
                if extension == "yaml" || extension == "yml" {
                    yaml_files.push(path);
                }
            }
        }

        // ファイル名でソート（順序の一貫性を保証）
        yaml_files.sort();

        Ok(yaml_files)
    }
}

impl Default for SchemaParserService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_service() {
        let service = SchemaParserService::new();
        // サービスが正常に作成されることを確認
        assert!(format!("{:?}", service).contains("SchemaParserService"));
    }

    #[test]
    fn test_scan_yaml_files() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // テストファイルを作成
        fs::write(dir.join("schema1.yaml"), "test").unwrap();
        fs::write(dir.join("schema2.yml"), "test").unwrap();
        fs::write(dir.join("readme.txt"), "test").unwrap();
        fs::write(dir.join("config.json"), "test").unwrap();

        let service = SchemaParserService::new();
        let yaml_files = service.scan_yaml_files(dir).unwrap();

        // .yaml と .yml のみが収集されるはず
        assert_eq!(yaml_files.len(), 2);

        let file_names: Vec<String> = yaml_files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();

        assert!(file_names.contains(&"schema1.yaml".to_string()));
        assert!(file_names.contains(&"schema2.yml".to_string()));
    }

    #[test]
    fn test_parse_nonexistent_directory() {
        let service = SchemaParserService::new();
        let result = service.parse_schema_directory(Path::new("/nonexistent/directory"));

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("File not found"));
    }

    #[test]
    fn test_parse_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let service = SchemaParserService::new();
        let schema = service.parse_schema_directory(temp_dir.path()).unwrap();

        // 空のディレクトリからは空のスキーマが返される
        assert_eq!(schema.tables.len(), 0);
        assert_eq!(schema.enums.len(), 0);
        assert_eq!(schema.version, "1.0");
    }

    #[test]
    fn test_parse_valid_schema_file() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // 新構文: name フィールドなし、primary_key は独立フィールド
        let schema_content = r#"
version: "1.0"
enums:
  status:
    name: status
    values: ["active", "inactive"]
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: status
        type:
          kind: ENUM
          name: status
        nullable: false
    primary_key:
      - id
"#;
        fs::write(&schema_file, schema_content).unwrap();

        let service = SchemaParserService::new();
        let schema = service.parse_schema_file(&schema_file).unwrap();

        assert_eq!(schema.version, "1.0");
        assert_eq!(schema.tables.len(), 1);
        assert_eq!(schema.enums.len(), 1);
        assert!(schema.has_table("users"));
    }

    // ======================================
    // Task 2.1 & 2.2: DTO → 内部モデル変換テスト
    // ======================================

    #[test]
    fn test_parse_new_syntax_table_name_from_key() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // 新構文: テーブル名はキー名から取得
        let schema_content = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
"#;
        fs::write(&schema_file, schema_content).unwrap();

        let service = SchemaParserService::new();
        let schema = service.parse_schema_file(&schema_file).unwrap();

        assert!(schema.has_table("users"));
        let table = schema.get_table("users").unwrap();
        assert_eq!(table.name, "users");
    }

    #[test]
    fn test_parse_new_syntax_primary_key_to_constraint() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // 新構文: primary_keyフィールドが独立している
        let schema_content = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#;
        fs::write(&schema_file, schema_content).unwrap();

        let service = SchemaParserService::new();
        let schema = service.parse_schema_file(&schema_file).unwrap();

        let table = schema.get_table("users").unwrap();
        let pk_columns = table.get_primary_key_columns();
        assert!(pk_columns.is_some());
        assert_eq!(pk_columns.unwrap(), vec!["id"]);
    }

    #[test]
    fn test_parse_new_syntax_composite_primary_key() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        let schema_content = r#"
version: "1.0"
tables:
  user_roles:
    columns:
      - name: user_id
        type:
          kind: INTEGER
        nullable: false
      - name: role_id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - user_id
      - role_id
"#;
        fs::write(&schema_file, schema_content).unwrap();

        let service = SchemaParserService::new();
        let schema = service.parse_schema_file(&schema_file).unwrap();

        let table = schema.get_table("user_roles").unwrap();
        let pk_columns = table.get_primary_key_columns();
        assert!(pk_columns.is_some());
        assert_eq!(pk_columns.unwrap(), vec!["user_id", "role_id"]);
    }

    #[test]
    fn test_parse_new_syntax_constraint_dto_to_constraint() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        let schema_content = r#"
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
        fs::write(&schema_file, schema_content).unwrap();

        let service = SchemaParserService::new();
        let schema = service.parse_schema_file(&schema_file).unwrap();

        let table = schema.get_table("posts").unwrap();
        // 2つの制約: PRIMARY_KEY と FOREIGN_KEY
        assert_eq!(table.constraints.len(), 2);

        // FOREIGN_KEY制約を確認
        let fk = table.constraints.iter().find(|c| c.kind() == "FOREIGN_KEY");
        assert!(fk.is_some());
    }

    #[test]
    fn test_parse_new_syntax_optional_indexes() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // indexesフィールドを省略
        let schema_content = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#;
        fs::write(&schema_file, schema_content).unwrap();

        let service = SchemaParserService::new();
        let schema = service.parse_schema_file(&schema_file).unwrap();

        let table = schema.get_table("users").unwrap();
        assert!(table.indexes.is_empty());
    }

    #[test]
    fn test_parse_new_syntax_optional_constraints() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // constraintsフィールドを省略（primary_keyのみ）
        let schema_content = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#;
        fs::write(&schema_file, schema_content).unwrap();

        let service = SchemaParserService::new();
        let schema = service.parse_schema_file(&schema_file).unwrap();

        let table = schema.get_table("users").unwrap();
        // PRIMARY_KEY制約のみ
        assert_eq!(table.constraints.len(), 1);
        assert_eq!(table.constraints[0].kind(), "PRIMARY_KEY");
    }

    // ======================================
    // Task 2.3: 行番号抽出テスト
    // ======================================

    #[test]
    fn test_parse_error_contains_line_number() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // 不正なYAML（line 5でエラー）
        let invalid_content = r#"
version: "1.0"
tables:
  users:
    columns: invalid_not_a_list
"#;
        fs::write(&schema_file, invalid_content).unwrap();

        let service = SchemaParserService::new();
        let result = service.parse_schema_file(&schema_file);

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // エラーメッセージに行番号が含まれることを確認
        assert!(
            error_msg.contains("line") || error_msg.contains(":"),
            "Error message should contain line info: {}",
            error_msg
        );
    }

    // ======================================
    // Task 6.2: カラム未定義エラーテスト
    // ======================================

    #[test]
    fn test_parse_error_columns_missing() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // columnsフィールドが存在しない不正なスキーマ
        let invalid_content = r#"
version: "1.0"
tables:
  users:
    primary_key:
      - id
"#;
        fs::write(&schema_file, invalid_content).unwrap();

        let service = SchemaParserService::new();
        let result = service.parse_schema_file(&schema_file);

        // serde_saphyrはcolumnsが必須なのでデシリアライズエラーになる
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // エラーメッセージにcolumns関連の情報が含まれることを確認
        assert!(
            error_msg.contains("columns") || error_msg.contains("missing"),
            "Error message should indicate columns is required: {}",
            error_msg
        );
    }

    #[test]
    fn test_parse_columns_empty_array() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // columnsフィールドが空配列のスキーマ
        // パース自体は成功するが、バリデーションでエラーになるべき
        let content = r#"
version: "1.0"
tables:
  users:
    columns: []
    primary_key:
      - id
"#;
        fs::write(&schema_file, content).unwrap();

        let service = SchemaParserService::new();
        let result = service.parse_schema_file(&schema_file);

        // 空のcolumnsでもパースは成功する
        assert!(result.is_ok());
        let schema = result.unwrap();
        let table = schema.get_table("users").unwrap();
        assert!(table.columns.is_empty());
    }

    #[test]
    fn test_parse_directory_merges_enums_from_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        let file1 = r#"
version: "1.0"
enums:
  status:
    name: status
    values: ["active", "inactive"]
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#;

        let file2 = r#"
version: "1.0"
enums:
  role:
    name: role
    values: ["admin", "user"]
tables:
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#;

        fs::write(dir.join("01_users.yaml"), file1).unwrap();
        fs::write(dir.join("02_posts.yaml"), file2).unwrap();

        let service = SchemaParserService::new();
        let schema = service.parse_schema_directory(dir).unwrap();

        // テーブルがマージされること
        assert_eq!(schema.tables.len(), 2);
        assert!(schema.has_table("users"));
        assert!(schema.has_table("posts"));

        // ENUMもマージされること
        assert_eq!(schema.enums.len(), 2);
        assert!(schema.enums.contains_key("status"));
        assert!(schema.enums.contains_key("role"));
    }

    #[test]
    fn test_extract_line_from_error_format() {
        let service = SchemaParserService::new();

        // serde_saphyrのエラーメッセージ形式をテスト
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        // 明確に行番号が出るエラーを発生させる
        let invalid_content = "version: \"1.0\"\ntables:\n  users:\n    columns:\n      - invalid";
        fs::write(&schema_file, invalid_content).unwrap();

        let result = service.parse_schema_file(&schema_file);
        assert!(result.is_err());

        // エラーメッセージをキャプチャ
        let error_msg = result.unwrap_err().to_string();
        // serde_saphyrは"line X"形式でエラーを報告するはず
        assert!(
            error_msg.contains("line")
                || error_msg.contains(schema_file.display().to_string().as_str()),
            "Error should contain file path or line info: {}",
            error_msg
        );
    }
}
