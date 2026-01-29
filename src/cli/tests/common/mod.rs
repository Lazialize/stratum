// CLIテスト共通ヘルパー
//
// テスト全体で共有されるユーティリティ関数を集約する。
// テストファイルから `mod common;` で利用可能。

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use strata::core::config::{Config, DatabaseConfig, Dialect};
use strata::services::config_serializer::ConfigSerializer;
use tempfile::TempDir;

/// テスト用のConfig作成ヘルパー
pub fn create_test_config(dialect: Dialect, database_path: Option<&str>) -> Config {
    let mut environments = HashMap::new();

    let db_config = DatabaseConfig {
        host: String::new(),
        database: database_path.unwrap_or(":memory:").to_string(),
        ..Default::default()
    };

    environments.insert("development".to_string(), db_config);

    Config {
        version: "1.0".to_string(),
        dialect,
        schema_dir: PathBuf::from("schema"),
        migrations_dir: PathBuf::from("migrations"),
        environments,
    }
}

/// テスト用のプロジェクトディレクトリを作成
#[allow(dead_code)]
pub fn setup_test_project(
    dialect: Dialect,
    database_path: Option<&str>,
    create_migrations: bool,
) -> Result<(TempDir, PathBuf)> {
    let temp_dir = TempDir::new()?;
    let project_path = temp_dir.path().to_path_buf();

    // 設定ファイルを作成
    let config = create_test_config(dialect, database_path);
    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    let config_yaml = ConfigSerializer::to_yaml(&config)?;
    fs::write(&config_path, config_yaml)?;

    // スキーマディレクトリを作成
    fs::create_dir_all(project_path.join("schema"))?;

    // マイグレーションディレクトリを作成（必要な場合）
    if create_migrations {
        fs::create_dir_all(project_path.join("migrations"))?;
    }

    Ok((temp_dir, project_path))
}

/// テスト用のマイグレーションファイルを作成
#[allow(dead_code)]
pub fn create_test_migration(
    project_path: &Path,
    version: &str,
    description: &str,
    up_sql: &str,
    down_sql: &str,
    checksum: &str,
) -> Result<()> {
    let migration_dir = project_path
        .join("migrations")
        .join(format!("{}_{}", version, description));
    fs::create_dir_all(&migration_dir)?;

    fs::write(migration_dir.join("up.sql"), up_sql)?;
    fs::write(migration_dir.join("down.sql"), down_sql)?;

    let meta = format!(
        "version: \"{}\"\ndescription: \"{}\"\ndialect: \"sqlite\"\nchecksum: \"{}\"\ndestructive_changes: {{}}\n",
        version, description, checksum
    );
    fs::write(migration_dir.join(".meta.yaml"), meta)?;

    Ok(())
}

/// テスト用の設定ファイルを書き込む
#[allow(dead_code)]
pub fn write_config(project_path: &Path, dialect: Dialect, db_path: Option<&str>) {
    let config = create_test_config(dialect, db_path);
    let config_path = project_path.join(Config::DEFAULT_CONFIG_PATH);
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let config_yaml = ConfigSerializer::to_yaml(&config).unwrap();
    fs::write(config_path, config_yaml).unwrap();
}

/// テスト用のスキーマYAMLファイルを書き込む（単一テーブル、idカラムのみ）
#[allow(dead_code)]
pub fn write_schema_file(project_path: &Path, table_name: &str) {
    let schema_dir = project_path.join("schema");
    fs::create_dir_all(&schema_dir).unwrap();
    let schema_content = format!(
        r#"version: "1.0"
tables:
  {}:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#,
        table_name
    );
    fs::write(
        schema_dir.join(format!("{}.yaml", table_name)),
        schema_content,
    )
    .unwrap();
}

/// テスト用のスキーマスナップショットを書き込む（単一テーブル、idカラムのみ）
#[allow(dead_code)]
pub fn write_schema_snapshot(project_path: &Path, table_name: &str) {
    let migrations_dir = project_path.join("migrations");
    fs::create_dir_all(&migrations_dir).unwrap();
    let snapshot_content = format!(
        r#"version: "1.0"
tables:
  {}:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#,
        table_name
    );
    fs::write(
        migrations_dir.join(".schema_snapshot.yaml"),
        snapshot_content,
    )
    .unwrap();
}

/// YAMLスキーマから差分を検出し、パイプラインでSQL生成するヘルパー
#[allow(dead_code)]
pub fn generate_migration_sql(
    old_yaml: &str,
    new_yaml: &str,
    dialect: Dialect,
) -> (String, String) {
    let temp_dir = TempDir::new().unwrap();
    let old_path = temp_dir.path().join("old.yaml");
    let new_path = temp_dir.path().join("new.yaml");

    fs::write(&old_path, old_yaml).unwrap();
    fs::write(&new_path, new_yaml).unwrap();

    let parser = strata::services::schema_io::schema_parser::SchemaParserService::new();
    let old_schema = parser.parse_schema_file(&old_path).unwrap();
    let new_schema = parser.parse_schema_file(&new_path).unwrap();

    let detector = strata::services::schema_diff_detector::SchemaDiffDetectorService::new();
    let diff = detector.detect_diff(&old_schema, &new_schema);

    let pipeline = strata::services::migration_pipeline::MigrationPipeline::new(&diff, dialect)
        .with_schemas(&old_schema, &new_schema);

    let (up_sql, _) = pipeline.generate_up().unwrap();
    let (down_sql, _) = pipeline.generate_down().unwrap();

    (up_sql, down_sql)
}

/// テスト用マイグレーションをメタデータ付きで作成（破壊的変更メタ対応）
#[allow(dead_code)]
pub fn create_migration_with_meta(
    project_path: &Path,
    version: &str,
    description: &str,
    up_sql: &str,
    destructive_meta: &str,
) -> PathBuf {
    let migration_dir = project_path
        .join("migrations")
        .join(format!("{}_{}", version, description));
    fs::create_dir_all(&migration_dir).unwrap();
    fs::write(migration_dir.join("up.sql"), up_sql).unwrap();
    fs::write(migration_dir.join("down.sql"), "--").unwrap();
    fs::write(migration_dir.join(".meta.yaml"), destructive_meta).unwrap();
    migration_dir
}
