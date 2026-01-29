/// 同一テーブル内の複数カラム同時変更、auto_increment、インデックス操作のエッジケーステスト
mod common;

#[cfg(test)]
mod multiple_column_changes_same_table {
    use crate::common;
    use strata::core::config::Dialect;

    /// 同一テーブル内で複数カラムの型変更を同時に行う
    #[test]
    fn test_multiple_columns_type_change_same_table_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  users:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: age
        type:
          kind: INTEGER
        nullable: true
      - name: score
        type:
          kind: INTEGER
        nullable: true
      - name: rating
        type:
          kind: INTEGER
        nullable: true
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
      - name: age
        type:
          kind: VARCHAR
          length: 10
        nullable: true
      - name: score
        type:
          kind: DECIMAL
          precision: 10
          scale: 2
        nullable: true
      - name: rating
        type:
          kind: FLOAT
        nullable: true
    primary_key:
      - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#""age" TYPE"#),
            "Expected type change for age: {}",
            up_sql
        );
        assert!(
            up_sql.contains(r#""score" TYPE"#),
            "Expected type change for score: {}",
            up_sql
        );
        assert!(
            up_sql.contains(r#""rating" TYPE"#),
            "Expected type change for rating: {}",
            up_sql
        );
    }

    /// カラム追加 + 型変更 + リネームを同時に行う
    #[test]
    fn test_add_modify_rename_same_table_postgres() {
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
      - name: age
        type:
          kind: INTEGER
        nullable: true
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
      - name: full_name
        type:
          kind: VARCHAR
          length: 200
        nullable: false
        renamed_from: name
      - name: age
        type:
          kind: VARCHAR
          length: 10
        nullable: true
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
    primary_key:
      - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // リネーム
        assert!(
            up_sql.contains(r#"RENAME COLUMN "name" TO "full_name""#),
            "Expected rename: {}",
            up_sql
        );
        // 型変更（リネーム後のカラム名で）
        assert!(
            up_sql.contains(r#""full_name" TYPE"#),
            "Expected type change for full_name: {}",
            up_sql
        );
        // 型変更
        assert!(
            up_sql.contains(r#""age" TYPE"#),
            "Expected type change for age: {}",
            up_sql
        );
        // カラム追加
        assert!(
            up_sql.contains(r#"ADD COLUMN "email""#),
            "Expected ADD COLUMN for email: {}",
            up_sql
        );
    }

    /// カラム追加 + インデックス追加を同時に行う
    #[test]
    fn test_add_column_with_index_postgres() {
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
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#"ADD COLUMN "email""#),
            "Expected ADD COLUMN: {}",
            up_sql
        );
        assert!(
            up_sql.contains(r#"CREATE UNIQUE INDEX "idx_users_email""#),
            "Expected CREATE UNIQUE INDEX: {}",
            up_sql
        );

        // カラム追加がインデックス作成より先
        let add_pos = up_sql.find(r#"ADD COLUMN "email""#).unwrap();
        let idx_pos = up_sql.find(r#"CREATE UNIQUE INDEX"#).unwrap();
        assert!(
            add_pos < idx_pos,
            "ADD COLUMN should come before CREATE INDEX: {}",
            up_sql
        );
    }
}

#[cfg(test)]
mod auto_increment_edge_cases {
    use crate::common;
    use strata::core::config::Dialect;

    /// auto_increment付きカラムの精度変更 (INT → BIGINT相当)
    #[test]
    fn test_auto_increment_precision_change_postgres() {
        let old_yaml = r#"
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
          precision: 8
        nullable: false
        auto_increment: true
      - name: name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
    primary_key:
      - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#""id" TYPE"#) || up_sql.contains(r#""id""#),
            "Expected type change for auto_increment id: {}",
            up_sql
        );
    }

    /// auto_increment付きカラムの精度変更 (MySQL)
    #[test]
    fn test_auto_increment_precision_change_mysql() {
        let old_yaml = r#"
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
          precision: 8
        nullable: false
        auto_increment: true
      - name: name
        type:
          kind: VARCHAR
          length: 100
        nullable: false
    primary_key:
      - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::MySQL);

        assert!(
            up_sql.contains("MODIFY COLUMN `id`"),
            "Expected MODIFY COLUMN for auto_increment id: {}",
            up_sql
        );
        assert!(
            up_sql.contains("AUTO_INCREMENT"),
            "Expected AUTO_INCREMENT to be preserved: {}",
            up_sql
        );
    }
}

#[cfg(test)]
mod index_operations_with_column_changes {
    use crate::common;
    use strata::core::config::Dialect;

    /// インデックス削除 + カラム型変更 + 新インデックス追加を同時に行う
    #[test]
    fn test_drop_index_type_change_add_index_postgres() {
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
      - name: name
        type:
          kind: VARCHAR
          length: 50
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_users_email
        columns:
          - email
        unique: true
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
      - name: name
        type:
          kind: VARCHAR
          length: 200
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_users_name
        columns:
          - name
        unique: false
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // 新インデックスの追加
        assert!(
            up_sql.contains(r#"CREATE INDEX "idx_users_name""#),
            "Expected CREATE INDEX for new index: {}",
            up_sql
        );
        // 型変更（emailとname両方が変更される）
        assert!(
            up_sql.contains(r#""email" TYPE"#),
            "Expected type change for email: {}",
            up_sql
        );
        assert!(
            up_sql.contains(r#""name" TYPE"#),
            "Expected type change for name: {}",
            up_sql
        );
    }

    /// カラムリネーム + 旧インデックス削除 + 新インデックス追加
    #[test]
    fn test_rename_column_with_index_change_postgres() {
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
    indexes:
      - name: idx_users_email
        columns:
          - email
        unique: true
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
      - name: email_address
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        renamed_from: email
    primary_key:
      - id
    indexes:
      - name: idx_users_email_address
        columns:
          - email_address
        unique: true
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // カラムリネーム
        assert!(
            up_sql.contains(r#"RENAME COLUMN "email" TO "email_address""#),
            "Expected rename SQL: {}",
            up_sql
        );
    }
}
