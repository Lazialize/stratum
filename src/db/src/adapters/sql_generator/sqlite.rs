// SQLite用SQLジェネレーター
//
// スキーマ定義からSQLite用のDDL文を生成します。
// SQLiteはALTER TABLEの機能が制限されているため、制約はCREATE TABLE内で定義します。

use crate::adapters::sql_generator::sqlite_table_recreator::SqliteTableRecreator;
use crate::adapters::sql_generator::{
    build_column_definition, format_check_constraint, quote_columns_sqlite,
    quote_identifier_sqlite, MigrationDirection, SqlGenerator,
};
use crate::adapters::type_mapping::TypeMappingService;
use crate::core::config::Dialect;
use crate::core::schema::{Column, ColumnType, Constraint, Table};
use crate::core::schema_diff::{ColumnDiff, RenamedColumn};

/// SQLite用SQLジェネレーター
#[derive(Debug, Clone)]
pub struct SqliteSqlGenerator {
    type_mapping: TypeMappingService,
}

impl SqliteSqlGenerator {
    /// 新しいSqliteSqlGeneratorを作成
    pub fn new() -> Self {
        Self {
            type_mapping: TypeMappingService::new(Dialect::SQLite),
        }
    }

    /// ColumnTypeをSQLiteの型文字列にマッピング
    ///
    /// TypeMappingServiceに委譲して型変換を行います。
    fn map_column_type(&self, column_type: &ColumnType) -> String {
        self.type_mapping.to_sql_type(column_type)
    }
}

impl SqlGenerator for SqliteSqlGenerator {
    fn quote_identifier(&self, name: &str) -> String {
        quote_identifier_sqlite(name)
    }

    fn quote_columns(&self, columns: &[String]) -> String {
        quote_columns_sqlite(columns)
    }

    fn generate_column_definition(&self, column: &Column) -> String {
        let type_str = self.map_column_type(&column.column_type);
        let quoted_name = quote_identifier_sqlite(&column.name);
        build_column_definition(&quoted_name, column, type_str, &[])
    }

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
            } => format_check_constraint(check_expression),
            Constraint::FOREIGN_KEY {
                columns,
                referenced_table,
                referenced_columns,
                on_delete,
                on_update,
            } => {
                // SQLiteではFOREIGN KEYをCREATE TABLE内で定義
                let mut sql = format!(
                    "FOREIGN KEY ({}) REFERENCES {} ({})",
                    quote_columns_sqlite(columns),
                    quote_identifier_sqlite(referenced_table),
                    quote_columns_sqlite(referenced_columns)
                );

                if let Some(action) = on_delete {
                    sql.push_str(&format!(" ON DELETE {}", action.as_sql()));
                }
                if let Some(action) = on_update {
                    sql.push_str(&format!(" ON UPDATE {}", action.as_sql()));
                }

                sql
            }
        }
    }

    /// SQLiteは全制約をCREATE TABLE内で定義
    fn should_add_as_table_constraint(&self, _constraint: &Constraint) -> bool {
        true
    }

    fn generate_alter_table_add_constraint(
        &self,
        _table: &Table,
        _constraint_index: usize,
    ) -> String {
        // SQLiteはALTER TABLE ADD CONSTRAINTをサポートしていない
        // すべての制約はCREATE TABLE内で定義する必要がある
        String::new()
    }

    /// カラム型変更のALTER TABLE文を生成（SQLite）
    ///
    /// SQLiteはALTER COLUMN TYPEをサポートしていないため、
    /// テーブル再作成パターンを使用します。
    ///
    /// # Arguments
    ///
    /// * `table` - 対象テーブルの完全な定義（direction=Upなら新定義、Downなら旧定義）
    /// * `column_diff` - カラム差分情報
    /// * `direction` - マイグレーション方向（Up/Down）
    ///
    /// # Returns
    ///
    /// テーブル再作成SQLのベクター
    fn generate_alter_column_type(
        &self,
        table: &Table,
        column_diff: &ColumnDiff,
        direction: MigrationDirection,
    ) -> Vec<String> {
        let recreator = SqliteTableRecreator::new();
        recreator.generate_table_recreation(table, column_diff, direction)
    }

    /// カラム型変更のALTER TABLE文を生成（旧テーブル情報付き、SQLite）
    ///
    /// SQLiteはテーブル再作成パターンを使用するため、旧テーブル情報を活用して
    /// 列交差ベースのデータコピーSQLを生成します。
    ///
    /// # Arguments
    ///
    /// * `table` - 対象テーブルの新しい定義
    /// * `old_table` - 対象テーブルの古い定義（列交差のための参照）
    /// * `column_diff` - カラム差分情報
    /// * `direction` - マイグレーション方向（Up/Down）
    ///
    /// # Returns
    ///
    /// テーブル再作成SQLのベクター
    fn generate_alter_column_type_with_old_table(
        &self,
        table: &Table,
        old_table: Option<&Table>,
        _column_diff: &ColumnDiff,
        _direction: MigrationDirection,
    ) -> Vec<String> {
        let recreator = SqliteTableRecreator::new();
        recreator.generate_table_recreation_with_old_table(table, old_table)
    }

    fn generate_rename_column(
        &self,
        table: &Table,
        renamed_column: &RenamedColumn,
        direction: MigrationDirection,
    ) -> Vec<String> {
        // SQLite 3.25.0以降はALTER TABLE RENAME COLUMNをサポート
        let (from_name, to_name) = match direction {
            MigrationDirection::Up => (&renamed_column.old_name, &renamed_column.new_column.name),
            MigrationDirection::Down => (&renamed_column.new_column.name, &renamed_column.old_name),
        };

        vec![format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {}",
            quote_identifier_sqlite(&table.name),
            quote_identifier_sqlite(from_name),
            quote_identifier_sqlite(to_name)
        )]
    }

    /// SQLiteでは CREATE OR REPLACE VIEW が使えないため DROP + CREATE を使用
    fn generate_create_view(&self, view_name: &str, definition: &str) -> String {
        format!(
            "DROP VIEW IF EXISTS {};\n\nCREATE VIEW {} AS\n{}",
            quote_identifier_sqlite(view_name),
            quote_identifier_sqlite(view_name),
            definition
        )
    }

    /// SQLiteでは ALTER VIEW RENAME TO が使えないため DROP + CREATE を使用
    fn generate_rename_view(&self, old_name: &str, _new_name: &str) -> String {
        // SQLiteではビューリネームはサポートされない。
        // 呼び出し元で DROP + CREATE の組み合わせが使われることを想定。
        // migration pipeline側で DROP + CREATE に変換する。
        format!(
            "-- SQLite does not support ALTER VIEW RENAME. Use DROP + CREATE instead.\nDROP VIEW IF EXISTS {}",
            quote_identifier_sqlite(old_name),
        )
    }
}

impl Default for SqliteSqlGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::Index;
    use crate::core::schema_diff::ColumnChange;

    #[test]
    fn test_new_generator() {
        let generator = SqliteSqlGenerator::new();
        assert!(format!("{:?}", generator).contains("SqliteSqlGenerator"));
    }

    #[test]
    fn test_map_column_type_integer() {
        let generator = SqliteSqlGenerator::new();
        let col_type = ColumnType::INTEGER { precision: None };
        assert_eq!(generator.map_column_type(&col_type), "INTEGER");
    }

    #[test]
    fn test_map_column_type_varchar() {
        let generator = SqliteSqlGenerator::new();
        let col_type = ColumnType::VARCHAR { length: 255 };
        assert_eq!(generator.map_column_type(&col_type), "TEXT");
    }

    #[test]
    fn test_map_column_type_boolean() {
        let generator = SqliteSqlGenerator::new();
        let col_type = ColumnType::BOOLEAN;
        assert_eq!(generator.map_column_type(&col_type), "INTEGER");
    }

    #[test]
    fn test_map_column_type_timestamp() {
        let generator = SqliteSqlGenerator::new();
        let col_type = ColumnType::TIMESTAMP {
            with_time_zone: Some(false),
        };
        assert_eq!(generator.map_column_type(&col_type), "TEXT");
    }

    #[test]
    fn test_map_column_type_json() {
        let generator = SqliteSqlGenerator::new();
        let col_type = ColumnType::JSON;
        assert_eq!(generator.map_column_type(&col_type), "TEXT");
    }

    #[test]
    fn test_generate_column_definition() {
        let generator = SqliteSqlGenerator::new();
        let column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, r#""name" TEXT NOT NULL"#);
    }

    #[test]
    fn test_generate_column_definition_nullable() {
        let generator = SqliteSqlGenerator::new();
        let column = Column::new(
            "bio".to_string(),
            ColumnType::TEXT,
            true, // nullable
        );

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, r#""bio" TEXT"#);
    }

    #[test]
    fn test_generate_column_definition_with_default() {
        let generator = SqliteSqlGenerator::new();
        let mut column = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 20 },
            false,
        );
        column.default_value = Some("'active'".to_string());

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, r#""status" TEXT NOT NULL DEFAULT 'active'"#);
    }

    #[test]
    fn test_generate_constraint_primary_key() {
        let generator = SqliteSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, r#"PRIMARY KEY ("id")"#);
    }

    #[test]
    fn test_generate_constraint_unique() {
        let generator = SqliteSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, r#"UNIQUE ("email")"#);
    }

    #[test]
    fn test_generate_constraint_check() {
        let generator = SqliteSqlGenerator::new();
        let constraint = Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "CHECK (price >= 0)");
    }

    #[test]
    fn test_generate_constraint_foreign_key() {
        let generator = SqliteSqlGenerator::new();
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, r#"FOREIGN KEY ("user_id") REFERENCES "users" ("id")"#);
    }

    #[test]
    fn test_generate_alter_table_returns_empty() {
        let generator = SqliteSqlGenerator::new();
        let table = Table::new("test".to_string());

        // SQLiteはALTER TABLE ADD CONSTRAINTをサポートしていない
        let sql = generator.generate_alter_table_add_constraint(&table, 0);
        assert_eq!(sql, "");
    }

    // ==========================================================
    // generate_alter_column_type テスト
    // ==========================================================

    fn create_test_table_with_columns() -> Table {
        let mut table = Table::new("users".to_string());
        table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        table.columns.push(Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            true,
        ));
        table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        table
    }

    #[test]
    fn test_generate_alter_column_type_up_direction() {
        let generator = SqliteSqlGenerator::new();
        let table = create_test_table_with_columns();

        let old_column = Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            true,
        );
        let new_column = Column::new("age".to_string(), ColumnType::VARCHAR { length: 50 }, true);

        let column_diff = ColumnDiff {
            column_name: "age".to_string(),
            old_column,
            new_column,
            changes: vec![ColumnChange::TypeChanged {
                old_type: "INTEGER".to_string(),
                new_type: "VARCHAR(50)".to_string(),
            }],
        };

        let statements =
            generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

        // テーブル再作成パターンを使用するため、複数のSQL文が生成される
        assert!(statements.len() >= 9);
        assert_eq!(statements[0], "PRAGMA foreign_keys=off");
        assert_eq!(statements[1], "BEGIN TRANSACTION");
        assert!(statements[2].contains(r#"CREATE TABLE "_stratum_tmp_recreate_users""#));
        assert!(statements[3].contains(r#"INSERT INTO "_stratum_tmp_recreate_users""#));
        assert!(statements[4].contains(r#"DROP TABLE "users""#));
        assert!(statements[5]
            .contains(r#"ALTER TABLE "_stratum_tmp_recreate_users" RENAME TO "users""#));
    }

    #[test]
    fn test_generate_alter_column_type_down_direction() {
        let generator = SqliteSqlGenerator::new();

        // Down方向では古いスキーマを使用
        let mut table = Table::new("users".to_string());
        table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.columns.push(Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            true,
        ));
        table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        let old_column = Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            true,
        );
        let new_column = Column::new("age".to_string(), ColumnType::VARCHAR { length: 50 }, true);

        let column_diff = ColumnDiff {
            column_name: "age".to_string(),
            old_column,
            new_column,
            changes: vec![ColumnChange::TypeChanged {
                old_type: "INTEGER".to_string(),
                new_type: "VARCHAR(50)".to_string(),
            }],
        };

        let statements =
            generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Down);

        // テーブル再作成パターンを使用
        assert!(statements.len() >= 9);
        assert_eq!(statements[0], "PRAGMA foreign_keys=off");

        // Down方向なのでINTEGERに戻す
        assert!(statements[2].contains("INTEGER"));
    }

    #[test]
    fn test_generate_alter_column_type_with_indexes() {
        let generator = SqliteSqlGenerator::new();
        let mut table = create_test_table_with_columns();

        // インデックスを追加
        table.indexes.push(Index {
            name: "idx_users_name".to_string(),
            columns: vec!["name".to_string()],
            unique: false,
        });

        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new("name".to_string(), ColumnType::TEXT, false);

        let column_diff = ColumnDiff {
            column_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![ColumnChange::TypeChanged {
                old_type: "VARCHAR(100)".to_string(),
                new_type: "TEXT".to_string(),
            }],
        };

        let statements =
            generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

        // インデックスがある場合、再作成ステートメントが含まれる
        assert!(statements.len() >= 10);

        // インデックス再作成が含まれているか確認
        let has_create_index = statements.iter().any(|s| s.contains("CREATE INDEX"));
        assert!(has_create_index);
    }

    #[test]
    fn test_generate_alter_column_type_preserves_constraints() {
        let generator = SqliteSqlGenerator::new();
        let table = create_test_table_with_columns();

        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );

        let column_diff = ColumnDiff {
            column_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![ColumnChange::TypeChanged {
                old_type: "VARCHAR(100)".to_string(),
                new_type: "VARCHAR(255)".to_string(),
            }],
        };

        let statements =
            generator.generate_alter_column_type(&table, &column_diff, MigrationDirection::Up);

        // PRIMARY KEY制約が保持されていることを確認
        let create_table_stmt = &statements[2];
        assert!(create_table_stmt.contains("PRIMARY KEY"));
    }

    // ==========================================
    // generate_rename_column のテスト
    // ==========================================

    use crate::core::schema_diff::RenamedColumn;

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
        table
    }

    #[test]
    fn test_generate_rename_column_up() {
        // Up方向：old_name → new_name
        let generator = SqliteSqlGenerator::new();
        let table = create_test_table();

        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let sql = generator.generate_rename_column(&table, &renamed, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" RENAME COLUMN "name" TO "user_name""#
        );
    }

    #[test]
    fn test_generate_rename_column_down() {
        // Down方向：new_name → old_name（逆リネーム）
        let generator = SqliteSqlGenerator::new();
        let table = create_test_table();

        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let sql = generator.generate_rename_column(&table, &renamed, MigrationDirection::Down);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" RENAME COLUMN "user_name" TO "name""#
        );
    }
}
