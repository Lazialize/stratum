use super::{DiffValidationResult, GenerateCommand, GenerateCommandHandler, GeneratedSql};
use crate::cli::command_context::CommandContext;
use crate::core::config::Config;
use crate::core::schema::Schema;
use crate::services::schema_checksum::SchemaChecksumService;
use crate::services::schema_io::schema_parser::SchemaParserService;
use crate::services::schema_io::schema_serializer::SchemaSerializerService;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

impl GenerateCommandHandler {
    /// スキーマの読み込み
    pub(super) fn load_schemas(
        &self,
        context: &CommandContext,
        project_path: &Path,
        config: &Config,
    ) -> Result<(Schema, Schema)> {
        let schema_dir = context.require_schema_dir()?;
        let parser = SchemaParserService::new();
        let current_schema = parser
            .parse_schema_directory(&schema_dir)
            .with_context(|| "Failed to read schema")?;
        let previous_schema = self.load_previous_schema(project_path, config)?;
        Ok((current_schema, previous_schema))
    }

    /// 前回のスキーマ状態を読み込む
    pub(super) fn load_previous_schema(&self, project_path: &Path, config: &Config) -> Result<Schema> {
        let snapshot_path = project_path
            .join(&config.migrations_dir)
            .join(".schema_snapshot.yaml");

        if !snapshot_path.exists() {
            // 初回の場合は空のスキーマを返す
            return Ok(Schema::new("1.0".to_string()));
        }

        // SchemaParserServiceを使って新構文形式のスナップショットを読み込む
        let parser = SchemaParserService::new();
        parser
            .parse_schema_file(&snapshot_path)
            .with_context(|| "Failed to parse schema snapshot")
    }

    /// 現在のスキーマを保存（新構文形式を使用）
    pub(super) fn save_current_schema(
        &self,
        project_path: &Path,
        config: &Config,
        schema: &Schema,
    ) -> Result<()> {
        let snapshot_path = project_path
            .join(&config.migrations_dir)
            .join(".schema_snapshot.yaml");

        // SchemaSerializerServiceを使用して新構文形式でシリアライズ
        let serializer = SchemaSerializerService::new();
        let yaml = serializer
            .serialize_to_string(schema)
            .with_context(|| "Failed to serialize schema")?;

        fs::write(&snapshot_path, yaml)
            .with_context(|| format!("Failed to write schema snapshot: {:?}", snapshot_path))?;

        Ok(())
    }

    /// マイグレーションファイルの書き出し
    pub(super) fn write_migration_files(
        &self,
        context: &CommandContext,
        config: &Config,
        dvr: &DiffValidationResult,
        generated: &GeneratedSql,
        current_schema: &Schema,
        command: &GenerateCommand,
    ) -> Result<(String, PathBuf)> {
        let migrations_dir = context.migrations_dir();
        let migration_dir = migrations_dir.join(&dvr.migration_name);
        fs::create_dir_all(&migration_dir).with_context(|| {
            format!("Failed to create migration directory: {:?}", migration_dir)
        })?;

        // UP SQL
        let up_sql_path = migration_dir.join("up.sql");
        fs::write(&up_sql_path, &generated.up_sql)
            .with_context(|| format!("Failed to write up.sql: {:?}", up_sql_path))?;

        // DOWN SQL
        let down_sql_path = migration_dir.join("down.sql");
        fs::write(&down_sql_path, &generated.down_sql)
            .with_context(|| format!("Failed to write down.sql: {:?}", down_sql_path))?;

        // チェックサム・メタデータ
        let checksum_calculator = SchemaChecksumService::new();
        let checksum = checksum_calculator.calculate_checksum(current_schema);

        let metadata = self.services.generator.generate_migration_metadata(
            &dvr.timestamp,
            &dvr.sanitized_description,
            config.dialect,
            &checksum,
            dvr.destructive_report.clone(),
        )?;
        let meta_path = migration_dir.join(".meta.yaml");
        fs::write(&meta_path, metadata)
            .with_context(|| format!("Failed to write metadata: {:?}", meta_path))?;

        // スキーマスナップショット保存
        self.save_current_schema(&command.project_path, config, current_schema)?;

        Ok((dvr.migration_name.clone(), migration_dir))
    }
}
