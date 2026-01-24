/// カラムリネーム機能の統合テスト
///
/// YAMLスキーマからの完全なリネームフローを検証します：
/// パース → 差分検出 → 検証 → SQL生成
///
/// Task 7.1: YAMLスキーマからの完全なリネームフローテスト

use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod column_rename_integration_tests {
    use super::*;
    use strata::core::config::Dialect;
    use strata::services::migration_pipeline::MigrationPipeline;
    use strata::services::schema_diff_detector::SchemaDiffDetector;
    use strata::services::schema_parser::SchemaParserService;
    use strata::services::schema_validator::SchemaValidatorService;

    // ==========================================
    // テスト用YAMLスキーマの作成ヘルパー
    // ==========================================

    /// 旧スキーマYAMLを作成（リネーム前）
    fn create_old_schema_yaml() -> &'static str {
        r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_users_email
        columns:
          - email
        unique: true
"#
    }

    /// 新スキーマYAML（単純なリネーム）を作成
    fn create_new_schema_simple_rename_yaml() -> &'static str {
        r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: user_name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
        renamed_from: name
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_users_email
        columns:
          - email
        unique: true
"#
    }

    /// 新スキーマYAML（リネーム+型変更）を作成
    fn create_new_schema_rename_with_type_change_yaml() -> &'static str {
        r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: user_name
        type:
          kind: VARCHAR
          length: 200
        nullable: false
        renamed_from: name
      - name: email_address
        type:
          kind: VARCHAR
          length: 500
        nullable: true
        renamed_from: email
    primary_key:
      - id
    indexes:
      - name: idx_users_email
        columns:
          - email_address
        unique: true
"#
    }

    /// 新スキーマYAML（複数カラムリネーム）を作成
    fn create_new_schema_multiple_renames_yaml() -> &'static str {
        r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: full_name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
        renamed_from: name
      - name: email_address
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        renamed_from: email
    primary_key:
      - id
    indexes:
      - name: idx_users_email
        columns:
          - email_address
        unique: true
"#
    }

    /// 新スキーマYAML（無効なrenamed_from - 存在しないカラム）を作成
    fn create_new_schema_invalid_renamed_from_yaml() -> &'static str {
        r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
      - name: user_email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        renamed_from: nonexistent_column
    primary_key:
      - id
"#
    }

    // ==========================================
    // 統合テスト: パース → 差分検出 → 検証 → SQL生成
    // ==========================================

    /// 単純なカラムリネームの完全フローテスト
    #[test]
    fn test_simple_column_rename_full_flow() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        // YAMLファイル作成
        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_simple_rename_yaml()).unwrap();

        // パーサーで解析
        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        // スキーマ差分検出
        let detector = SchemaDiffDetector::new();
        let (diff, warnings) = detector.detect_diff_with_warnings(&old_schema, &new_schema);

        // リネームが検出されていることを確認
        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.renamed_columns.len(), 1);
        assert_eq!(table_diff.renamed_columns[0].old_name, "name");
        assert_eq!(table_diff.renamed_columns[0].new_column.name, "user_name");

        // リネーム済みカラムはadded/removedに含まれない
        assert!(table_diff.added_columns.is_empty());
        assert!(table_diff.removed_columns.is_empty());

        // 警告が生成されていないことを確認
        assert!(warnings.is_empty());

        // バリデーション
        let validator = SchemaValidatorService::new();
        let validation_result = validator.validate_renames(&new_schema);
        assert!(validation_result.is_valid());
    }

    /// リネーム+型変更の完全フローテスト
    #[test]
    fn test_rename_with_type_change_full_flow() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_rename_with_type_change_yaml()).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        let detector = SchemaDiffDetector::new();
        let (diff, _) = detector.detect_diff_with_warnings(&old_schema, &new_schema);

        // 2つのリネームが検出されていることを確認
        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.renamed_columns.len(), 2);

        // 型変更も含まれていることを確認
        for renamed in &table_diff.renamed_columns {
            // name → user_name (VARCHAR(100) → VARCHAR(200))
            // email → email_address (VARCHAR(255) → VARCHAR(500), nullable変更)
            assert!(!renamed.changes.is_empty());
        }
    }

    /// 複数カラムリネームの完全フローテスト
    #[test]
    fn test_multiple_column_renames_full_flow() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_multiple_renames_yaml()).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        let detector = SchemaDiffDetector::new();
        let (diff, _) = detector.detect_diff_with_warnings(&old_schema, &new_schema);

        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.renamed_columns.len(), 2);

        let old_names: Vec<&str> = table_diff
            .renamed_columns
            .iter()
            .map(|r| r.old_name.as_str())
            .collect();
        assert!(old_names.contains(&"name"));
        assert!(old_names.contains(&"email"));
    }

    /// 無効なrenamed_from（存在しないカラム）の警告テスト
    #[test]
    fn test_invalid_renamed_from_warning() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_invalid_renamed_from_yaml()).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        let detector = SchemaDiffDetector::new();
        let (diff, warnings) = detector.detect_diff_with_warnings(&old_schema, &new_schema);

        // 警告が生成されることを確認
        assert!(!warnings.is_empty());
        assert!(warnings
            .iter()
            .any(|w| w.message.contains("nonexistent_column")));

        // 無効なリネームはリネームとして検出されない
        let table_diff = &diff.modified_tables[0];
        assert!(table_diff.renamed_columns.is_empty());

        // 代わりに追加されたカラムとして扱われる
        assert!(table_diff
            .added_columns
            .iter()
            .any(|c| c.name == "user_email"));
    }

    // ==========================================
    // PostgreSQL SQL生成テスト
    // ==========================================

    /// PostgreSQLでの単純リネームSQL生成テスト
    #[test]
    fn test_postgresql_simple_rename_sql_generation() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_simple_rename_yaml()).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        let detector = SchemaDiffDetector::new();
        let diff = detector.detect_diff(&old_schema, &new_schema);

        // MigrationPipelineでSQL生成
        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);

        let (up_sql, _) = pipeline.generate_up().unwrap();
        let (down_sql, _) = pipeline.generate_down().unwrap();

        // Up SQL にリネーム文が含まれることを確認
        assert!(
            up_sql.contains("ALTER TABLE users RENAME COLUMN name TO user_name"),
            "Expected rename SQL in up.sql: {}",
            up_sql
        );

        // Down SQL に逆リネーム文が含まれることを確認
        assert!(
            down_sql.contains("ALTER TABLE users RENAME COLUMN user_name TO name"),
            "Expected reverse rename SQL in down.sql: {}",
            down_sql
        );
    }

    /// PostgreSQLでのリネーム+型変更SQL生成テスト（実行順序確認）
    #[test]
    fn test_postgresql_rename_with_type_change_sql_order() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_rename_with_type_change_yaml()).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        let detector = SchemaDiffDetector::new();
        let diff = detector.detect_diff(&old_schema, &new_schema);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);

        let (up_sql, _) = pipeline.generate_up().unwrap();

        // リネームSQLが含まれることを確認
        assert!(
            up_sql.contains("RENAME COLUMN name TO user_name"),
            "Expected rename SQL in up.sql: {}",
            up_sql
        );

        // 型変更SQLが含まれることを確認
        assert!(
            up_sql.contains("ALTER TABLE users ALTER COLUMN user_name TYPE"),
            "Expected type change SQL in up.sql: {}",
            up_sql
        );

        // Up方向: リネームが型変更より先に実行されることを確認
        let rename_pos = up_sql.find("RENAME COLUMN name TO user_name").unwrap();
        let type_change_pos = up_sql
            .find("ALTER TABLE users ALTER COLUMN user_name TYPE")
            .unwrap();
        assert!(
            rename_pos < type_change_pos,
            "Rename should come before type change in Up SQL"
        );
    }

    // ==========================================
    // MySQL SQL生成テスト
    // ==========================================

    /// MySQLでの単純リネームSQL生成テスト
    #[test]
    fn test_mysql_simple_rename_sql_generation() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_simple_rename_yaml()).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        let detector = SchemaDiffDetector::new();
        let diff = detector.detect_diff(&old_schema, &new_schema);

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL)
            .with_schemas(&old_schema, &new_schema);

        let (up_sql, _) = pipeline.generate_up().unwrap();
        let (down_sql, _) = pipeline.generate_down().unwrap();

        // MySQLではCHANGE COLUMN構文を使用（完全なカラム定義が必要）
        assert!(
            up_sql.contains("ALTER TABLE users CHANGE COLUMN name user_name"),
            "Expected MySQL CHANGE COLUMN SQL in up.sql: {}",
            up_sql
        );

        assert!(
            down_sql.contains("ALTER TABLE users CHANGE COLUMN user_name name"),
            "Expected MySQL reverse CHANGE COLUMN SQL in down.sql: {}",
            down_sql
        );
    }

    /// MySQLでの複数カラムリネームSQL生成テスト
    #[test]
    fn test_mysql_multiple_renames_sql_generation() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_multiple_renames_yaml()).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        let detector = SchemaDiffDetector::new();
        let diff = detector.detect_diff(&old_schema, &new_schema);

        let pipeline = MigrationPipeline::new(&diff, Dialect::MySQL)
            .with_schemas(&old_schema, &new_schema);

        let (up_sql, _) = pipeline.generate_up().unwrap();

        // MySQLではCHANGE COLUMN構文を使用
        assert!(
            up_sql.contains("CHANGE COLUMN name full_name")
                || up_sql.contains("CHANGE COLUMN email email_address"),
            "Expected CHANGE COLUMN SQLs in up.sql: {}",
            up_sql
        );
    }

    // ==========================================
    // SQLite SQL生成テスト
    // ==========================================

    /// SQLiteでの単純リネームSQL生成テスト
    #[test]
    fn test_sqlite_simple_rename_sql_generation() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_simple_rename_yaml()).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        let detector = SchemaDiffDetector::new();
        let diff = detector.detect_diff(&old_schema, &new_schema);

        let pipeline = MigrationPipeline::new(&diff, Dialect::SQLite)
            .with_schemas(&old_schema, &new_schema);

        let (up_sql, _) = pipeline.generate_up().unwrap();
        let (down_sql, _) = pipeline.generate_down().unwrap();

        // SQLite 3.25.0+形式のリネームSQL
        assert!(
            up_sql.contains("ALTER TABLE users RENAME COLUMN name TO user_name"),
            "Expected SQLite rename SQL in up.sql: {}",
            up_sql
        );

        assert!(
            down_sql.contains("ALTER TABLE users RENAME COLUMN user_name TO name"),
            "Expected SQLite reverse rename SQL in down.sql: {}",
            down_sql
        );
    }

    /// SQLiteでの複数カラムリネームSQL生成テスト
    #[test]
    fn test_sqlite_multiple_renames_sql_generation() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_multiple_renames_yaml()).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        let detector = SchemaDiffDetector::new();
        let diff = detector.detect_diff(&old_schema, &new_schema);

        let pipeline = MigrationPipeline::new(&diff, Dialect::SQLite)
            .with_schemas(&old_schema, &new_schema);

        let (up_sql, _) = pipeline.generate_up().unwrap();

        // 両方のリネームが含まれることを確認
        assert!(
            up_sql.contains("RENAME COLUMN"),
            "Expected rename SQL in up.sql: {}",
            up_sql
        );
    }

    // ==========================================
    // 方言間SQL出力比較テスト
    // ==========================================

    /// 3つの方言で同じリネームが正しく生成されることを確認
    #[test]
    fn test_all_dialects_generate_rename_sql() {
        let temp_dir = TempDir::new().unwrap();
        let old_schema_path = temp_dir.path().join("old_schema.yaml");
        let new_schema_path = temp_dir.path().join("new_schema.yaml");

        fs::write(&old_schema_path, create_old_schema_yaml()).unwrap();
        fs::write(&new_schema_path, create_new_schema_simple_rename_yaml()).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_schema_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_schema_path).unwrap();

        let detector = SchemaDiffDetector::new();
        let diff = detector.detect_diff(&old_schema, &new_schema);

        // PostgreSQLとSQLiteはRENAME COLUMN構文
        for dialect in [Dialect::PostgreSQL, Dialect::SQLite] {
            let pipeline =
                MigrationPipeline::new(&diff, dialect).with_schemas(&old_schema, &new_schema);

            let (up_sql, _) = pipeline.generate_up().unwrap();

            assert!(
                up_sql.contains("RENAME COLUMN name TO user_name"),
                "Expected RENAME COLUMN SQL for {:?}: {}",
                dialect,
                up_sql
            );
        }

        // MySQLはCHANGE COLUMN構文
        let pipeline =
            MigrationPipeline::new(&diff, Dialect::MySQL).with_schemas(&old_schema, &new_schema);

        let (up_sql, _) = pipeline.generate_up().unwrap();

        assert!(
            up_sql.contains("CHANGE COLUMN name user_name"),
            "Expected CHANGE COLUMN SQL for MySQL: {}",
            up_sql
        );
    }

    // ==========================================
    // 検証統合テスト
    // ==========================================

    /// 重複リネームのバリデーションエラーテスト
    #[test]
    fn test_duplicate_rename_validation_error() {
        let duplicate_rename_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: new_email1
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        renamed_from: email
      - name: new_email2
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        renamed_from: email
    primary_key:
      - id
"#;

        let temp_dir = TempDir::new().unwrap();
        let schema_path = temp_dir.path().join("schema.yaml");
        fs::write(&schema_path, duplicate_rename_yaml).unwrap();

        let parser = SchemaParserService::new();
        let schema = parser.parse_schema_file(&schema_path).unwrap();

        let validator = SchemaValidatorService::new();
        let result = validator.validate_renames(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("email") && e.to_string().contains("duplicate")));
    }

    /// 名前衝突のバリデーションエラーテスト
    #[test]
    fn test_name_collision_validation_error() {
        let collision_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: existing_name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
      - name: new_name
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        renamed_from: existing_name
    primary_key:
      - id
"#;

        let temp_dir = TempDir::new().unwrap();
        let schema_path = temp_dir.path().join("schema.yaml");
        fs::write(&schema_path, collision_yaml).unwrap();

        let parser = SchemaParserService::new();
        let schema = parser.parse_schema_file(&schema_path).unwrap();

        let validator = SchemaValidatorService::new();
        let result = validator.validate_renames(&schema);

        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| {
            e.to_string().contains("existing_name") && e.to_string().contains("collision")
        }));
    }

    // ==========================================
    // renamed_fromのシリアライズ/デシリアライズテスト
    // ==========================================

    /// renamed_from属性がYAMLから正しくパースされることを確認
    #[test]
    fn test_renamed_from_yaml_parsing() {
        let yaml_with_renamed_from = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: new_name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
        renamed_from: old_name
    primary_key:
      - id
"#;

        let temp_dir = TempDir::new().unwrap();
        let schema_path = temp_dir.path().join("schema.yaml");
        fs::write(&schema_path, yaml_with_renamed_from).unwrap();

        let parser = SchemaParserService::new();
        let schema = parser.parse_schema_file(&schema_path).unwrap();

        let users_table = schema.get_table("users").unwrap();
        let new_name_column = users_table.get_column("new_name").unwrap();

        assert_eq!(new_name_column.renamed_from, Some("old_name".to_string()));
    }

    /// renamed_fromがNoneの場合、YAMLから除外されることを確認
    #[test]
    fn test_renamed_from_none_not_serialized() {
        use strata::services::schema_serializer::SchemaSerializerService;

        let yaml_without_renamed_from = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
    primary_key:
      - id
"#;

        let temp_dir = TempDir::new().unwrap();
        let original_path = temp_dir.path().join("original.yaml");
        let serialized_path = temp_dir.path().join("serialized.yaml");

        fs::write(&original_path, yaml_without_renamed_from).unwrap();

        let parser = SchemaParserService::new();
        let schema = parser.parse_schema_file(&original_path).unwrap();

        let serializer = SchemaSerializerService::new();
        serializer.serialize_to_file(&schema, &serialized_path).unwrap();

        let serialized_content = fs::read_to_string(&serialized_path).unwrap();

        // renamed_fromがないカラムはYAMLに含まれない
        assert!(
            !serialized_content.contains("renamed_from"),
            "renamed_from should not be in serialized YAML when None"
        );
    }
}
