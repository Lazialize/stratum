// SQLiteテーブル再作成サービス
//
// SQLiteはALTER COLUMN TYPEをサポートしていないため、
// テーブル再作成パターンで型変更を実現します。

use crate::adapters::sql_generator::{
    quote_columns_sqlite, quote_identifier_sqlite, MigrationDirection,
};
use crate::core::schema::{Column, ColumnType, Constraint, Index, Table};
use crate::core::schema_diff::ColumnDiff;

/// SQLiteテーブル再作成サービス
///
/// テーブル再作成パターンによる型変更SQL生成を行います。
/// 12ステップのテーブル再作成手順を生成します。
pub struct SqliteTableRecreator;

impl SqliteTableRecreator {
    /// 新しいSqliteTableRecreatorを作成
    pub fn new() -> Self {
        Self
    }

    /// テーブル再作成SQLを生成
    ///
    /// # Arguments
    /// * `table` - 再作成後のテーブル定義（方向に応じた完全な定義）
    /// * `column_diff` - カラム差分情報
    /// * `direction` - マイグレーション方向（Up/Down）
    ///
    /// # Returns
    /// 以下の順序のSQL文ベクター:
    /// 1. PRAGMA foreign_keys=off
    /// 2. BEGIN TRANSACTION
    /// 3. CREATE TABLE new_{table} (新スキーマ)
    /// 4. INSERT INTO new_{table} SELECT ... FROM {table}
    /// 5. DROP TABLE {table}
    /// 6. ALTER TABLE new_{table} RENAME TO {table}
    /// 7. インデックス再作成
    /// 8. COMMIT
    /// 9. PRAGMA foreign_keys=on
    /// 10. PRAGMA foreign_key_check
    pub fn generate_table_recreation(
        &self,
        table: &Table,
        _column_diff: &ColumnDiff,
        _direction: MigrationDirection,
    ) -> Vec<String> {
        // 旧テーブル情報がない場合は、新テーブルと同じカラムを仮定
        // （型変更のみの場合、カラム名は同じ）
        self.generate_table_recreation_with_old_table(table, None)
    }

    /// 旧テーブル情報付きでテーブル再作成SQLを生成
    ///
    /// # Arguments
    /// * `new_table` - 再作成後のテーブル定義
    /// * `old_table` - 再作成前のテーブル定義（Noneの場合はnew_tableと同じカラムを仮定）
    ///
    /// # Returns
    /// テーブル再作成SQLのベクター
    pub fn generate_table_recreation_with_old_table(
        &self,
        new_table: &Table,
        old_table: Option<&Table>,
    ) -> Vec<String> {
        let mut statements = Vec::new();
        let table_name = &new_table.name;
        let new_table_name = format!("new_{}", table_name);
        let quoted_table = quote_identifier_sqlite(table_name);
        let quoted_new_table = quote_identifier_sqlite(&new_table_name);

        // 1. 外部キー制約を一時的に無効化
        statements.push("PRAGMA foreign_keys=off".to_string());

        // 2. トランザクション開始
        statements.push("BEGIN TRANSACTION".to_string());

        // 3. 新テーブル作成
        let create_table_sql = self.generate_create_table_with_name(new_table, &new_table_name);
        statements.push(create_table_sql);

        // 4. データコピー（列交差ベース）
        let insert_sql = self.generate_data_copy_sql_with_column_intersection(new_table, old_table);
        statements.push(insert_sql);

        // 5. 旧テーブル削除
        statements.push(format!("DROP TABLE {}", quoted_table));

        // 6. テーブルリネーム
        statements.push(format!(
            "ALTER TABLE {} RENAME TO {}",
            quoted_new_table, quoted_table
        ));

        // 7. インデックス再作成
        for index in &new_table.indexes {
            let index_sql = self.generate_create_index(new_table, index);
            statements.push(index_sql);
        }

        // 8. トランザクションコミット
        statements.push("COMMIT".to_string());

        // 9. 外部キー制約を再有効化
        statements.push("PRAGMA foreign_keys=on".to_string());

        // 10. 外部キー整合性チェック
        statements.push(format!("PRAGMA foreign_key_check({})", quoted_table));

        statements
    }

    /// 指定した名前でCREATE TABLE文を生成
    fn generate_create_table_with_name(&self, table: &Table, table_name: &str) -> String {
        let mut parts = Vec::new();

        parts.push(format!(
            "CREATE TABLE {}",
            quote_identifier_sqlite(table_name)
        ));
        parts.push("(".to_string());

        let mut elements = Vec::new();

        // カラム定義
        for column in &table.columns {
            elements.push(format!("    {}", self.generate_column_definition(column)));
        }

        // テーブル制約
        for constraint in &table.constraints {
            let constraint_def = self.generate_constraint_definition(constraint);
            if !constraint_def.is_empty() {
                elements.push(format!("    {}", constraint_def));
            }
        }

        parts.push(elements.join(",\n"));
        parts.push(")".to_string());

        parts.join("\n")
    }

    /// カラム定義のSQL文字列を生成
    fn generate_column_definition(&self, column: &Column) -> String {
        let mut parts = Vec::new();

        parts.push(quote_identifier_sqlite(&column.name));
        parts.push(self.map_column_type(&column.column_type));

        if !column.nullable {
            parts.push("NOT NULL".to_string());
        }

        if let Some(ref default_value) = column.default_value {
            parts.push(format!("DEFAULT {}", default_value));
        }

        parts.join(" ")
    }

    /// ColumnTypeをSQLiteの型文字列にマッピング
    fn map_column_type(&self, column_type: &ColumnType) -> String {
        match column_type {
            ColumnType::INTEGER { .. } => "INTEGER".to_string(),
            ColumnType::VARCHAR { .. } => "TEXT".to_string(),
            ColumnType::TEXT => "TEXT".to_string(),
            ColumnType::BOOLEAN => "INTEGER".to_string(),
            ColumnType::TIMESTAMP { .. } => "TEXT".to_string(),
            ColumnType::JSON | ColumnType::JSONB => "TEXT".to_string(),
            ColumnType::DECIMAL { .. } => "TEXT".to_string(),
            ColumnType::FLOAT | ColumnType::DOUBLE => "REAL".to_string(),
            ColumnType::CHAR { .. } => "TEXT".to_string(),
            ColumnType::DATE => "TEXT".to_string(),
            ColumnType::TIME { .. } => "TEXT".to_string(),
            ColumnType::BLOB => "BLOB".to_string(),
            ColumnType::UUID => "TEXT".to_string(),
            ColumnType::Enum { name } => name.clone(),
            ColumnType::DialectSpecific { kind, .. } => kind.to_string(),
        }
    }

    /// 制約定義のSQL文字列を生成
    fn generate_constraint_definition(&self, constraint: &Constraint) -> String {
        match constraint {
            Constraint::PRIMARY_KEY { columns } => {
                format!("PRIMARY KEY ({})", quote_columns_sqlite(columns))
            }
            Constraint::UNIQUE { columns } => {
                format!("UNIQUE ({})", quote_columns_sqlite(columns))
            }
            Constraint::CHECK {
                check_expression, ..
            } => {
                format!("CHECK ({})", check_expression)
            }
            Constraint::FOREIGN_KEY {
                columns,
                referenced_table,
                referenced_columns,
            } => {
                format!(
                    "FOREIGN KEY ({}) REFERENCES {} ({})",
                    quote_columns_sqlite(columns),
                    quote_identifier_sqlite(referenced_table),
                    quote_columns_sqlite(referenced_columns)
                )
            }
        }
    }

    /// データコピーSQLを生成（列交差ベース）
    ///
    /// old_schemaとnew_schemaの共通カラムを明示的にリストし、
    /// 追加されたカラムにはDEFAULT値またはNULLを設定します。
    ///
    /// # Arguments
    /// * `new_table` - 新しいテーブル定義
    /// * `old_table` - 旧テーブル定義（Noneの場合は新テーブルと同じカラムを仮定）
    fn generate_data_copy_sql_with_column_intersection(
        &self,
        new_table: &Table,
        old_table: Option<&Table>,
    ) -> String {
        let new_table_name = format!("new_{}", new_table.name);
        let quoted_new_table = quote_identifier_sqlite(&new_table_name);
        let quoted_table = quote_identifier_sqlite(&new_table.name);

        // 旧テーブル情報がない場合は、新テーブルと同じカラムを仮定
        let old_columns: std::collections::HashSet<&str> = match old_table {
            Some(old) => old.columns.iter().map(|c| c.name.as_str()).collect(),
            None => new_table.columns.iter().map(|c| c.name.as_str()).collect(),
        };

        // 新テーブルのカラムごとに処理
        let mut insert_columns = Vec::new();
        let mut select_expressions = Vec::new();

        for column in &new_table.columns {
            insert_columns.push(quote_identifier_sqlite(&column.name));

            if old_columns.contains(column.name.as_str()) {
                // 共通カラム: そのままコピー
                select_expressions.push(quote_identifier_sqlite(&column.name));
            } else {
                // 追加されたカラム: DEFAULT値またはNULLを使用
                if let Some(ref default_value) = column.default_value {
                    select_expressions.push(default_value.clone());
                } else if column.nullable {
                    select_expressions.push("NULL".to_string());
                } else {
                    // NOT NULLでDEFAULTがない場合はエラーになる可能性があるが、
                    // 検証フェーズで事前に検出されるべき
                    // ここでは空文字列や0を使用するフォールバック
                    let fallback = self.get_fallback_value(&column.column_type);
                    select_expressions.push(fallback);
                }
            }
        }

        let insert_columns_str = insert_columns.join(", ");
        let select_expressions_str = select_expressions.join(", ");

        format!(
            "INSERT INTO {} ({}) SELECT {} FROM {}",
            quoted_new_table, insert_columns_str, select_expressions_str, quoted_table
        )
    }

    /// NOT NULLカラムのフォールバック値を取得
    fn get_fallback_value(&self, column_type: &ColumnType) -> String {
        match column_type {
            ColumnType::INTEGER { .. } => "0".to_string(),
            ColumnType::FLOAT | ColumnType::DOUBLE => "0.0".to_string(),
            ColumnType::BOOLEAN => "0".to_string(),
            ColumnType::VARCHAR { .. }
            | ColumnType::TEXT
            | ColumnType::CHAR { .. }
            | ColumnType::UUID => "''".to_string(),
            ColumnType::DECIMAL { .. } => "'0'".to_string(),
            ColumnType::DATE | ColumnType::TIME { .. } | ColumnType::TIMESTAMP { .. } => {
                "''".to_string()
            }
            ColumnType::JSON | ColumnType::JSONB => "'{}'".to_string(),
            ColumnType::BLOB => "X''".to_string(),
            ColumnType::Enum { .. } | ColumnType::DialectSpecific { .. } => "''".to_string(),
        }
    }

    /// CREATE INDEX文を生成
    fn generate_create_index(&self, table: &Table, index: &Index) -> String {
        let index_type = if index.unique {
            "UNIQUE INDEX"
        } else {
            "INDEX"
        };

        format!(
            "CREATE {} {} ON {} ({})",
            index_type,
            quote_identifier_sqlite(&index.name),
            quote_identifier_sqlite(&table.name),
            quote_columns_sqlite(&index.columns)
        )
    }
}

impl Default for SqliteTableRecreator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::Column;

    fn create_test_table() -> Table {
        let mut table = Table::new("users".to_string());
        table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table.columns.push(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            true,
        ));
        table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        table
    }

    #[test]
    fn test_new_recreator() {
        let recreator = SqliteTableRecreator::new();
        assert!(std::mem::size_of_val(&recreator) == 0); // Zero-size struct
    }

    #[test]
    fn test_generate_table_recreation_basic() {
        let recreator = SqliteTableRecreator::new();
        let table = create_test_table();

        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let statements = recreator.generate_table_recreation(&table, &diff, MigrationDirection::Up);

        // インデックスがない場合は9ステップ（インデックスがある場合は+インデックス数）
        assert!(statements.len() >= 9);
        assert_eq!(statements[0], "PRAGMA foreign_keys=off");
        assert_eq!(statements[1], "BEGIN TRANSACTION");
        assert!(statements[2].starts_with(r#"CREATE TABLE "new_users""#));
        assert!(statements[3].starts_with(r#"INSERT INTO "new_users""#));
        assert_eq!(statements[4], r#"DROP TABLE "users""#);
        assert_eq!(
            statements[5],
            r#"ALTER TABLE "new_users" RENAME TO "users""#
        );
        // インデックスがない場合、次はCOMMIT
        assert!(statements.contains(&"COMMIT".to_string()));
        assert!(statements.contains(&"PRAGMA foreign_keys=on".to_string()));
        assert!(statements.contains(&r#"PRAGMA foreign_key_check("users")"#.to_string()));
    }

    #[test]
    fn test_generate_table_recreation_with_indexes() {
        let recreator = SqliteTableRecreator::new();
        let mut table = create_test_table();
        table.indexes.push(Index::new(
            "idx_users_email".to_string(),
            vec!["email".to_string()],
            false,
        ));

        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let statements = recreator.generate_table_recreation(&table, &diff, MigrationDirection::Up);

        // インデックス再作成を確認
        assert!(statements
            .iter()
            .any(|s| s.contains(r#"CREATE INDEX "idx_users_email""#)));
    }

    #[test]
    fn test_generate_table_recreation_with_constraints() {
        let recreator = SqliteTableRecreator::new();
        let mut table = create_test_table();
        table.constraints.push(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });

        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let statements = recreator.generate_table_recreation(&table, &diff, MigrationDirection::Up);

        // CREATE TABLE内にUNIQUE制約が含まれることを確認
        let create_table_stmt = &statements[2];
        assert!(create_table_stmt.contains(r#"UNIQUE ("email")"#));
    }

    #[test]
    fn test_generate_data_copy_sql() {
        let recreator = SqliteTableRecreator::new();
        let table = create_test_table();

        // 旧テーブル情報なしの場合、新テーブルと同じカラムを仮定
        let sql = recreator.generate_data_copy_sql_with_column_intersection(&table, None);

        assert_eq!(
            sql,
            r#"INSERT INTO "new_users" ("id", "name", "email") SELECT "id", "name", "email" FROM "users""#
        );
    }

    #[test]
    fn test_generate_create_table_with_name() {
        let recreator = SqliteTableRecreator::new();
        let table = create_test_table();

        let sql = recreator.generate_create_table_with_name(&table, "new_users");

        assert!(sql.starts_with(r#"CREATE TABLE "new_users""#));
        assert!(sql.contains(r#""id" INTEGER NOT NULL"#));
        assert!(sql.contains(r#""name" TEXT NOT NULL"#));
        assert!(sql.contains(r#""email" TEXT"#));
        assert!(sql.contains(r#"PRIMARY KEY ("id")"#));
    }

    #[test]
    fn test_map_column_type() {
        let recreator = SqliteTableRecreator::new();

        assert_eq!(
            recreator.map_column_type(&ColumnType::INTEGER { precision: None }),
            "INTEGER"
        );
        assert_eq!(
            recreator.map_column_type(&ColumnType::VARCHAR { length: 255 }),
            "TEXT"
        );
        assert_eq!(recreator.map_column_type(&ColumnType::BOOLEAN), "INTEGER");
        assert_eq!(
            recreator.map_column_type(&ColumnType::TIMESTAMP {
                with_time_zone: None
            }),
            "TEXT"
        );
        assert_eq!(recreator.map_column_type(&ColumnType::JSON), "TEXT");
        assert_eq!(recreator.map_column_type(&ColumnType::BLOB), "BLOB");
    }

    #[test]
    fn test_down_direction() {
        let recreator = SqliteTableRecreator::new();
        let table = create_test_table();

        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        // Down方向でも同じ構造のSQLが生成される
        let statements =
            recreator.generate_table_recreation(&table, &diff, MigrationDirection::Down);

        assert!(statements.len() >= 9);
        assert_eq!(statements[0], "PRAGMA foreign_keys=off");
        assert!(statements.contains(&"COMMIT".to_string()));
    }

    // ==========================================
    // 列交差ベースのデータコピーテスト
    // ==========================================

    #[test]
    fn test_data_copy_with_column_intersection_same_columns() {
        let recreator = SqliteTableRecreator::new();
        let table = create_test_table();

        // 旧テーブルと同じカラム構成の場合
        let sql = recreator.generate_data_copy_sql_with_column_intersection(&table, Some(&table));

        assert_eq!(
            sql,
            r#"INSERT INTO "new_users" ("id", "name", "email") SELECT "id", "name", "email" FROM "users""#
        );
    }

    #[test]
    fn test_data_copy_with_added_nullable_column() {
        let recreator = SqliteTableRecreator::new();

        // 旧テーブル: id, name
        let mut old_table = Table::new("users".to_string());
        old_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        old_table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));

        // 新テーブル: id, name, bio (nullable追加)
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        new_table.columns.push(Column::new(
            "bio".to_string(),
            ColumnType::TEXT,
            true, // nullable
        ));

        let sql =
            recreator.generate_data_copy_sql_with_column_intersection(&new_table, Some(&old_table));

        // 追加されたnullableカラムにはNULLが入る
        assert_eq!(
            sql,
            r#"INSERT INTO "new_users" ("id", "name", "bio") SELECT "id", "name", NULL FROM "users""#
        );
    }

    #[test]
    fn test_data_copy_with_added_column_with_default() {
        let recreator = SqliteTableRecreator::new();

        // 旧テーブル: id, name
        let mut old_table = Table::new("users".to_string());
        old_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        old_table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));

        // 新テーブル: id, name, status (NOT NULL with default)
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        let mut status_col = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 20 },
            false, // NOT NULL
        );
        status_col.default_value = Some("'active'".to_string());
        new_table.columns.push(status_col);

        let sql =
            recreator.generate_data_copy_sql_with_column_intersection(&new_table, Some(&old_table));

        // 追加されたカラムにはDEFAULT値が入る
        assert_eq!(
            sql,
            r#"INSERT INTO "new_users" ("id", "name", "status") SELECT "id", "name", 'active' FROM "users""#
        );
    }

    #[test]
    fn test_data_copy_with_removed_column() {
        let recreator = SqliteTableRecreator::new();

        // 旧テーブル: id, name, email
        let old_table = create_test_table();

        // 新テーブル: id, name (emailを削除)
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));

        let sql =
            recreator.generate_data_copy_sql_with_column_intersection(&new_table, Some(&old_table));

        // 削除されたカラムはSELECTに含まれない
        assert_eq!(
            sql,
            r#"INSERT INTO "new_users" ("id", "name") SELECT "id", "name" FROM "users""#
        );
    }

    #[test]
    fn test_data_copy_with_not_null_no_default_fallback() {
        let recreator = SqliteTableRecreator::new();

        // 旧テーブル: id
        let mut old_table = Table::new("users".to_string());
        old_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        // 新テーブル: id, count (NOT NULL without default)
        let mut new_table = Table::new("users".to_string());
        new_table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        new_table.columns.push(Column::new(
            "count".to_string(),
            ColumnType::INTEGER { precision: None },
            false, // NOT NULL, no default
        ));

        let sql =
            recreator.generate_data_copy_sql_with_column_intersection(&new_table, Some(&old_table));

        // NOT NULLでDEFAULTがない場合はフォールバック値（0）
        assert_eq!(
            sql,
            r#"INSERT INTO "new_users" ("id", "count") SELECT "id", 0 FROM "users""#
        );
    }

    #[test]
    fn test_data_copy_without_old_table_info() {
        let recreator = SqliteTableRecreator::new();
        let table = create_test_table();

        // 旧テーブル情報がない場合は新テーブルと同じカラムを仮定
        let sql = recreator.generate_data_copy_sql_with_column_intersection(&table, None);

        assert_eq!(
            sql,
            r#"INSERT INTO "new_users" ("id", "name", "email") SELECT "id", "name", "email" FROM "users""#
        );
    }
}
