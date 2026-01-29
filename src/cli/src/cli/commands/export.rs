// exportコマンドハンドラー
//
// スキーマのエクスポート機能を実装します。
// 責務は以下の3層に分離されています:
// - DB introspection: DatabaseIntrospector（adapters層）
// - 変換: SchemaConversionService（services層）
// - 出力: このモジュール（CLI層、YAMLシリアライズとファイル/標準出力）

use crate::adapters::database_introspector::{create_introspector, DatabaseIntrospector};
use crate::cli::OutputFormat;
use crate::cli::command_context::CommandContext;
use crate::cli::commands::{CommandOutput, render_output};
use crate::core::config::Dialect;
use crate::core::schema::Schema;
use crate::services::schema_conversion::{RawTableInfo, SchemaConversionService};
use crate::services::schema_io::schema_serializer::SchemaSerializerService;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use sqlx::AnyPool;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

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
        // 設定ファイルを読み込む
        let context = CommandContext::load_with_config(
            command.project_path.clone(),
            command.config_path.clone(),
        )?;
        let config = &context.config;

        // データベースに接続
        let pool = context.connect_pool(&command.env).await?;

        // データベースからスキーマ情報を取得
        let schema = self
            .extract_schema_from_database(&pool, config.dialect)
            .await
            .with_context(|| "Failed to get schema information")?;

        // テーブル名のリストを取得
        let table_names: Vec<String> = schema.tables.keys().cloned().collect();

        // YAML形式にシリアライズ（新構文形式を使用）
        let serializer = SchemaSerializerService::new();
        let yaml_content = serializer
            .serialize_to_string(&schema)
            .with_context(|| "Failed to serialize schema to YAML")?;

        // 出力先に応じて処理
        if let Some(output_dir) = &command.output_dir {
            // ディレクトリに出力
            fs::create_dir_all(output_dir)
                .with_context(|| format!("Failed to create output directory: {:?}", output_dir))?;

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
                text_message: self.format_export_summary(&table_names, Some(output_dir)),
            };

            render_output(&output, &command.format)
        } else {
            // 標準出力に出力
            let output = ExportOutput {
                tables: table_names,
                output_path: None,
                text_message: yaml_content,
            };

            render_output(&output, &command.format)
        }
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
    ) -> String {
        let mut output = String::new();

        output.push_str("=== Schema Export Complete ===\n\n");

        output.push_str(&format!("Exported tables: {}\n\n", table_names.len()));

        for table_name in table_names {
            output.push_str(&format!("  - {}\n", table_name));
        }

        output.push('\n');

        if let Some(dir) = output_dir {
            output.push_str(&format!("Output: {:?}\n", dir.join("schema.yaml")));
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

        let summary = handler.format_export_summary(&table_names, output_path.as_ref());

        assert!(summary.contains("Export Complete"));
        assert!(summary.contains("2"));
        assert!(summary.contains("users"));
        assert!(summary.contains("posts"));
    }

    #[test]
    fn test_format_export_summary_stdout() {
        let handler = ExportCommandHandler::new();

        let table_names = vec!["users".to_string()];

        let summary = handler.format_export_summary(&table_names, None);

        assert!(summary.contains("stdout"));
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
