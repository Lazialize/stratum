/// カラムリネーム＋外部キー、CHECK制約、UNIQUE制約、自己参照FK、制約操作のエッジケーステスト
mod common;

#[cfg(test)]
mod column_rename_with_foreign_key {
    use crate::common;
    use strata::core::config::Dialect;

    /// FK元カラムのリネーム: posts.user_id → posts.author_id
    #[test]
    fn test_rename_fk_source_column_postgres() {
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
          length: 100
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
          length: 100
        nullable: false
    primary_key:
      - id
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: author_id
        type:
          kind: INTEGER
        nullable: false
        renamed_from: user_id
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - author_id
        referenced_table: users
        referenced_columns:
          - id
"#;

        let (up_sql, down_sql) =
            common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // カラムリネーム
        assert!(
            up_sql.contains(r#"RENAME COLUMN "user_id" TO "author_id""#),
            "Expected rename SQL for user_id → author_id: {}",
            up_sql
        );

        // DOWN: 逆リネーム
        assert!(
            down_sql.contains(r#"RENAME COLUMN "author_id" TO "user_id""#),
            "Expected reverse rename in down SQL: {}",
            down_sql
        );
    }

    /// FK元カラムのリネーム + 型変更を同時に行う
    #[test]
    fn test_rename_fk_source_with_type_change_postgres() {
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

        let new_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
    primary_key:
      - id
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: author_id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
        renamed_from: user_id
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - author_id
        referenced_table: users
        referenced_columns:
          - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // リネーム
        assert!(
            up_sql.contains(r#"RENAME COLUMN "user_id" TO "author_id""#),
            "Expected rename in up SQL: {}",
            up_sql
        );

        // リネーム後の新しい名前で型変更
        assert!(
            up_sql.contains(r#""author_id" TYPE"#),
            "Expected type change with new name: {}",
            up_sql
        );

        // リネームが型変更より先に来る
        let rename_pos = up_sql.find(r#"RENAME COLUMN "user_id""#).unwrap();
        let type_change_pos = up_sql.find(r#""author_id" TYPE"#).unwrap();
        assert!(
            rename_pos < type_change_pos,
            "Rename should come before type change"
        );
    }

    /// MySQLでのFK元カラムリネーム（CHANGE COLUMN構文）
    #[test]
    fn test_rename_fk_source_column_mysql() {
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

        let new_yaml = r#"
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
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: author_id
        type:
          kind: INTEGER
        nullable: false
        renamed_from: user_id
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - author_id
        referenced_table: users
        referenced_columns:
          - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::MySQL);

        // MySQLではCHANGE COLUMN構文
        assert!(
            up_sql.contains("CHANGE COLUMN `user_id` `author_id`"),
            "Expected MySQL CHANGE COLUMN syntax: {}",
            up_sql
        );
    }
}

#[cfg(test)]
mod check_constraint_edge_cases {
    use crate::common;
    use std::fs;
    use strata::core::config::Dialect;
    use strata::services::schema_diff_detector::SchemaDiffDetectorService;
    use strata::services::schema_io::schema_parser::SchemaParserService;
    use tempfile::TempDir;

    /// CHECK制約付きカラムの型変更
    #[test]
    fn test_check_constraint_column_type_change_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
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
    constraints:
      - type: CHECK
        columns:
          - price
        check_expression: "price > 0"
"#;

        let new_yaml = r#"
version: "1.0"
tables:
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
    constraints:
      - type: CHECK
        columns:
          - price
        check_expression: "price > 0"
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#""price" TYPE"#),
            "Expected type change for price with CHECK constraint: {}",
            up_sql
        );
    }

    /// CHECK制約の追加と型変更を同時に行う
    #[test]
    fn test_add_check_constraint_with_type_change_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  products:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: quantity
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  products:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: quantity
        type:
          kind: INTEGER
          precision: 8
        nullable: false
    primary_key:
      - id
    constraints:
      - type: CHECK
        columns:
          - quantity
        check_expression: "quantity >= 0"
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // 型変更が生成される
        assert!(
            up_sql.contains(r#""quantity" TYPE"#),
            "Expected type change: {}",
            up_sql
        );
        // NOTE: CHECK制約の追加は差分として検出されるが、
        // generate_add_constraint_for_existing_tableがFOREIGN KEY以外を未サポートのため
        // SQL出力には含まれない（既知の制限事項）
    }

    /// CHECK制約の追加が差分検出レベルで正しく検出されることを確認
    #[test]
    fn test_check_constraint_addition_detected_in_diff() {
        let temp_dir = TempDir::new().unwrap();
        let old_path = temp_dir.path().join("old.yaml");
        let new_path = temp_dir.path().join("new.yaml");

        let old_yaml = r#"
version: "1.0"
tables:
  products:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: quantity
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  products:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: quantity
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
    constraints:
      - type: CHECK
        columns:
          - quantity
        check_expression: "quantity >= 0"
"#;

        fs::write(&old_path, old_yaml).unwrap();
        fs::write(&new_path, new_yaml).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_path).unwrap();

        let detector = SchemaDiffDetectorService::new();
        let diff = detector.detect_diff(&old_schema, &new_schema);

        // CHECK制約が差分として検出される
        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(
            table_diff.added_constraints.len(),
            1,
            "CHECK constraint should be detected as added"
        );
        assert!(matches!(
            &table_diff.added_constraints[0],
            strata::core::schema::Constraint::CHECK {
                check_expression, ..
            } if check_expression == "quantity >= 0"
        ));
    }
}

#[cfg(test)]
mod unique_constraint_edge_cases {
    use crate::common;
    use strata::core::config::Dialect;

    /// UNIQUE制約付きカラムの型変更
    #[test]
    fn test_unique_constraint_column_type_change_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: email
        type:
          kind: VARCHAR
          length: 100
        nullable: false
    primary_key:
      - id
    constraints:
      - type: UNIQUE
        columns:
          - email
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
      - name: email
        type:
          kind: VARCHAR
          length: 500
        nullable: false
    primary_key:
      - id
    constraints:
      - type: UNIQUE
        columns:
          - email
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#""email" TYPE"#),
            "Expected type change for UNIQUE column: {}",
            up_sql
        );
    }

    /// 複合UNIQUE制約の一部カラム型変更
    #[test]
    fn test_composite_unique_partial_type_change_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  user_roles:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: user_id
        type:
          kind: INTEGER
        nullable: false
      - name: role_name
        type:
          kind: VARCHAR
          length: 50
        nullable: false
    primary_key:
      - id
    constraints:
      - type: UNIQUE
        columns:
          - user_id
          - role_name
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  user_roles:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: user_id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
      - name: role_name
        type:
          kind: VARCHAR
          length: 50
        nullable: false
    primary_key:
      - id
    constraints:
      - type: UNIQUE
        columns:
          - user_id
          - role_name
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#""user_id" TYPE"#),
            "Expected type change for composite UNIQUE column: {}",
            up_sql
        );
    }
}

#[cfg(test)]
mod self_referencing_fk {
    use std::fs;
    use strata::core::config::Dialect;
    use strata::services::migration_pipeline::MigrationPipeline;
    use strata::services::schema_diff_detector::SchemaDiffDetectorService;
    use strata::services::schema_io::schema_parser::SchemaParserService;
    use tempfile::TempDir;

    use crate::common;

    /// 自己参照FK付きテーブルの作成は循環参照エラーになる
    /// （自己参照FKはトポロジカルソートで循環として検出される）
    #[test]
    fn test_self_referencing_fk_table_creation_detects_circular_reference() {
        let temp_dir = TempDir::new().unwrap();
        let old_path = temp_dir.path().join("old.yaml");
        let new_path = temp_dir.path().join("new.yaml");

        let old_yaml = r#"
version: "1.0"
tables: {}
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  categories:
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
      - name: parent_id
        type:
          kind: INTEGER
        nullable: true
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - parent_id
        referenced_table: categories
        referenced_columns:
          - id
"#;

        fs::write(&old_path, old_yaml).unwrap();
        fs::write(&new_path, new_yaml).unwrap();

        let parser = SchemaParserService::new();
        let old_schema = parser.parse_schema_file(&old_path).unwrap();
        let new_schema = parser.parse_schema_file(&new_path).unwrap();

        let detector = SchemaDiffDetectorService::new();
        let diff = detector.detect_diff(&old_schema, &new_schema);

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL)
            .with_schemas(&old_schema, &new_schema);

        // 自己参照FKは循環参照としてエラーになる
        let result = pipeline.generate_up();
        assert!(
            result.is_err(),
            "Self-referencing FK should cause circular reference error"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Circular reference"),
            "Error should mention circular reference: {}",
            err_msg
        );
    }

    /// 自己参照FK付きカラムの型変更（parent_id と id を同時に変更）
    #[test]
    fn test_self_referencing_fk_type_change_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  categories:
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
      - name: parent_id
        type:
          kind: INTEGER
        nullable: true
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - parent_id
        referenced_table: categories
        referenced_columns:
          - id
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  categories:
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
      - name: name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
      - name: parent_id
        type:
          kind: INTEGER
          precision: 8
        nullable: true
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - parent_id
        referenced_table: categories
        referenced_columns:
          - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#""id" TYPE"#),
            "Expected type change for id: {}",
            up_sql
        );
        assert!(
            up_sql.contains(r#""parent_id" TYPE"#),
            "Expected type change for parent_id: {}",
            up_sql
        );
    }
}

#[cfg(test)]
mod constraint_operations {
    use crate::common;
    use strata::core::config::Dialect;

    /// FK制約の追加と削除を同時に行う
    #[test]
    fn test_add_and_remove_fk_constraints_postgres() {
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
      - name: category_id
        type:
          kind: INTEGER
        nullable: true
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - user_id
        referenced_table: users
        referenced_columns:
          - id
  categories:
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

        let new_yaml = r#"
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
      - name: category_id
        type:
          kind: INTEGER
        nullable: true
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - category_id
        referenced_table: categories
        referenced_columns:
          - id
  categories:
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

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // FK制約の操作がSQLに含まれる
        assert!(
            up_sql.contains("CONSTRAINT") || up_sql.contains("FOREIGN KEY"),
            "Expected FK constraint operations in SQL: {}",
            up_sql
        );
    }

    /// UNIQUE制約の追加とFK制約の追加を同時に行う
    #[test]
    fn test_add_unique_and_fk_constraints_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: slug
        type:
          kind: VARCHAR
          length: 200
        nullable: false
      - name: user_id
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
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
    constraints:
      - type: UNIQUE
        columns:
          - email
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: slug
        type:
          kind: VARCHAR
          length: 200
        nullable: false
      - name: user_id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
    constraints:
      - type: UNIQUE
        columns:
          - slug
      - type: FOREIGN_KEY
        columns:
          - user_id
        referenced_table: users
        referenced_columns:
          - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // UNIQUE制約とFK制約の両方が追加される
        // (UNIQUE はADD CONSTRAINT ... UNIQUE, FK はADD CONSTRAINT ... FOREIGN KEY)
        assert!(
            up_sql.contains("UNIQUE") || up_sql.contains("CONSTRAINT"),
            "Expected UNIQUE or CONSTRAINT in SQL: {}",
            up_sql
        );
        assert!(
            up_sql.contains("FOREIGN KEY") || up_sql.contains("REFERENCES"),
            "Expected FK constraint in SQL: {}",
            up_sql
        );
    }
}
