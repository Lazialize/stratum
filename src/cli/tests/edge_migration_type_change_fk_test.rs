/// 型変換＋外部キー、型変換＋インデックス、複合キー＋型変換、nullable/default変更＋型変換のエッジケーステスト
mod common;

#[cfg(test)]
mod type_change_with_foreign_key {
    use crate::common;
    use strata::core::config::Dialect;

    /// FKターゲットカラム(users.id)の型変更: INTEGER → BIGINT
    /// posts.user_id が users.id を参照している状態で、
    /// users.id と posts.user_id を同時に BIGINT に変更
    #[test]
    fn test_fk_target_and_source_type_change_postgres() {
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
          precision: 8
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

        let (up_sql, down_sql) =
            common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // 両方のテーブルで型変更が生成される
        assert!(
            up_sql.contains(r#"ALTER TABLE "users" ALTER COLUMN "id" TYPE"#),
            "Expected type change for users.id in up SQL: {}",
            up_sql
        );
        assert!(
            up_sql.contains(r#"ALTER TABLE "posts" ALTER COLUMN "user_id" TYPE"#),
            "Expected type change for posts.user_id in up SQL: {}",
            up_sql
        );

        // DOWN SQLも生成される
        assert!(
            !down_sql.is_empty(),
            "Down SQL should not be empty: {}",
            down_sql
        );
    }

    /// FKターゲットカラムの型変更: INTEGER → BIGINT (MySQL)
    #[test]
    fn test_fk_target_and_source_type_change_mysql() {
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
          precision: 8
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
          precision: 8
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

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::MySQL);

        // MySQLではMODIFY COLUMNで型変更
        assert!(
            up_sql.contains("MODIFY COLUMN `id`") || up_sql.contains("ALTER TABLE `users`"),
            "Expected type change for users.id in MySQL up SQL: {}",
            up_sql
        );
        assert!(
            up_sql.contains("MODIFY COLUMN `user_id`") || up_sql.contains("ALTER TABLE `posts`"),
            "Expected type change for posts.user_id in MySQL up SQL: {}",
            up_sql
        );
    }

    /// FKターゲットカラムの型変更: INTEGER → BIGINT (SQLite)
    /// SQLiteではテーブル再作成パターンが必要
    #[test]
    fn test_fk_target_and_source_type_change_sqlite() {
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
          precision: 8
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
          precision: 8
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

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::SQLite);

        // SQLiteではPRAGMA foreign_keys=offが含まれる
        assert!(
            up_sql.contains("PRAGMA foreign_keys=off"),
            "Expected PRAGMA foreign_keys=off in SQLite up SQL: {}",
            up_sql
        );
        // テーブル再作成パターン
        assert!(
            up_sql.contains("CREATE TABLE") && up_sql.contains("DROP TABLE"),
            "Expected table recreation pattern in SQLite up SQL: {}",
            up_sql
        );
    }

    /// 新しい外部キー制約を追加する際に、参照先が異なる型の場合
    /// users.id は INTEGER、posts.user_id を新規追加してFKを張る
    #[test]
    fn test_add_fk_column_with_matching_type() {
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
      - name: title
        type:
          kind: VARCHAR
          length: 200
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
    primary_key:
      - id
  posts:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: title
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
      - type: FOREIGN_KEY
        columns:
          - user_id
        referenced_table: users
        referenced_columns:
          - id
"#;

        let (up_sql, down_sql) =
            common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // カラム追加
        assert!(
            up_sql.contains(r#"ADD COLUMN "user_id""#),
            "Expected ADD COLUMN for user_id: {}",
            up_sql
        );
        // FK制約追加
        assert!(
            up_sql.contains("FOREIGN KEY") || up_sql.contains("REFERENCES"),
            "Expected FK constraint in up SQL: {}",
            up_sql
        );

        // DOWN: FK削除とカラム削除
        assert!(
            !down_sql.is_empty(),
            "Down SQL should handle FK removal: {}",
            down_sql
        );
    }

    /// 複数のFKが同一テーブルを参照している場合の型変更
    /// posts.author_id と posts.editor_id が両方 users.id を参照
    #[test]
    fn test_multiple_fks_referencing_same_table_type_change() {
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
      - name: author_id
        type:
          kind: INTEGER
        nullable: false
      - name: editor_id
        type:
          kind: INTEGER
        nullable: true
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - author_id
        referenced_table: users
        referenced_columns:
          - id
      - type: FOREIGN_KEY
        columns:
          - editor_id
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
          precision: 8
        nullable: false
      - name: editor_id
        type:
          kind: INTEGER
          precision: 8
        nullable: true
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - author_id
        referenced_table: users
        referenced_columns:
          - id
      - type: FOREIGN_KEY
        columns:
          - editor_id
        referenced_table: users
        referenced_columns:
          - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // 3つのカラム全てで型変更が生成される
        assert!(
            up_sql.contains(r#""id" TYPE"#),
            "Expected type change for users.id: {}",
            up_sql
        );
        assert!(
            up_sql.contains(r#""author_id" TYPE"#),
            "Expected type change for posts.author_id: {}",
            up_sql
        );
        assert!(
            up_sql.contains(r#""editor_id" TYPE"#),
            "Expected type change for posts.editor_id: {}",
            up_sql
        );
    }
}

#[cfg(test)]
mod type_change_with_index {
    use crate::common;
    use strata::core::config::Dialect;

    /// インデックス付きカラムの型変更 (INTEGER → VARCHAR)
    #[test]
    fn test_indexed_column_type_change_postgres() {
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
    primary_key:
      - id
    indexes:
      - name: idx_users_age
        columns:
          - age
        unique: false
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
    primary_key:
      - id
    indexes:
      - name: idx_users_age
        columns:
          - age
        unique: false
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // 型変更SQL
        assert!(
            up_sql.contains(r#"ALTER TABLE "users" ALTER COLUMN "age" TYPE"#),
            "Expected type change for age: {}",
            up_sql
        );
    }

    /// UNIQUEインデックス付きカラムの型変更
    #[test]
    fn test_unique_indexed_column_type_change_postgres() {
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
            up_sql.contains(r#""email" TYPE"#),
            "Expected type change for email: {}",
            up_sql
        );
    }

    /// 複合インデックスの一部カラムが型変更される場合
    #[test]
    fn test_composite_index_partial_type_change_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  orders:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: customer_id
        type:
          kind: INTEGER
        nullable: false
      - name: status
        type:
          kind: VARCHAR
          length: 20
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_orders_customer_status
        columns:
          - customer_id
          - status
        unique: false
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  orders:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: customer_id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
      - name: status
        type:
          kind: VARCHAR
          length: 20
        nullable: false
    primary_key:
      - id
    indexes:
      - name: idx_orders_customer_status
        columns:
          - customer_id
          - status
        unique: false
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#""customer_id" TYPE"#),
            "Expected type change for customer_id in composite index: {}",
            up_sql
        );
    }

    /// SQLiteでインデックス付きカラムの型変更（テーブル再作成+インデックス再作成）
    #[test]
    fn test_indexed_column_type_change_sqlite() {
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
    indexes:
      - name: idx_users_score
        columns:
          - score
        unique: false
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
    indexes:
      - name: idx_users_score
        columns:
          - score
        unique: false
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::SQLite);

        // テーブル再作成パターン
        assert!(
            up_sql.contains("PRAGMA foreign_keys=off"),
            "Expected PRAGMA foreign_keys=off: {}",
            up_sql
        );
        // インデックス再作成
        assert!(
            up_sql.contains(r#"CREATE INDEX "idx_users_score""#),
            "Expected index recreation in SQLite: {}",
            up_sql
        );
    }
}

#[cfg(test)]
mod composite_key_type_change {
    use crate::common;
    use strata::core::config::Dialect;

    /// 複合プライマリキーの一部カラムの型変更
    #[test]
    fn test_composite_pk_partial_type_change_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  order_items:
    columns:
      - name: order_id
        type:
          kind: INTEGER
        nullable: false
      - name: item_id
        type:
          kind: INTEGER
        nullable: false
      - name: quantity
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - order_id
      - item_id
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  order_items:
    columns:
      - name: order_id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
      - name: item_id
        type:
          kind: INTEGER
        nullable: false
      - name: quantity
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - order_id
      - item_id
"#;

        let (up_sql, down_sql) =
            common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#""order_id" TYPE"#),
            "Expected type change for composite PK column order_id: {}",
            up_sql
        );

        assert!(
            !down_sql.is_empty(),
            "Down SQL should be generated for composite PK type change"
        );
    }

    /// 複合プライマリキーの両方のカラムの型変更
    #[test]
    fn test_composite_pk_both_columns_type_change_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  order_items:
    columns:
      - name: order_id
        type:
          kind: INTEGER
        nullable: false
      - name: item_id
        type:
          kind: INTEGER
        nullable: false
      - name: quantity
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - order_id
      - item_id
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  order_items:
    columns:
      - name: order_id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
      - name: item_id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
      - name: quantity
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - order_id
      - item_id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        assert!(
            up_sql.contains(r#""order_id" TYPE"#),
            "Expected type change for order_id: {}",
            up_sql
        );
        assert!(
            up_sql.contains(r#""item_id" TYPE"#),
            "Expected type change for item_id: {}",
            up_sql
        );
    }

    /// 複合外部キーの型変更
    #[test]
    fn test_composite_fk_type_change_postgres() {
        let old_yaml = r#"
version: "1.0"
tables:
  order_items:
    columns:
      - name: order_id
        type:
          kind: INTEGER
        nullable: false
      - name: item_id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - order_id
      - item_id
  shipments:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: order_id
        type:
          kind: INTEGER
        nullable: false
      - name: item_id
        type:
          kind: INTEGER
        nullable: false
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - order_id
          - item_id
        referenced_table: order_items
        referenced_columns:
          - order_id
          - item_id
"#;

        let new_yaml = r#"
version: "1.0"
tables:
  order_items:
    columns:
      - name: order_id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
      - name: item_id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
    primary_key:
      - order_id
      - item_id
  shipments:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: order_id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
      - name: item_id
        type:
          kind: INTEGER
          precision: 8
        nullable: false
    primary_key:
      - id
    constraints:
      - type: FOREIGN_KEY
        columns:
          - order_id
          - item_id
        referenced_table: order_items
        referenced_columns:
          - order_id
          - item_id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // 全4カラムで型変更が生成される
        // order_items.order_id, order_items.item_id
        assert!(
            up_sql.contains(r#"ALTER TABLE "order_items""#),
            "Expected ALTER for order_items: {}",
            up_sql
        );
        // shipments.order_id, shipments.item_id
        assert!(
            up_sql.contains(r#"ALTER TABLE "shipments""#),
            "Expected ALTER for shipments: {}",
            up_sql
        );
    }
}

#[cfg(test)]
mod nullable_default_with_type_change {
    use crate::common;
    use strata::core::config::Dialect;

    /// NOT NULL → NULL + 型変更を同時に行う
    #[test]
    fn test_nullable_change_with_type_change_postgres() {
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
      - name: age
        type:
          kind: VARCHAR
          length: 10
        nullable: true
    primary_key:
      - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::PostgreSQL);

        // 型変更
        assert!(
            up_sql.contains(r#""age" TYPE"#),
            "Expected type change for age: {}",
            up_sql
        );
    }

    /// DEFAULT値付きカラムの型変更 (MySQL)
    #[test]
    fn test_default_value_with_type_change_mysql() {
        let old_yaml = r#"
version: "1.0"
tables:
  products:
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
      - name: status
        type:
          kind: VARCHAR
          length: 20
        nullable: false
        default_value: "'active'"
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
      - name: status
        type:
          kind: VARCHAR
          length: 50
        nullable: false
        default_value: "'active'"
    primary_key:
      - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::MySQL);

        // MODIFY COLUMNでDEFAULT値が保持される
        assert!(
            up_sql.contains("MODIFY COLUMN `status`"),
            "Expected MODIFY COLUMN for status: {}",
            up_sql
        );
        assert!(
            up_sql.contains("DEFAULT"),
            "Expected DEFAULT to be preserved: {}",
            up_sql
        );
    }

    /// NULL → NOT NULL + 型変更 + DEFAULT追加を同時に行う
    #[test]
    fn test_nullable_to_not_null_with_type_change_and_default_mysql() {
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
          kind: VARCHAR
          length: 20
        nullable: true
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
      - name: price
        type:
          kind: DECIMAL
          precision: 10
          scale: 2
        nullable: false
        default_value: "0.00"
    primary_key:
      - id
"#;

        let (up_sql, _) = common::generate_migration_sql(old_yaml, new_yaml, Dialect::MySQL);

        assert!(
            up_sql.contains("MODIFY COLUMN `price`"),
            "Expected MODIFY COLUMN for price: {}",
            up_sql
        );
        assert!(
            up_sql.contains("NOT NULL"),
            "Expected NOT NULL in type change: {}",
            up_sql
        );
    }
}
