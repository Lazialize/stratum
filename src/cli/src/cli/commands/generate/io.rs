use super::{DiffValidationResult, GenerateCommand, GenerateCommandHandler, GeneratedSql};
use crate::cli::command_context::CommandContext;
use crate::cli::commands::migration_loader;
use crate::core::config::Config;
use crate::core::schema::Schema;
use crate::services::schema_checksum::SchemaChecksumService;
use crate::services::schema_io::schema_parser::SchemaParserService;
use crate::services::schema_io::schema_serializer::SchemaSerializerService;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

impl GenerateCommandHandler {
    /// スキーマの読み込み
    ///
    /// `schema_dir_override` が指定されている場合はそちらを優先する。
    /// 指定されていない場合は設定ファイルのschema_dirを使用する。
    pub(super) fn load_schemas(
        &self,
        context: &CommandContext,
        project_path: &Path,
        config: &Config,
        schema_dir_override: Option<&PathBuf>,
    ) -> Result<(Schema, Schema)> {
        let schema_dir = if let Some(override_dir) = schema_dir_override {
            if !override_dir.exists() {
                return Err(anyhow::anyhow!(
                    "Schema directory not found: {:?}",
                    override_dir
                ));
            }
            override_dir.clone()
        } else {
            context.require_schema_dir()?
        };
        let parser = SchemaParserService::new();
        let current_schema = parser
            .parse_schema_directory(&schema_dir)
            .with_context(|| "Failed to read schema")?;
        let previous_schema = self.load_previous_schema(project_path, config)?;
        Ok((current_schema, previous_schema))
    }

    /// 前回のスキーマ状態を読み込む
    ///
    /// マイグレーションディレクトリ内のper-migrationスナップショットから前回のスキーマを復元する。
    /// 最新のマイグレーションディレクトリにある `.schema_snapshot.yaml` を優先的に使用し、
    /// 存在しない場合はグローバルスナップショットにフォールバックする。
    /// これにより、失敗したマイグレーションのディレクトリが削除された場合でも
    /// 正しいスキーマ状態を復元できる。
    pub(super) fn load_previous_schema(
        &self,
        project_path: &Path,
        config: &Config,
    ) -> Result<Schema> {
        let migrations_dir = project_path.join(&config.migrations_dir);
        let parser = SchemaParserService::new();

        // マイグレーションディレクトリが存在する場合、per-migrationスナップショットを探す
        if migrations_dir.exists() {
            let migrations = migration_loader::load_available_migrations(&migrations_dir)
                .with_context(|| {
                    format!(
                        "Failed to load available migrations from: {:?}",
                        migrations_dir
                    )
                })?;

            // 最新のマイグレーションから順にper-migrationスナップショットを探す
            for (_version, _description, migration_path) in migrations.iter().rev() {
                let per_migration_snapshot = migration_path.join(".schema_snapshot.yaml");
                if per_migration_snapshot.exists() {
                    debug!(
                        snapshot = %per_migration_snapshot.display(),
                        "Loading previous schema from per-migration snapshot"
                    );
                    return parser
                        .parse_schema_file(&per_migration_snapshot)
                        .with_context(|| {
                            format!(
                                "Failed to parse per-migration schema snapshot: {:?}",
                                per_migration_snapshot
                            )
                        });
                }
            }
        }

        // per-migrationスナップショットが見つからない場合、グローバルスナップショットにフォールバック
        let global_snapshot_path = migrations_dir.join(".schema_snapshot.yaml");
        if global_snapshot_path.exists() {
            debug!("Falling back to global schema snapshot");
            return parser
                .parse_schema_file(&global_snapshot_path)
                .with_context(|| "Failed to parse schema snapshot");
        }

        // 初回の場合は空のスキーマを返す
        debug!("No schema snapshot found, using empty schema");
        Ok(Schema::new("1.0".to_string()))
    }

    /// マイグレーションディレクトリ内にスキーマスナップショットを保存
    ///
    /// 各マイグレーションディレクトリに `.schema_snapshot.yaml` を保存することで、
    /// マイグレーションディレクトリが削除された場合にも正しいスキーマ状態を復元できる。
    fn save_migration_schema_snapshot(&self, migration_dir: &Path, schema: &Schema) -> Result<()> {
        let snapshot_path = migration_dir.join(".schema_snapshot.yaml");

        let serializer = SchemaSerializerService::new();
        let yaml = serializer
            .serialize_to_string(schema)
            .with_context(|| "Failed to serialize schema for per-migration snapshot")?;

        fs::write(&snapshot_path, yaml).with_context(|| {
            format!(
                "Failed to write per-migration schema snapshot: {:?}",
                snapshot_path
            )
        })?;

        Ok(())
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

        // per-migrationスナップショット保存（マイグレーションディレクトリ内）
        self.save_migration_schema_snapshot(&migration_dir, current_schema)?;

        // グローバルスナップショット保存（後方互換性のため維持）
        self.save_current_schema(&command.project_path, config, current_schema)?;

        Ok((dvr.migration_name.clone(), migration_dir))
    }
}
