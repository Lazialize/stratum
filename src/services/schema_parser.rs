// スキーマパーサーサービス
//
// YAMLスキーマファイルの読み込み、解析、マージ処理を行うサービス。
// ディレクトリ全体のスキーマファイルをスキャンし、統合されたスキーマを生成します。

use crate::core::error::IoError;
use crate::core::schema::Schema;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// スキーマパーサーサービス
///
/// YAMLスキーマファイルの解析とマージを行います。
#[derive(Debug, Clone)]
pub struct SchemaParserService {
    // 将来的な拡張のためのフィールドを予約
}

impl SchemaParserService {
    /// 新しいSchemaParserServiceを作成
    pub fn new() -> Self {
        Self {}
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
            return Err(
                IoError::FileNotFound {
                    path: schema_dir.display().to_string(),
                }
                .into(),
            );
        }

        if !schema_dir.is_dir() {
            return Err(anyhow::anyhow!(
                "指定されたパスはディレクトリではありません: {}",
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
            let schema = self.parse_schema_file(&file_path).with_context(|| {
                format!("スキーマファイルの解析に失敗しました: {:?}", file_path)
            })?;

            // バージョンを保持（最初に見つかったバージョンを使用）
            if merged_schema.table_count() == 0 {
                merged_schema.version = schema.version;
            }

            // テーブルをマージ
            for (table_name, table) in schema.tables {
                merged_schema.tables.insert(table_name, table);
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
            return Err(
                IoError::FileNotFound {
                    path: file_path.display().to_string(),
                }
                .into(),
            );
        }

        // ファイル内容を読み込み
        let content = fs::read_to_string(file_path).map_err(|e| IoError::FileRead {
            path: file_path.display().to_string(),
            cause: e.to_string(),
        })?;

        // YAMLを解析
        let schema: Schema = serde_saphyr::from_str(&content)
            .with_context(|| format!("YAMLの解析に失敗しました: {:?}", file_path))?;

        Ok(schema)
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
    use std::io::Write;
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
        assert!(error_message.contains("ファイルが見つかりません"));
    }

    #[test]
    fn test_parse_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let service = SchemaParserService::new();
        let schema = service.parse_schema_directory(temp_dir.path()).unwrap();

        // 空のディレクトリからは空のスキーマが返される
        assert_eq!(schema.tables.len(), 0);
        assert_eq!(schema.version, "1.0");
    }

    #[test]
    fn test_parse_valid_schema_file() {
        let temp_dir = TempDir::new().unwrap();
        let schema_file = temp_dir.path().join("schema.yaml");

        let schema_content = r#"
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
    indexes: []
    constraints: []
"#;
        fs::write(&schema_file, schema_content).unwrap();

        let service = SchemaParserService::new();
        let schema = service.parse_schema_file(&schema_file).unwrap();

        assert_eq!(schema.version, "1.0");
        assert_eq!(schema.tables.len(), 1);
        assert!(schema.has_table("users"));
    }
}
