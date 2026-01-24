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
use crate::services::schema_serializer::SchemaSerializerService;
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

/// generateコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct GenerateCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// マイグレーションの説明（オプション）
    pub description: Option<String>,
    /// ドライラン（SQLを表示するがファイルは作成しない）
    pub dry_run: bool,
}

/// 型変更情報
#[derive(Debug)]
struct TypeChangeInfo {
    table: String,
    column: String,
    old_type: String,
    new_type: String,
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
                "Config file not found: {:?}. Please initialize the project first with the `init` command.",
                config_path
            ));
        }

        let config =
            Config::from_file(&config_path).with_context(|| "Failed to read config file")?;

        // スキーマディレクトリのパスを解決
        let schema_dir = command.project_path.join(&config.schema_dir);
        if !schema_dir.exists() {
            return Err(anyhow!("Schema directory not found: {:?}", schema_dir));
        }

        // 現在のスキーマを読み込む
        let parser = SchemaParserService::new();
        let current_schema = parser
            .parse_schema_directory(&schema_dir)
            .with_context(|| "Failed to read schema")?;

        // 前回のスキーマ状態を読み込む（存在しない場合は空のスキーマ）
        let previous_schema = self.load_previous_schema(&command.project_path, &config)?;

        // 差分を検出
        let detector = SchemaDiffDetector::new();
        let diff = detector.detect_diff(&previous_schema, &current_schema);

        // 差分がない場合はエラー
        if diff.is_empty() {
            return Err(anyhow!(
                "No schema changes found. No migration files were generated."
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
        let migration_name =
            generator.generate_migration_filename(&timestamp, &sanitized_description);

        // スキーマ付きでSQLを生成（型変更検証を含む）
        let sql_result = generator.generate_up_sql_with_schemas(
            &diff,
            &previous_schema,
            &current_schema,
            config.dialect,
        );

        // 型変更検証エラーの処理
        if let Err(e) = &sql_result {
            if command.dry_run {
                // dry-runモードではエラーを色付きで表示
                return self.execute_dry_run_with_error(&migration_name, e, &diff);
            }
            return Err(anyhow::anyhow!("{}", e));
        }

        let (up_sql, validation_result) = sql_result.unwrap();

        let (down_sql, _) = generator
            .generate_down_sql_with_schemas(
                &diff,
                &previous_schema,
                &current_schema,
                config.dialect,
            )
            .map_err(|e| anyhow::anyhow!("Failed to generate DOWN SQL: {}", e))?;

        // dry-runモードの場合はSQLを表示して終了
        if command.dry_run {
            return self.execute_dry_run(
                &migration_name,
                &up_sql,
                &down_sql,
                &diff,
                &validation_result,
            );
        }

        // Create migration directory
        let migrations_dir = command.project_path.join(&config.migrations_dir);
        let migration_dir = migrations_dir.join(&migration_name);
        fs::create_dir_all(&migration_dir).with_context(|| {
            format!("Failed to create migration directory: {:?}", migration_dir)
        })?;

        // UP SQLを書き込み
        let up_sql_path = migration_dir.join("up.sql");
        fs::write(&up_sql_path, &up_sql)
            .with_context(|| format!("Failed to write up.sql: {:?}", up_sql_path))?;

        // DOWN SQLを書き込み
        let down_sql_path = migration_dir.join("down.sql");
        fs::write(&down_sql_path, &down_sql)
            .with_context(|| format!("Failed to write down.sql: {:?}", down_sql_path))?;

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
            .with_context(|| format!("Failed to write metadata: {:?}", meta_path))?;

        // 現在のスキーマを保存（次回の差分検出用）
        self.save_current_schema(&command.project_path, &config, &current_schema)?;

        Ok(migration_name)
    }

    /// dry-runモードの実行
    fn execute_dry_run(
        &self,
        migration_name: &str,
        up_sql: &str,
        down_sql: &str,
        diff: &crate::core::schema_diff::SchemaDiff,
        validation_result: &crate::core::error::ValidationResult,
    ) -> Result<String> {
        use std::fmt::Write;

        let mut output = String::new();

        // ヘッダー（太字）
        writeln!(
            &mut output,
            "{}",
            "=== Dry Run: Migration Preview ===".bold()
        )
        .unwrap();
        writeln!(&mut output, "Migration: {}", migration_name.cyan()).unwrap();
        writeln!(&mut output).unwrap();

        // 型変更のプレビュー
        let type_changes = self.collect_type_changes(diff);
        if !type_changes.is_empty() {
            writeln!(&mut output, "{}", "--- Type Changes ---".bold()).unwrap();
            for change in &type_changes {
                let location = format!("{}.{}", change.table, change.column).cyan();
                let arrow = "→".bold();
                writeln!(
                    &mut output,
                    "  {}: {} {} {}",
                    location, change.old_type, arrow, change.new_type
                )
                .unwrap();
            }
            writeln!(&mut output).unwrap();
        }

        // 警告の表示（黄色）
        if validation_result.warning_count() > 0 {
            writeln!(
                &mut output,
                "{}",
                format!("--- Warnings ({}) ---", validation_result.warning_count())
                    .yellow()
                    .bold()
            )
            .unwrap();
            for warning in &validation_result.warnings {
                let location = warning
                    .location
                    .as_ref()
                    .map(|loc| {
                        let table = loc.table.as_deref().unwrap_or("");
                        let column = loc
                            .column
                            .as_ref()
                            .map(|c| format!(".{}", c))
                            .unwrap_or_default();
                        format!("[{}{}]", table, column).cyan().to_string()
                    })
                    .unwrap_or_default();
                writeln!(
                    &mut output,
                    "  {} {} {}",
                    "⚠".yellow(),
                    location,
                    warning.message.yellow()
                )
                .unwrap();
            }
            writeln!(&mut output).unwrap();
        }

        // UP SQL
        writeln!(&mut output, "{}", "--- UP SQL ---".bold()).unwrap();
        writeln!(&mut output, "{}", up_sql).unwrap();
        writeln!(&mut output).unwrap();

        // DOWN SQL
        writeln!(&mut output, "{}", "--- DOWN SQL ---".bold()).unwrap();
        writeln!(&mut output, "{}", down_sql).unwrap();
        writeln!(&mut output).unwrap();

        // サマリー（太字）
        writeln!(&mut output, "{}", "=== Summary ===".bold()).unwrap();
        let warning_count = validation_result.warning_count();
        let warning_text = if warning_count > 0 {
            format!("Warnings: {}", warning_count)
                .yellow()
                .bold()
                .to_string()
        } else {
            format!("Warnings: {}", warning_count).green().to_string()
        };
        writeln!(&mut output, "{}", warning_text).unwrap();
        writeln!(
            &mut output,
            "Files would be created: 3 (up.sql, down.sql, .meta.yaml)"
        )
        .unwrap();
        writeln!(
            &mut output,
            "\n{}",
            "No files were created (dry-run mode).".dimmed()
        )
        .unwrap();

        Ok(output)
    }

    /// dry-runモードでのエラー表示
    fn execute_dry_run_with_error(
        &self,
        migration_name: &str,
        error: &str,
        diff: &crate::core::schema_diff::SchemaDiff,
    ) -> Result<String> {
        use std::fmt::Write;

        let mut output = String::new();

        // ヘッダー（太字）
        writeln!(
            &mut output,
            "{}",
            "=== Dry Run: Migration Preview ===".bold()
        )
        .unwrap();
        writeln!(&mut output, "Migration: {}", migration_name.cyan()).unwrap();
        writeln!(&mut output).unwrap();

        // 型変更のプレビュー
        let type_changes = self.collect_type_changes(diff);
        if !type_changes.is_empty() {
            writeln!(&mut output, "{}", "--- Type Changes ---".bold()).unwrap();
            for change in &type_changes {
                let location = format!("{}.{}", change.table, change.column).cyan();
                let arrow = "→".bold();
                writeln!(
                    &mut output,
                    "  {}: {} {} {}",
                    location, change.old_type, arrow, change.new_type
                )
                .unwrap();
            }
            writeln!(&mut output).unwrap();
        }

        // エラーの表示（赤色）
        writeln!(&mut output, "{}", "--- Errors ---".red().bold()).unwrap();

        // エラーメッセージをパースして表示
        for line in error.lines() {
            if line.starts_with("Type change validation failed:") {
                writeln!(&mut output, "{}", line.red()).unwrap();
            } else if line.contains("Type conversion error:") {
                // エラーから位置情報を抽出して色付け
                writeln!(&mut output, "  {} {}", "✗".red(), line.red()).unwrap();
            } else if !line.is_empty() {
                writeln!(&mut output, "  {} {}", "✗".red(), line.red()).unwrap();
            }
        }
        writeln!(&mut output).unwrap();

        // 修正提案（緑色）
        writeln!(&mut output, "{}", "--- Suggestion ---".green().bold()).unwrap();
        writeln!(
            &mut output,
            "  {}",
            "Use TEXT as an intermediate type or reconsider the type change".green()
        )
        .unwrap();
        writeln!(&mut output).unwrap();

        // サマリー（太字）
        writeln!(&mut output, "{}", "=== Summary ===".bold()).unwrap();
        writeln!(
            &mut output,
            "{}",
            "Errors: 1 (migration cannot be generated)".red().bold()
        )
        .unwrap();
        writeln!(
            &mut output,
            "\n{}",
            "Migration generation aborted due to type conversion errors.".red()
        )
        .unwrap();

        // エラー終了
        Err(anyhow::anyhow!("{}", output))
    }

    /// 型変更情報を収集
    fn collect_type_changes(
        &self,
        diff: &crate::core::schema_diff::SchemaDiff,
    ) -> Vec<TypeChangeInfo> {
        let mut changes = Vec::new();

        for table_diff in &diff.modified_tables {
            for column_diff in &table_diff.modified_columns {
                for change in &column_diff.changes {
                    if let crate::core::schema_diff::ColumnChange::TypeChanged {
                        old_type,
                        new_type,
                    } = change
                    {
                        changes.push(TypeChangeInfo {
                            table: table_diff.table_name.clone(),
                            column: column_diff.column_name.clone(),
                            old_type: old_type.clone(),
                            new_type: new_type.clone(),
                        });
                    }
                }
            }
        }

        changes
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
            .with_context(|| format!("Failed to read schema snapshot: {:?}", snapshot_path))?;

        serde_saphyr::from_str(&content).with_context(|| "Failed to parse schema snapshot")
    }

    /// 現在のスキーマを保存（新構文形式を使用）
    fn save_current_schema(
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

    /// 差分から自動的にdescriptionを生成
    fn generate_auto_description(&self, diff: &crate::core::schema_diff::SchemaDiff) -> String {
        let mut parts = Vec::new();

        if !diff.added_tables.is_empty() {
            let table_names: Vec<&str> =
                diff.added_tables.iter().map(|t| t.name.as_str()).collect();
            parts.push(format!("add tables {}", table_names.join(", ")));
        }

        if !diff.removed_tables.is_empty() {
            let removed_names: Vec<&str> = diff.removed_tables.iter().map(|s| s.as_str()).collect();
            parts.push(format!("remove tables {}", removed_names.join(", ")));
        }

        if !diff.modified_tables.is_empty() {
            let table_names: Vec<&str> = diff
                .modified_tables
                .iter()
                .map(|t| t.table_name.as_str())
                .collect();
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

    #[test]
    fn test_generate_command_has_dry_run_field() {
        let command = GenerateCommand {
            project_path: std::path::PathBuf::from("/tmp"),
            description: Some("test".to_string()),
            dry_run: true,
        };
        assert!(command.dry_run);
    }

    #[test]
    fn test_collect_type_changes() {
        use crate::core::schema::{Column, ColumnType};
        use crate::core::schema_diff::{ColumnDiff, SchemaDiff, TableDiff};

        let handler = GenerateCommandHandler::new();

        let mut diff = SchemaDiff::new();
        let old_column = Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let new_column = Column::new("age".to_string(), ColumnType::VARCHAR { length: 50 }, false);
        let column_diff = ColumnDiff::new("age".to_string(), old_column, new_column);

        let mut table_diff = TableDiff::new("users".to_string());
        table_diff.modified_columns.push(column_diff);
        diff.modified_tables.push(table_diff);

        let changes = handler.collect_type_changes(&diff);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].table, "users");
        assert_eq!(changes[0].column, "age");
    }

    #[test]
    fn test_execute_dry_run_output_format() {
        use crate::core::error::ValidationResult;
        use crate::core::schema_diff::SchemaDiff;

        let handler = GenerateCommandHandler::new();
        let diff = SchemaDiff::new();
        let validation_result = ValidationResult::new();

        let result = handler.execute_dry_run(
            "20260124120000_test",
            "CREATE TABLE users (id INTEGER);",
            "DROP TABLE users;",
            &diff,
            &validation_result,
        );

        assert!(result.is_ok());
        let output = result.unwrap();
        // ANSI escape codes may be present, so just check key content
        assert!(output.contains("Dry Run"));
        assert!(output.contains("Migration"));
        assert!(output.contains("UP SQL"));
        assert!(output.contains("DOWN SQL"));
        assert!(output.contains("Summary"));
    }

    #[test]
    fn test_execute_dry_run_with_warnings() {
        use crate::core::error::{ErrorLocation, ValidationResult, ValidationWarning};
        use crate::core::schema_diff::SchemaDiff;

        let handler = GenerateCommandHandler::new();
        let diff = SchemaDiff::new();
        let mut validation_result = ValidationResult::new();
        validation_result.add_warning(ValidationWarning::data_loss(
            "VARCHAR(255) → VARCHAR(100) may truncate data".to_string(),
            Some(ErrorLocation {
                table: Some("users".to_string()),
                column: Some("email".to_string()),
                line: None,
            }),
        ));

        let result = handler.execute_dry_run(
            "20260124120000_test",
            "ALTER TABLE users ...",
            "ALTER TABLE users ...",
            &diff,
            &validation_result,
        );

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Warnings"));
        assert!(output.contains("1"));
    }

    #[test]
    fn test_execute_dry_run_with_error() {
        use crate::core::schema::{Column, ColumnType};
        use crate::core::schema_diff::{ColumnDiff, SchemaDiff, TableDiff};

        let handler = GenerateCommandHandler::new();

        // 型変更を含むdiffを作成
        let mut diff = SchemaDiff::new();
        let old_column = Column::new("data".to_string(), ColumnType::JSONB, false);
        let new_column = Column::new(
            "data".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let column_diff = ColumnDiff::new("data".to_string(), old_column, new_column);
        let mut table_diff = TableDiff::new("documents".to_string());
        table_diff.modified_columns.push(column_diff);
        diff.modified_tables.push(table_diff);

        let error_message = "Type change validation failed:\nType conversion error: JSONB → INTEGER is not supported";

        let result =
            handler.execute_dry_run_with_error("20260124120000_test", error_message, &diff);

        // エラーとして返されることを確認
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Type Changes") || err.contains("Errors"));
    }

    // ======================================
    // Task 4.2: スナップショット保存の新構文テスト
    // ======================================

    #[test]
    fn test_snapshot_serialization_uses_new_syntax() {
        use crate::core::schema::{Column, ColumnType, Constraint, Index, Table};
        use crate::services::schema_serializer::SchemaSerializerService;

        // 内部モデルを作成
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        table.add_index(Index::new(
            "idx_name".to_string(),
            vec!["name".to_string()],
            false,
        ));
        schema.add_table(table);

        // シリアライザーサービスを使用してシリアライズ
        let serializer = SchemaSerializerService::new();
        let yaml = serializer.serialize_to_string(&schema).unwrap();

        // 新構文形式の確認
        // 1. テーブル名がキーとして出力される
        assert!(yaml.contains("products:"));
        // 2. nameフィールドは出力されない
        assert!(!yaml.contains("name: products"));
        // 3. primary_keyフィールドが出力される
        assert!(yaml.contains("primary_key:"));
        // 4. constraints内にPRIMARY_KEYは含まれない
        assert!(!yaml.contains("type: PRIMARY_KEY"));
    }
}
