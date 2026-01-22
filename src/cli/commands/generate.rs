// generateコマンドハンドラー
//
// スキーマ差分検出とマイグレーションファイル生成を実装します。
// - スキーマ定義の読み込み
// - 前回のスキーマ状態の読み込み
// - 差分検出とマイグレーションファイル生成
// - 生成されたファイルパスの表示

use crate::core::config::Config;
use crate::core::schema::Schema;
use crate::services::migration_generator::MigrationGenerator;
use crate::services::schema_checksum::SchemaChecksumService;
use crate::services::schema_diff_detector::SchemaDiffDetector;
use crate::services::schema_parser::SchemaParserService;
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// generateコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct GenerateCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// マイグレーションの説明（オプション）
    pub description: Option<String>,
}

/// generateコマンドハンドラー
#[derive(Debug, Clone)]
pub struct GenerateCommandHandler {}

impl GenerateCommandHandler {
    /// 新しいGenerateCommandHandlerを作成
    pub fn new() -> Self {
        Self {}
    }

    /// generateコマンドを実行
    ///
    /// # Arguments
    ///
    /// * `command` - generateコマンドのパラメータ
    ///
    /// # Returns
    ///
    /// 成功時は生成されたマイグレーションディレクトリのパス、失敗時はエラーメッセージ
    pub fn execute(&self, command: &GenerateCommand) -> Result<String> {
        // 設定ファイルを読み込む
        let config_path = command.project_path.join(Config::DEFAULT_CONFIG_PATH);
        if !config_path.exists() {
            return Err(anyhow!(
                "設定ファイルが見つかりません: {:?}。まず `init` コマンドでプロジェクトを初期化してください。",
                config_path
            ));
        }

        let config = Config::from_file(&config_path)
            .with_context(|| "設定ファイルの読み込みに失敗しました")?;

        // スキーマディレクトリのパスを解決
        let schema_dir = command.project_path.join(&config.schema_dir);
        if !schema_dir.exists() {
            return Err(anyhow!(
                "スキーマディレクトリが見つかりません: {:?}",
                schema_dir
            ));
        }

        // 現在のスキーマを読み込む
        let parser = SchemaParserService::new();
        let current_schema = parser
            .parse_schema_directory(&schema_dir)
            .with_context(|| "スキーマの読み込みに失敗しました")?;

        // 前回のスキーマ状態を読み込む（存在しない場合は空のスキーマ）
        let previous_schema = self.load_previous_schema(&command.project_path, &config)?;

        // 差分を検出
        let detector = SchemaDiffDetector::new();
        let diff = detector.detect_diff(&previous_schema, &current_schema);

        // 差分がない場合はエラー
        if diff.is_empty() {
            return Err(anyhow!(
                "スキーマに変更がありません。マイグレーションファイルは生成されませんでした。"
            ));
        }

        // マイグレーションを生成
        let generator = MigrationGenerator::new();
        let timestamp = generator.generate_timestamp();

        // descriptionを決定（指定されていない場合は自動生成）
        let description = command
            .description
            .clone()
            .unwrap_or_else(|| self.generate_auto_description(&diff));

        let sanitized_description = generator.sanitize_description(&description);
        let migration_name = generator.generate_migration_filename(&timestamp, &sanitized_description);

        // マイグレーションディレクトリを作成
        let migrations_dir = command.project_path.join(&config.migrations_dir);
        let migration_dir = migrations_dir.join(&migration_name);
        fs::create_dir_all(&migration_dir)
            .with_context(|| format!("マイグレーションディレクトリの作成に失敗しました: {:?}", migration_dir))?;

        // UP SQLを生成
        let up_sql = generator.generate_up_sql(&diff, config.dialect)
            .map_err(|e| anyhow::anyhow!("UP SQLの生成に失敗しました: {}", e))?;
        let up_sql_path = migration_dir.join("up.sql");
        fs::write(&up_sql_path, up_sql)
            .with_context(|| format!("up.sqlの書き込みに失敗しました: {:?}", up_sql_path))?;

        // DOWN SQLを生成
        let down_sql = generator.generate_down_sql(&diff, config.dialect)
            .map_err(|e| anyhow::anyhow!("DOWN SQLの生成に失敗しました: {}", e))?;
        let down_sql_path = migration_dir.join("down.sql");
        fs::write(&down_sql_path, down_sql)
            .with_context(|| format!("down.sqlの書き込みに失敗しました: {:?}", down_sql_path))?;

        // チェックサムを計算
        let checksum_calculator = SchemaChecksumService::new();
        let checksum = checksum_calculator.calculate_checksum(&current_schema);

        // メタデータを生成
        let metadata = generator.generate_migration_metadata(
            &timestamp,
            &sanitized_description,
            config.dialect,
            &checksum,
        );
        let meta_path = migration_dir.join(".meta.yaml");
        fs::write(&meta_path, metadata)
            .with_context(|| format!("メタデータの書き込みに失敗しました: {:?}", meta_path))?;

        // 現在のスキーマを保存（次回の差分検出用）
        self.save_current_schema(&command.project_path, &config, &current_schema)?;

        Ok(migration_name)
    }

    /// 前回のスキーマ状態を読み込む
    fn load_previous_schema(&self, project_path: &Path, config: &Config) -> Result<Schema> {
        let snapshot_path = project_path
            .join(&config.migrations_dir)
            .join(".schema_snapshot.yaml");

        if !snapshot_path.exists() {
            // 初回の場合は空のスキーマを返す
            return Ok(Schema::new("1.0".to_string()));
        }

        let content = fs::read_to_string(&snapshot_path)
            .with_context(|| format!("スキーマスナップショットの読み込みに失敗しました: {:?}", snapshot_path))?;

        serde_saphyr::from_str(&content)
            .with_context(|| "スキーマスナップショットのパースに失敗しました")
    }

    /// 現在のスキーマを保存
    fn save_current_schema(&self, project_path: &Path, config: &Config, schema: &Schema) -> Result<()> {
        let snapshot_path = project_path
            .join(&config.migrations_dir)
            .join(".schema_snapshot.yaml");

        let yaml = serde_saphyr::to_string(schema)
            .with_context(|| "スキーマのシリアライズに失敗しました")?;

        fs::write(&snapshot_path, yaml)
            .with_context(|| format!("スキーマスナップショットの書き込みに失敗しました: {:?}", snapshot_path))?;

        Ok(())
    }

    /// 差分から自動的にdescriptionを生成
    fn generate_auto_description(&self, diff: &crate::core::schema_diff::SchemaDiff) -> String {
        let mut parts = Vec::new();

        if !diff.added_tables.is_empty() {
            let table_names: Vec<String> = diff.added_tables.iter().map(|t| t.name.clone()).collect();
            parts.push(format!("add tables {}", table_names.join(", ")));
        }

        if !diff.removed_tables.is_empty() {
            parts.push(format!("remove tables {}", diff.removed_tables.join(", ")));
        }

        if !diff.modified_tables.is_empty() {
            let table_names: Vec<String> = diff.modified_tables.iter().map(|t| t.table_name.clone()).collect();
            parts.push(format!("modify tables {}", table_names.join(", ")));
        }

        if parts.is_empty() {
            "schema changes".to_string()
        } else {
            parts.join(" and ")
        }
    }
}

impl Default for GenerateCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_handler() {
        let handler = GenerateCommandHandler::new();
        assert!(format!("{:?}", handler).contains("GenerateCommandHandler"));
    }

    #[test]
    fn test_generate_auto_description() {
        use crate::core::schema::Table;
        use crate::core::schema_diff::SchemaDiff;

        let handler = GenerateCommandHandler::new();

        let mut diff = SchemaDiff::new();
        let table = Table::new("users".to_string());
        diff.added_tables.push(table);

        let description = handler.generate_auto_description(&diff);
        assert!(description.contains("users"));
    }
}
