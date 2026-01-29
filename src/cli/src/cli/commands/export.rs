// exportコマンドハンドラー
//
// スキーマのエクスポート機能を実装します。
// 責務は以下の3層に分離されています:
// - DB introspection: DatabaseIntrospector（adapters層）
// - 変換: SchemaConversionService（services層）
// - 出力: このモジュール（CLI層、YAMLシリアライズとファイル/標準出力）

use crate::adapters::database_introspector::{create_introspector, DatabaseIntrospector};
use crate::cli::command_context::CommandContext;
use crate::cli::commands::{render_output, CommandOutput};
use crate::cli::OutputFormat;
use crate::core::config::Dialect;
use crate::core::schema::Schema;
use crate::services::schema_conversion::{RawTableInfo, SchemaConversionService};
use crate::services::schema_io::schema_serializer::SchemaSerializerService;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use sqlx::AnyPool;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

/// exportコマンドの出力構造体
#[derive(Debug, Clone, Serialize)]
pub struct ExportOutput {
    /// エクスポートされたテーブル一覧
    pub tables: Vec<String>,
    /// 出力先パス（Noneの場合はstdout）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    /// テキスト出力メッセージ
    #[serde(skip)]
    pub text_message: String,
}

impl CommandOutput for ExportOutput {
    fn to_text(&self) -> String {
        self.text_message.clone()
    }
}

/// exportコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct ExportCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// カスタム設定ファイルパス
    pub config_path: Option<PathBuf>,
    /// 環境名
    pub env: String,
    /// 出力先ディレクトリ（Noneの場合は標準出力）
    pub output_dir: Option<PathBuf>,
    /// 既存ファイルを確認なしで上書き
    pub force: bool,
    /// 出力フォーマット
    pub format: OutputFormat,
    /// テーブルごとに個別のYAMLファイルに分割出力
    pub split: bool,
    /// エクスポート対象のテーブル（空の場合は全テーブル）
    pub tables: Vec<String>,
    /// エクスポートから除外するテーブル
    pub exclude_tables: Vec<String>,
}

/// exportコマンドハンドラー
///
/// 責務: CLI 層のオーケストレーション
/// - 設定読み込み、DB接続、各サービスの呼び出し、出力処理
#[derive(Debug, Default)]
pub struct ExportCommandHandler {}

impl ExportCommandHandler {
    /// 新しいExportCommandHandlerを作成
    pub fn new() -> Self {
        Self {}
    }

    /// exportコマンドを実行
    ///
    /// # Arguments
    ///
    /// * `command` - exportコマンドのパラメータ
    ///
    /// # Returns
    ///
    /// 成功時はエクスポート結果のサマリー（または標準出力用のYAML）、失敗時はエラーメッセージ
    pub async fn execute(&self, command: &ExportCommand) -> Result<String> {
        // --tables と --exclude-tables の同時指定を禁止
        if !command.tables.is_empty() && !command.exclude_tables.is_empty() {
            return Err(anyhow!(
                "Cannot use --tables and --exclude-tables together."
            ));
        }

        // --split は --output と併用が必要
        if command.split && command.output_dir.is_none() {
            return Err(anyhow!(
                "--split requires --output to specify the output directory."
            ));
        }

        // 設定ファイルを読み込む
        let context = CommandContext::load_with_config(
            command.project_path.clone(),
            command.config_path.clone(),
        )?;
        let config = &context.config;

        // データベースに接続
        let pool = context.connect_pool(&command.env).await?;

        // データベースからスキーマ情報を取得
        debug!(dialect = ?config.dialect, "Extracting schema from database");
        let mut schema = self
            .extract_schema_from_database(&pool, config.dialect)
            .await
            .with_context(|| "Failed to get schema information")?;

        // テーブルフィルタリング
        self.filter_tables(&mut schema, &command.tables, &command.exclude_tables)?;

        // テーブル名のリストを取得
        let mut table_names: Vec<String> = schema.tables.keys().cloned().collect();
        table_names.sort();
        debug!(tables = table_names.len(), "Schema extracted successfully");

        let serializer = SchemaSerializerService::new();

        // 出力先に応じて処理
        if let Some(output_dir) = &command.output_dir {
            // ディレクトリに出力
            fs::create_dir_all(output_dir)
                .with_context(|| format!("Failed to create output directory: {:?}", output_dir))?;

            if command.split {
                // テーブルごとに個別ファイルに出力
                self.write_split_files(&schema, &serializer, output_dir, command.force)
                    .with_context(|| "Failed to write split schema files")?;

                let output = ExportOutput {
                    tables: table_names.clone(),
                    output_path: Some(output_dir.to_string_lossy().to_string()),
                    text_message: self.format_export_summary(&table_names, Some(output_dir), true),
                };

                render_output(&output, &command.format)
            } else {
                // 単一ファイルに出力
                let yaml_content = serializer
                    .serialize_to_string(&schema)
                    .with_context(|| "Failed to serialize schema to YAML")?;

                let output_file = output_dir.join("schema.yaml");

                // 上書き確認
                if output_file.exists() && !command.force {
                    return Err(anyhow!(
                        "Output file already exists: {:?}\nUse --force to overwrite.",
                        output_file
                    ));
                }

                fs::write(&output_file, &yaml_content)
                    .with_context(|| format!("Failed to write schema file: {:?}", output_file))?;

                let output = ExportOutput {
                    tables: table_names.clone(),
                    output_path: Some(output_file.to_string_lossy().to_string()),
                    text_message: self.format_export_summary(&table_names, Some(output_dir), false),
                };

                render_output(&output, &command.format)
            }
        } else {
            // 標準出力に出力
            let yaml_content = serializer
                .serialize_to_string(&schema)
                .with_context(|| "Failed to serialize schema to YAML")?;

            let output = ExportOutput {
                tables: table_names,
                output_path: None,
                text_message: yaml_content,
            };

            render_output(&output, &command.format)
        }
    }

    /// テーブルフィルタリングを適用
    fn filter_tables(
        &self,
        schema: &mut Schema,
        tables: &[String],
        exclude_tables: &[String],
    ) -> Result<()> {
        if !tables.is_empty() {
            // --tables: 指定テーブルのみ残す
            let include_set: HashSet<&str> = tables.iter().map(|s| s.as_str()).collect();

            // 指定されたテーブルが存在するか確認
            for name in tables {
                if !schema.tables.contains_key(name) {
                    return Err(anyhow!("Table '{}' not found in database.", name));
                }
            }

            schema
                .tables
                .retain(|name, _| include_set.contains(name.as_str()));
        } else if !exclude_tables.is_empty() {
            // --exclude-tables: 指定テーブルを除外
            let exclude_set: HashSet<&str> = exclude_tables.iter().map(|s| s.as_str()).collect();

            // 指定されたテーブルが存在するか確認
            for name in exclude_tables {
                if !schema.tables.contains_key(name) {
                    return Err(anyhow!("Table '{}' not found in database.", name));
                }
            }

            schema
                .tables
                .retain(|name, _| !exclude_set.contains(name.as_str()));
        }

        Ok(())
    }

    /// テーブルごとに個別YAMLファイルに出力
    ///
    /// --force でない場合、書き込みを開始する前に全出力ファイルの存在を確認し、
    /// 一部だけ書き換わる不整合状態を防ぎます。
    fn write_split_files(
        &self,
        schema: &Schema,
        serializer: &SchemaSerializerService,
        output_dir: &Path,
        force: bool,
    ) -> Result<()> {
        // テーブル名でソートして安定した出力順序を保証
        let mut table_names: Vec<&String> = schema.tables.keys().collect();
        table_names.sort();

        // --force でない場合、書き込み前に全ファイルの存在を一括チェック
        if !force {
            let mut existing_files = Vec::new();
            for table_name in &table_names {
                let output_file = output_dir.join(format!("{}.yaml", table_name));
                if output_file.exists() {
                    existing_files.push(output_file);
                }
            }
            if !existing_files.is_empty() {
                let file_list: Vec<String> = existing_files
                    .iter()
                    .map(|f| format!("  - {:?}", f))
                    .collect();
                return Err(anyhow!(
                    "Output files already exist:\n{}\nUse --force to overwrite.",
                    file_list.join("\n")
                ));
            }
        }

        for table_name in table_names {
            let table = schema.tables.get(table_name).unwrap();

            // テーブル単体のSchemaを作成
            let mut single_schema = Schema::new(schema.version.clone());
            single_schema.enum_recreate_allowed = schema.enum_recreate_allowed;
            single_schema.enums = schema.enums.clone();
            single_schema.add_table(table.clone());

            let yaml_content = serializer
                .serialize_to_string(&single_schema)
                .with_context(|| format!("Failed to serialize table '{}' to YAML", table_name))?;

            let output_file = output_dir.join(format!("{}.yaml", table_name));

            fs::write(&output_file, &yaml_content)
                .with_context(|| format!("Failed to write schema file: {:?}", output_file))?;

            debug!(table = table_name, file = ?output_file, "Wrote split schema file");
        }

        Ok(())
    }

    /// データベースからスキーマ情報を抽出
    ///
    /// DatabaseIntrospector と SchemaConversionService を使用して
    /// データベースからスキーマ情報を取得し、内部モデルに変換します。
    async fn extract_schema_from_database(
        &self,
        pool: &AnyPool,
        dialect: Dialect,
    ) -> Result<Schema> {
        // イントロスペクターを作成
        let introspector = create_introspector(dialect);

        // ENUM定義を取得（PostgreSQLのみ）
        let raw_enums = introspector
            .get_enums(pool)
            .await
            .with_context(|| "Failed to get ENUM definitions")?;

        // ENUM名のセットを作成（型変換で使用）
        let enum_names: HashSet<String> = raw_enums.iter().map(|e| e.name.clone()).collect();

        // 変換サービスを作成
        let conversion_service = SchemaConversionService::new(dialect).with_enum_names(enum_names);

        // テーブル一覧を取得
        let table_names = introspector
            .get_table_names(pool)
            .await
            .with_context(|| "Failed to get table names")?;

        // 各テーブルの情報を取得
        let mut raw_tables = Vec::new();
        for table_name in table_names {
            let raw_table = self
                .get_raw_table_info(introspector.as_ref(), pool, &table_name)
                .await
                .with_context(|| format!("Failed to get table info for '{}'", table_name))?;
            raw_tables.push(raw_table);
        }

        // スキーマを構築
        conversion_service
            .build_schema(raw_tables, raw_enums)
            .with_context(|| "Failed to build schema from raw data")
    }

    /// 単一テーブルの生情報を取得
    async fn get_raw_table_info(
        &self,
        introspector: &dyn DatabaseIntrospector,
        pool: &AnyPool,
        table_name: &str,
    ) -> Result<RawTableInfo> {
        let columns = introspector
            .get_columns(pool, table_name)
            .await
            .with_context(|| format!("Failed to get columns for '{}'", table_name))?;

        let indexes = introspector
            .get_indexes(pool, table_name)
            .await
            .with_context(|| format!("Failed to get indexes for '{}'", table_name))?;

        let constraints = introspector
            .get_constraints(pool, table_name)
            .await
            .with_context(|| format!("Failed to get constraints for '{}'", table_name))?;

        Ok(RawTableInfo {
            name: table_name.to_string(),
            columns,
            indexes,
            constraints,
        })
    }

    /// エクスポート結果のサマリーをフォーマット
    pub fn format_export_summary(
        &self,
        table_names: &[String],
        output_dir: Option<&PathBuf>,
        split: bool,
    ) -> String {
        let mut output = String::new();

        output.push_str("=== Schema Export Complete ===\n\n");

        output.push_str(&format!("Exported tables: {}\n\n", table_names.len()));

        for table_name in table_names {
            output.push_str(&format!("  - {}\n", table_name));
        }

        output.push('\n');

        if let Some(dir) = output_dir {
            if split {
                output.push_str(&format!("Output: {:?} (split mode)\n", dir));
                for table_name in table_names {
                    output.push_str(&format!("  - {}.yaml\n", table_name));
                }
            } else {
                output.push_str(&format!("Output: {:?}\n", dir.join("schema.yaml")));
            }
        } else {
            output.push_str("Output: stdout\n");
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_handler() {
        let handler = ExportCommandHandler::new();
        assert!(format!("{:?}", handler).contains("ExportCommandHandler"));
    }

    // ======================================
    // Task 4.1: 新構文形式でのシリアライズテスト
    // ======================================

    #[test]
    fn test_serialize_schema_new_syntax_format() {
        use crate::core::schema::{Column, ColumnType, Constraint, Index, Table};
        use crate::services::schema_io::schema_serializer::SchemaSerializerService;

        // 内部モデルを作成
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            true,
        ));
        schema.add_table(table);

        // シリアライザーサービスを使用してシリアライズ
        let serializer = SchemaSerializerService::new();
        let yaml = serializer.serialize_to_string(&schema).unwrap();

        // 新構文形式の確認
        // 1. テーブル名がキーとして出力される
        assert!(yaml.contains("users:"));
        // 2. nameフィールドは出力されない
        assert!(!yaml.contains("name: users"));
        // 3. primary_keyフィールドが出力される
        assert!(yaml.contains("primary_key:"));
        // 4. constraints内にPRIMARY_KEYは含まれない
        assert!(!yaml.contains("type: PRIMARY_KEY"));
    }

    #[test]
    fn test_format_export_summary() {
        let handler = ExportCommandHandler::new();

        let table_names = vec!["users".to_string(), "posts".to_string()];
        let output_path = Some(PathBuf::from("/test/output"));

        let summary = handler.format_export_summary(&table_names, output_path.as_ref(), false);

        assert!(summary.contains("Export Complete"));
        assert!(summary.contains("2"));
        assert!(summary.contains("users"));
        assert!(summary.contains("posts"));
        assert!(summary.contains("schema.yaml"));
    }

    #[test]
    fn test_format_export_summary_stdout() {
        let handler = ExportCommandHandler::new();

        let table_names = vec!["users".to_string()];

        let summary = handler.format_export_summary(&table_names, None, false);

        assert!(summary.contains("stdout"));
    }

    #[test]
    fn test_format_export_summary_split() {
        let handler = ExportCommandHandler::new();

        let table_names = vec!["users".to_string(), "posts".to_string()];
        let output_path = Some(PathBuf::from("/test/output"));

        let summary = handler.format_export_summary(&table_names, output_path.as_ref(), true);

        assert!(summary.contains("Export Complete"));
        assert!(summary.contains("split mode"));
        assert!(summary.contains("users.yaml"));
        assert!(summary.contains("posts.yaml"));
    }

    #[test]
    fn test_filter_tables_include() {
        use crate::core::schema::Table;

        let handler = ExportCommandHandler::new();
        let mut schema = Schema::new("1.0".to_string());
        schema.add_table(Table::new("users".to_string()));
        schema.add_table(Table::new("posts".to_string()));
        schema.add_table(Table::new("comments".to_string()));

        handler
            .filter_tables(
                &mut schema,
                &vec!["users".to_string(), "posts".to_string()],
                &vec![],
            )
            .unwrap();

        assert_eq!(schema.tables.len(), 2);
        assert!(schema.tables.contains_key("users"));
        assert!(schema.tables.contains_key("posts"));
        assert!(!schema.tables.contains_key("comments"));
    }

    #[test]
    fn test_filter_tables_exclude() {
        use crate::core::schema::Table;
        let handler = ExportCommandHandler::new();
        let mut schema = Schema::new("1.0".to_string());
        schema.add_table(Table::new("users".to_string()));
        schema.add_table(Table::new("posts".to_string()));
        schema.add_table(Table::new("comments".to_string()));

        handler
            .filter_tables(&mut schema, &vec![], &vec!["comments".to_string()])
            .unwrap();

        assert_eq!(schema.tables.len(), 2);
        assert!(schema.tables.contains_key("users"));
        assert!(schema.tables.contains_key("posts"));
        assert!(!schema.tables.contains_key("comments"));
    }

    #[test]
    fn test_filter_tables_nonexistent_table_error() {
        use crate::core::schema::Table;
        let handler = ExportCommandHandler::new();
        let mut schema = Schema::new("1.0".to_string());
        schema.add_table(Table::new("users".to_string()));

        let result = handler.filter_tables(&mut schema, &vec!["nonexistent".to_string()], &vec![]);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nonexistent"));
    }

    #[test]
    fn test_filter_tables_no_filter() {
        use crate::core::schema::Table;
        let handler = ExportCommandHandler::new();
        let mut schema = Schema::new("1.0".to_string());
        schema.add_table(Table::new("users".to_string()));
        schema.add_table(Table::new("posts".to_string()));

        handler
            .filter_tables(&mut schema, &vec![], &vec![])
            .unwrap();

        assert_eq!(schema.tables.len(), 2);
    }

    #[test]
    fn test_write_split_files_creates_per_table_files() {
        use crate::core::schema::Table;
        use crate::services::schema_io::schema_serializer::SchemaSerializerService;
        use tempfile::TempDir;

        let handler = ExportCommandHandler::new();
        let serializer = SchemaSerializerService::new();
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().to_path_buf();

        let mut schema = Schema::new("1.0".to_string());
        schema.add_table(Table::new("users".to_string()));
        schema.add_table(Table::new("posts".to_string()));

        handler
            .write_split_files(&schema, &serializer, &output_dir, false)
            .unwrap();

        assert!(output_dir.join("users.yaml").exists());
        assert!(output_dir.join("posts.yaml").exists());
        // schema.yaml は作成されない
        assert!(!output_dir.join("schema.yaml").exists());
    }

    #[test]
    fn test_write_split_files_rejects_existing_before_any_write() {
        use crate::core::schema::Table;
        use crate::services::schema_io::schema_serializer::SchemaSerializerService;
        use tempfile::TempDir;

        let handler = ExportCommandHandler::new();
        let serializer = SchemaSerializerService::new();
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().to_path_buf();

        let mut schema = Schema::new("1.0".to_string());
        schema.add_table(Table::new("aaa".to_string()));
        schema.add_table(Table::new("zzz".to_string()));

        // ソート順で後ろに来る zzz.yaml だけ事前に作成
        fs::write(output_dir.join("zzz.yaml"), "existing").unwrap();

        let result = handler.write_split_files(&schema, &serializer, &output_dir, false);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("zzz.yaml"));

        // aaa.yaml はまだ書き込まれていないことを確認（一括チェックが先行）
        assert!(!output_dir.join("aaa.yaml").exists());
    }

    #[test]
    fn test_write_split_files_force_overwrites_existing() {
        use crate::core::schema::Table;
        use crate::services::schema_io::schema_serializer::SchemaSerializerService;
        use tempfile::TempDir;

        let handler = ExportCommandHandler::new();
        let serializer = SchemaSerializerService::new();
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().to_path_buf();

        let mut schema = Schema::new("1.0".to_string());
        schema.add_table(Table::new("users".to_string()));

        // 事前にファイルを作成
        fs::write(output_dir.join("users.yaml"), "old content").unwrap();

        // --force で上書き成功
        handler
            .write_split_files(&schema, &serializer, &output_dir, true)
            .unwrap();

        let content = fs::read_to_string(output_dir.join("users.yaml")).unwrap();
        // 上書きされて YAML 形式になっている
        assert!(content.contains("version:"));
    }

    #[test]
    fn test_export_output_json_serialization() {
        let output = ExportOutput {
            tables: vec!["users".to_string(), "posts".to_string()],
            output_path: Some("/output/schema.yaml".to_string()),
            text_message: "should not appear in JSON".to_string(),
        };

        let json = serde_json::to_string_pretty(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // text_message は #[serde(skip)] のため含まれない
        assert!(parsed.get("text_message").is_none());
        assert_eq!(parsed["tables"][0], "users");
        assert_eq!(parsed["tables"][1], "posts");
        assert_eq!(parsed["output_path"], "/output/schema.yaml");

        // output_path が None の場合はフィールドがスキップされる
        let output_no_path = ExportOutput {
            tables: vec!["users".to_string()],
            output_path: None,
            text_message: "text".to_string(),
        };
        let json2 = serde_json::to_string_pretty(&output_no_path).unwrap();
        let parsed2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        assert!(parsed2.get("output_path").is_none());
    }
}
