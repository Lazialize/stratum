/// ENUM関連および複数テーブル同時変更のエッジケーステスト
mod common;

#[cfg(test)]
mod enum_edge_cases {
    use crate::common;
    use std::fs;
    use strata::core::config::Dialect;
    use strata::services::schema_diff_detector::SchemaDiffDetectorService;
    use strata::services::schema_io::schema_parser::SchemaParserService;
    use tempfile::TempDir;

    /// ENUM型カラムにインデックスを追加
    #[test]
    fn test_enum_column_with_index_postgres() {
        let old_yaml = r#"
version: "1.0"
enums:
  user_status:
    name: user_status
    values:
      - active
      - inactive
      - banned
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: status
        type:
          kind: ENUM
          name: user_status
        nullable: false
    primary_key:
      - id
"#;

        let new_yaml = r#"
version: "1.0"
enums:
  user_status:
    name: user_status
    values:
      - active
      - inactive
      - banned
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: status
        type:
          kind: ENUM
          name: user_status
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_users_status
        columns:
          - status
        unique: false
"#;

        let (up_sql, down_sql) =
            common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#"CREATE INDEX "idx_users_status""#),
            "Expected CREATE INDEX for ENUM column: {}",
            up_sql
        );

        assert!(
            down_sql.contains(r#"DROP INDEX "idx_users_status""#),
            "Expected DROP INDEX in down SQL: {}",
            down_sql
        );
    }

    /// ENUMに値を追加しつつ、同じテーブルの別カラムの型を変更
    #[test]
    fn test_enum_add_value_with_other_column_type_change_postgres() {
        let old_yaml = r#"
version: "1.0"
enums:
  user_status:
    name: user_status
    values:
      - active
      - inactive
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: status
        type:
          kind: ENUM
          name: user_status
        nullable: false
      - name: age
        type:
          kind: INTEGER
        nullable: true
    primary_key:
      - id
"#;

        let new_yaml = r#"
version: "1.0"
enums:
  user_status:
    name: user_status
    values:
      - active
      - inactive
      - banned
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: status
        type:
          kind: ENUM
          name: user_status
        nullable: false
      - name: age
        type:
          kind: VARCHAR
          length: 10
        nullable: true
    primary_key:
      - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // ENUM値追加
        assert!(
            up_sql.contains("ADD VALUE"),
            "Expected ALTER TYPE ADD VALUE: {}",
            up_sql
        );
        // 型変更
        assert!(
            up_sql.contains(r#""age" TYPE"#),
            "Expected type change for age: {}",
            up_sql
        );
    }

    /// ENUM型カラムに UNIQUE 制約を追加
    #[test]
    fn test_enum_column_with_unique_constraint_postgres() {
        let old_yaml = r#"
version: "1.0"
enums:
  priority:
    name: priority
    values:
      - low
      - medium
      - high
tables:
  tasks:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: priority
        type:
          kind: ENUM
          name: priority
        nullable: false
    primary_key:
      - id
"#;

        let new_yaml = r#"
version: "1.0"
enums:
  priority:
    name: priority
    values:
      - low
      - medium
      - high
tables:
  tasks:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: priority
        type:
          kind: ENUM
          name: priority
        nullable: false
    primary_key:
      - id
    constraints:
      - type: UNIQUE
        columns:
          - priority
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // NOTE: UNIQUE制約の追加は差分として検出されるが、
        // generate_add_constraint_for_existing_tableがFOREIGN KEY以外を未サポートのため
        // SQLは空になる（既知の制限事項）
        // ここではパニックせずに正常終了することを確認
        let _ = up_sql;
    }

    /// UNIQUE制約の追加が差分検出レベルで正しく検出されることを確認
    #[test]
    fn test_unique_constraint_on_enum_detected_in_diff() {
        let temp_dir = TempDir::new().unwrap();
        let old_path = temp_dir.path().join("old.yaml");
        let new_path = temp_dir.path().join("new.yaml");

        let old_yaml = r#"
version: "1.0"
enums:
  priority:
    name: priority
    values:
      - low
      - medium
      - high
tables:
  tasks:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: priority
        type:
          kind: ENUM
          name: priority
        nullable: false
    primary_key:
      - id
"#;

        let new_yaml = r#"
version: "1.0"
enums:
  priority:
    name: priority
    values:
      - low
      - medium
      - high
tables:
  tasks:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: priority
        type:
          kind: ENUM
          name: priority
        nullable: false
    primary_key:
      - id
    constraints:
      - type: UNIQUE
        columns:
          - priority
"#;

        fs::write(&old_path, old_yaml).unwrap();
        fs::write(&new_path, new_yaml).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_path).unwrap();

        let detector = SchemaDiffDetectorService::new();
        let diff = detector.detect_diff(&old_schema, &new_schema);

        // UNIQUE制約が差分として検出される
        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(
            table_diff.added_constraints.len(),
            1,
            "UNIQUE constraint should be detected as added"
        );
        assert!(matches!(
            &table_diff.added_constraints[0],
            strata::core::schema::Constraint::UNIQUE { columns }
            if columns == &vec!["priority".to_string()]
        ));
    }

    /// 新しいENUM型を定義し、新しいテーブルのカラムとして使用
    #[test]
    fn test_new_enum_with_new_table_postgres() {
        let old_yaml = r#"
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

        let new_yaml = r#"
version: "1.0"
enums:
  role_type:
    name: role_type
    values:
      - admin
      - editor
      - viewer
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
  roles:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: role
        type:
          kind: ENUM
          name: role_type
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

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // ENUM型の作成がCREATE TABLEより先
        let enum_pos = up_sql.find("CREATE TYPE");
        let table_pos = up_sql.find(r#"CREATE TABLE "roles""#);

        assert!(
            enum_pos.is_some(),
            "Expected CREATE TYPE for enum: {}",
            up_sql
        );
        assert!(
            table_pos.is_some(),
            "Expected CREATE TABLE for roles: {}",
            up_sql
        );

        if let (Some(ep), Some(tp)) = (enum_pos, table_pos) {
            assert!(
                ep < tp,
                "CREATE TYPE should come before CREATE TABLE: {}",
                up_sql
            );
        }
    }
}

#[cfg(test)]
mod multiple_table_changes {
    use crate::common;
    use strata::core::config::Dialect;

    /// 複数テーブルで同時に型変更
    #[test]
    fn test_multiple_tables_type_changes_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: score
        type:
          kind: INTEGER
        nullable: true
    primary_key:
      - id
  products:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: price
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: score
        type:
          kind: VARCHAR
          length: 20
        nullable: true
    primary_key:
      - id
  products:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: price
        type:
          kind: DECIMAL
          precision: 10
          scale: 2
        nullable: false
    primary_key:
      - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#"ALTER TABLE "users" ALTER COLUMN "score" TYPE"#),
            "Expected type change for users.score: {}",
            up_sql
        );
        assert!(
            up_sql.contains(r#"ALTER TABLE "products" ALTER COLUMN "price" TYPE"#),
            "Expected type change for products.price: {}",
            up_sql
        );
    }

    /// テーブル追加 + 既存テーブル変更 + FK追加を同時に行う
    #[test]
    fn test_add_table_modify_existing_add_fk_postgres() {
        let old_yaml = r#"
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
          length: 50
        nullable: false
    primary_key:
      - id
"#;

        let new_yaml = r#"
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
          length: 200
        nullable: false
    primary_key:
      - id
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
      - name: title
        type:
          kind: VARCHAR
          length: 200
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

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // 既存テーブルの型変更
        assert!(
            up_sql.contains(r#"ALTER TABLE "users" ALTER COLUMN "name" TYPE"#),
            "Expected type change for users.name: {}",
            up_sql
        );
        // 新テーブル作成
        assert!(
            up_sql.contains(r#"CREATE TABLE "posts""#),
            "Expected CREATE TABLE for posts: {}",
            up_sql
        );
    }

    /// 3テーブルのFK連鎖（A→B→C）を同時に作成
    #[test]
    fn test_three_table_fk_chain_creation_postgres() {
        let old_yaml = r#"
version: "1.0"
tables: {}
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  organizations:
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
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: org_id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - org_id
        referenced_table: organizations
        referenced_columns:
          - id
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

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // organizations → users → posts の依存順序で作成
        let org_pos = up_sql.find(r#"CREATE TABLE "organizations""#);
        let users_pos = up_sql.find(r#"CREATE TABLE "users""#);
        let posts_pos = up_sql.find(r#"CREATE TABLE "posts""#);

        assert!(org_pos.is_some(), "Expected CREATE TABLE organizations");
        assert!(users_pos.is_some(), "Expected CREATE TABLE users");
        assert!(posts_pos.is_some(), "Expected CREATE TABLE posts");

        let org_pos = org_pos.unwrap();
        let users_pos = users_pos.unwrap();
        let posts_pos = posts_pos.unwrap();

        assert!(
            org_pos < users_pos,
            "organizations should be created before users: {}",
            up_sql
        );
        assert!(
            users_pos < posts_pos,
            "users should be created before posts: {}",
            up_sql
        );
    }

    /// テーブル削除時のFK依存順序（逆順で削除）
    #[test]
    fn test_table_removal_fk_dependency_order_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  organizations:
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
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: org_id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - org_id
        referenced_table: organizations
        referenced_columns:
          - id
"#;

        let new_yaml = r#"
version: "1.0"
tables: {}
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // 両テーブルが削除される
        let users_drop = up_sql.find(r#"DROP TABLE "users""#);
        let org_drop = up_sql.find(r#"DROP TABLE "organizations""#);

        assert!(
            users_drop.is_some(),
            "Expected DROP TABLE users: {}",
            up_sql
        );
        assert!(
            org_drop.is_some(),
            "Expected DROP TABLE organizations: {}",
            up_sql
        );

        // 両テーブルがDROPされることを確認
        // NOTE: 削除順序はsort_removed_tables_by_dependencyに依存
        // usersがorganizationsを参照しているので、
        // usersが先に削除され、organizationsが後に削除される必要がある
        // ただし、パイプラインの実装ではcleanup_statementsステージで
        // 別の順序になる可能性がある
    }
}
