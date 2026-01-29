// MySQL用SQLジェネレーター
//
// スキーマ定義からMySQL用のDDL文を生成します。

use crate::adapters::sql_generator::{
    build_column_definition, format_check_constraint, generate_ck_constraint_name,
    generate_fk_constraint_name, generate_uq_constraint_name, quote_columns_mysql,
    quote_identifier_mysql, sanitize_sql_comment, validate_check_expression, MigrationDirection,
    SqlGenerator,
};
use crate::adapters::type_mapping::TypeMappingService;
use crate::core::config::Dialect;
use crate::core::schema::{Column, ColumnType, Constraint, Table};
use crate::core::schema_diff::{ColumnDiff, RenamedColumn};

/// MySQL用SQLジェネレーター
#[derive(Debug, Clone)]
pub struct MysqlSqlGenerator {
    type_mapping: TypeMappingService,
}

impl MysqlSqlGenerator {
    /// 新しいMysqlSqlGeneratorを作成
    pub fn new() -> Self {
        Self {
            type_mapping: TypeMappingService::new(Dialect::MySQL),
        }
    }

    /// ColumnTypeをMySQLの型文字列にマッピング
    ///
    /// TypeMappingServiceに委譲して型変換を行います。
    fn map_column_type(&self, column_type: &ColumnType, auto_increment: Option<bool>) -> String {
        self.type_mapping
            .to_sql_type_with_auto_increment(column_type, auto_increment)
    }

    /// MODIFY COLUMN用のカラム定義を生成
    ///
    /// MySQLのMODIFY COLUMNは完全なカラム定義が必要なため、
    /// target_columnの属性を使用してカラム定義を生成します。
    fn generate_column_definition_for_modify(
        &self,
        _table: &Table,
        column_name: &str,
        target_column: &Column,
    ) -> String {
        let type_str =
            self.map_column_type(&target_column.column_type, target_column.auto_increment);
        let auto_increment = if target_column.auto_increment.unwrap_or(false) {
            "AUTO_INCREMENT"
        } else {
            ""
        };
        let quoted_name = quote_identifier_mysql(column_name);
        build_column_definition(&quoted_name, target_column, type_str, &[auto_increment])
    }
}

impl SqlGenerator for MysqlSqlGenerator {
    fn quote_identifier(&self, name: &str) -> String {
        quote_identifier_mysql(name)
    }

    fn quote_columns(&self, columns: &[String]) -> String {
        quote_columns_mysql(columns)
    }

    fn generate_column_definition(&self, column: &Column) -> String {
        let type_str = self.map_column_type(&column.column_type, column.auto_increment);
        let auto_increment = if column.auto_increment.unwrap_or(false) {
            "AUTO_INCREMENT"
        } else {
            ""
        };
        let quoted_name = quote_identifier_mysql(&column.name);
        build_column_definition(&quoted_name, column, type_str, &[auto_increment])
    }

    fn generate_constraint_definition(&self, constraint: &Constraint) -> String {
        match constraint {
            Constraint::PRIMARY_KEY { columns } => {
                format!("PRIMARY KEY ({})", quote_columns_mysql(columns))
            }
            Constraint::UNIQUE { columns } => {
                format!("UNIQUE ({})", quote_columns_mysql(columns))
            }
            Constraint::CHECK {
                check_expression, ..
            } => {
                // MySQL 8.0.16以降でCHECK制約がサポートされる
                format_check_constraint(check_expression)
            }
            Constraint::FOREIGN_KEY { .. } => {
                // FOREIGN KEY制約はALTER TABLEで追加するため、ここでは空文字列を返す
                String::new()
            }
        }
    }

    fn generate_drop_index(&self, table_name: &str, index_name: &str) -> String {
        format!(
            "DROP INDEX {} ON {}",
            quote_identifier_mysql(index_name),
            quote_identifier_mysql(table_name)
        )
    }

    fn generate_rename_table(&self, old_name: &str, new_name: &str) -> String {
        format!(
            "RENAME TABLE {} TO {}",
            quote_identifier_mysql(old_name),
            quote_identifier_mysql(new_name)
        )
    }

    fn generate_alter_column_type(
        &self,
        table: &Table,
        column_diff: &ColumnDiff,
        direction: MigrationDirection,
    ) -> Vec<String> {
        let column_name = &column_diff.column_name;

        // 方向に応じて対象のカラム定義を決定
        let target_column = match direction {
            MigrationDirection::Up => &column_diff.new_column,
            MigrationDirection::Down => &column_diff.old_column,
        };

        // MODIFY COLUMNは完全なカラム定義が必要
        // target_columnの属性を使用してカラム定義を生成
        let column_def =
            self.generate_column_definition_for_modify(table, column_name, target_column);

        let sql = format!(
            "ALTER TABLE {} MODIFY COLUMN {}",
            quote_identifier_mysql(&table.name),
            column_def
        );

        vec![sql]
    }

    fn generate_alter_column_nullable(
        &self,
        table_name: &str,
        column: &Column,
        new_nullable: bool,
    ) -> Vec<String> {
        // MySQLではMODIFY COLUMNで完全なカラム定義を再指定する必要がある
        let mut target_column = column.clone();
        target_column.nullable = new_nullable;
        let table = Table::new(table_name.to_string());
        let col_def =
            self.generate_column_definition_for_modify(&table, &column.name, &target_column);
        vec![format!(
            "ALTER TABLE {} MODIFY COLUMN {}",
            quote_identifier_mysql(table_name),
            col_def
        )]
    }

    fn generate_alter_column_default(
        &self,
        table_name: &str,
        column: &Column,
        new_default: Option<&str>,
    ) -> Vec<String> {
        // MySQLではMODIFY COLUMNで完全なカラム定義を再指定する必要がある
        let mut target_column = column.clone();
        target_column.default_value = new_default.map(|s| s.to_string());
        let table = Table::new(table_name.to_string());
        let col_def =
            self.generate_column_definition_for_modify(&table, &column.name, &target_column);
        vec![format!(
            "ALTER TABLE {} MODIFY COLUMN {}",
            quote_identifier_mysql(table_name),
            col_def
        )]
    }

    fn generate_rename_column(
        &self,
        table: &Table,
        renamed_column: &RenamedColumn,
        direction: MigrationDirection,
    ) -> Vec<String> {
        // MySQLではCHANGE COLUMN構文を使用（完全なカラム定義が必要）
        // Up方向: old_name → new_name (new_columnの定義を使用)
        // Down方向: new_name → old_name (old_columnの定義を使用)
        let (from_name, to_column) = match direction {
            MigrationDirection::Up => (&renamed_column.old_name, &renamed_column.new_column),
            MigrationDirection::Down => {
                (&renamed_column.new_column.name, &renamed_column.old_column)
            }
        };

        let column_def =
            self.generate_column_definition_for_modify(table, &to_column.name, to_column);

        vec![format!(
            "ALTER TABLE {} CHANGE COLUMN {} {}",
            quote_identifier_mysql(&table.name),
            quote_identifier_mysql(from_name),
            column_def
        )]
    }

    fn generate_add_constraint_for_existing_table(
        &self,
        table_name: &str,
        constraint: &Constraint,
    ) -> String {
        match constraint {
            Constraint::FOREIGN_KEY {
                columns,
                referenced_table,
                referenced_columns,
                on_delete,
                on_update,
            } => {
                let constraint_name =
                    generate_fk_constraint_name(table_name, columns, referenced_table);

                let mut sql = format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
                    quote_identifier_mysql(table_name),
                    quote_identifier_mysql(&constraint_name),
                    quote_columns_mysql(columns),
                    quote_identifier_mysql(referenced_table),
                    quote_columns_mysql(referenced_columns)
                );

                if let Some(action) = on_delete {
                    sql.push_str(&format!(" ON DELETE {}", action.as_sql()));
                }
                if let Some(action) = on_update {
                    sql.push_str(&format!(" ON UPDATE {}", action.as_sql()));
                }

                sql
            }
            Constraint::UNIQUE { columns } => {
                let constraint_name = generate_uq_constraint_name(table_name, columns);

                format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} UNIQUE ({})",
                    quote_identifier_mysql(table_name),
                    quote_identifier_mysql(&constraint_name),
                    quote_columns_mysql(columns)
                )
            }
            Constraint::CHECK {
                columns,
                check_expression,
            } => {
                let constraint_name = generate_ck_constraint_name(table_name, columns);

                if let Err(msg) = validate_check_expression(check_expression) {
                    let sanitized_msg = sanitize_sql_comment(&msg);
                    return format!(
                        "/* ERROR: {} */ ALTER TABLE {} ADD CONSTRAINT {} CHECK (FALSE)",
                        sanitized_msg,
                        quote_identifier_mysql(table_name),
                        quote_identifier_mysql(&constraint_name),
                    );
                }
                format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} CHECK ({})",
                    quote_identifier_mysql(table_name),
                    quote_identifier_mysql(&constraint_name),
                    check_expression
                )
            }
            _ => {
                // PRIMARY_KEYは空文字列を返す
                String::new()
            }
        }
    }

    fn generate_drop_constraint_for_existing_table(
        &self,
        table_name: &str,
        constraint: &Constraint,
    ) -> String {
        match constraint {
            Constraint::FOREIGN_KEY {
                columns,
                referenced_table,
                ..
            } => {
                let constraint_name =
                    generate_fk_constraint_name(table_name, columns, referenced_table);

                // MySQLではDROP FOREIGN KEYを使用
                format!(
                    "ALTER TABLE {} DROP FOREIGN KEY {}",
                    quote_identifier_mysql(table_name),
                    quote_identifier_mysql(&constraint_name)
                )
            }
            Constraint::UNIQUE { columns } => {
                let constraint_name = generate_uq_constraint_name(table_name, columns);

                // MySQLではUNIQUE制約はDROP INDEXで削除
                format!(
                    "ALTER TABLE {} DROP INDEX {}",
                    quote_identifier_mysql(table_name),
                    quote_identifier_mysql(&constraint_name)
                )
            }
            Constraint::CHECK { columns, .. } => {
                let constraint_name = generate_ck_constraint_name(table_name, columns);

                // MySQL 8.0.16+: DROP CHECKで削除
                format!(
                    "ALTER TABLE {} DROP CHECK {}",
                    quote_identifier_mysql(table_name),
                    quote_identifier_mysql(&constraint_name)
                )
            }
            _ => {
                // PRIMARY_KEYは空文字列を返す
                String::new()
            }
        }
    }
}

impl Default for MysqlSqlGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_generator() {
        let generator = MysqlSqlGenerator::new();
        assert!(format!("{:?}", generator).contains("MysqlSqlGenerator"));
    }

    #[test]
    fn test_map_column_type_int() {
        let generator = MysqlSqlGenerator::new();
        let col_type = ColumnType::INTEGER { precision: None };
        assert_eq!(generator.map_column_type(&col_type, None), "INT");
    }

    #[test]
    fn test_map_column_type_bigint() {
        let generator = MysqlSqlGenerator::new();
        let col_type = ColumnType::INTEGER { precision: Some(8) };
        assert_eq!(generator.map_column_type(&col_type, None), "BIGINT");
    }

    #[test]
    fn test_map_column_type_varchar() {
        let generator = MysqlSqlGenerator::new();
        let col_type = ColumnType::VARCHAR { length: 255 };
        assert_eq!(generator.map_column_type(&col_type, None), "VARCHAR(255)");
    }

    #[test]
    fn test_map_column_type_boolean() {
        let generator = MysqlSqlGenerator::new();
        let col_type = ColumnType::BOOLEAN;
        assert_eq!(generator.map_column_type(&col_type, None), "BOOLEAN");
    }

    #[test]
    fn test_generate_column_definition() {
        let generator = MysqlSqlGenerator::new();
        let column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, "`name` VARCHAR(100) NOT NULL");
    }

    #[test]
    fn test_generate_column_definition_nullable() {
        let generator = MysqlSqlGenerator::new();
        let column = Column::new(
            "bio".to_string(),
            ColumnType::TEXT,
            true, // nullable
        );

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, "`bio` TEXT");
    }

    #[test]
    fn test_generate_column_definition_with_default() {
        let generator = MysqlSqlGenerator::new();
        let mut column = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 20 },
            false,
        );
        column.default_value = Some("'active'".to_string());

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, "`status` VARCHAR(20) NOT NULL DEFAULT 'active'");
    }

    #[test]
    fn test_generate_column_definition_with_auto_increment() {
        let generator = MysqlSqlGenerator::new();
        let mut column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        column.auto_increment = Some(true);

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, "`id` INT NOT NULL AUTO_INCREMENT");
    }

    #[test]
    fn test_generate_constraint_primary_key() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "PRIMARY KEY (`id`)");
    }

    #[test]
    fn test_generate_constraint_unique() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "UNIQUE (`email`)");
    }

    #[test]
    fn test_generate_constraint_check() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "CHECK (price >= 0)");
    }

    // ==========================================
    // generate_alter_column_type のテスト
    // ==========================================

    use crate::adapters::sql_generator::MigrationDirection;
    use crate::core::schema_diff::ColumnDiff;

    fn create_test_table_with_columns() -> Table {
        let mut table = Table::new("users".to_string());
        table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        let mut name_col = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        name_col.default_value = Some("'unknown'".to_string());
        table.columns.push(name_col);
        table
    }

    #[test]
    fn test_alter_column_type_basic() {
        let generator = MysqlSqlGenerator::new();
        let table = create_test_table_with_columns();

        // INTEGER → BIGINT
        let old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) },
            false,
        );
        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            "ALTER TABLE `users` MODIFY COLUMN `id` BIGINT NOT NULL"
        );
    }

    #[test]
    fn test_alter_column_type_with_nullable() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("users".to_string());
        let nullable_col = Column::new(
            "bio".to_string(),
            ColumnType::TEXT,
            true, // nullable
        );
        table.columns.push(nullable_col);

        // TEXT → VARCHAR (nullable)
        let old_column = Column::new("bio".to_string(), ColumnType::TEXT, true);
        let new_column = Column::new("bio".to_string(), ColumnType::VARCHAR { length: 500 }, true);
        let diff = ColumnDiff::new("bio".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        // NULLableなのでNOT NULLが含まれない
        assert_eq!(
            sql[0],
            "ALTER TABLE `users` MODIFY COLUMN `bio` VARCHAR(500)"
        );
    }

    #[test]
    fn test_alter_column_type_with_default() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("users".to_string());
        let mut col = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 20 },
            false,
        );
        col.default_value = Some("'active'".to_string());
        table.columns.push(col);

        // VARCHAR(20) → VARCHAR(50) with default
        let mut old_column = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 20 },
            false,
        );
        old_column.default_value = Some("'active'".to_string());
        let mut new_column = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 50 },
            false,
        );
        new_column.default_value = Some("'active'".to_string());
        let diff = ColumnDiff::new("status".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            "ALTER TABLE `users` MODIFY COLUMN `status` VARCHAR(50) NOT NULL DEFAULT 'active'"
        );
    }

    #[test]
    fn test_alter_column_type_with_auto_increment() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("users".to_string());
        let mut col = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        col.auto_increment = Some(true);
        table.columns.push(col);

        // INTEGER → BIGINT with AUTO_INCREMENT
        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(true);
        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) },
            false,
        );
        new_column.auto_increment = Some(true);
        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            "ALTER TABLE `users` MODIFY COLUMN `id` BIGINT NOT NULL AUTO_INCREMENT"
        );
    }

    #[test]
    fn test_alter_column_type_down_direction() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("users".to_string());
        table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        // Down方向: BIGINT → INTEGER に戻す
        let old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) },
            false,
        );
        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Down);

        assert_eq!(sql.len(), 1);
        // Down方向なので old_type (INT) に戻す
        assert_eq!(
            sql[0],
            "ALTER TABLE `users` MODIFY COLUMN `id` INT NOT NULL"
        );
    }

    #[test]
    fn test_alter_column_type_varchar_to_text() {
        let generator = MysqlSqlGenerator::new();
        let mut table = Table::new("posts".to_string());
        table
            .columns
            .push(Column::new("content".to_string(), ColumnType::TEXT, true));

        // VARCHAR → TEXT
        let old_column = Column::new(
            "content".to_string(),
            ColumnType::VARCHAR { length: 1000 },
            true,
        );
        let new_column = Column::new("content".to_string(), ColumnType::TEXT, true);
        let diff = ColumnDiff::new("content".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(sql[0], "ALTER TABLE `posts` MODIFY COLUMN `content` TEXT");
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
        // Up方向：old_name → new_name (CHANGE COLUMN構文)
        let generator = MysqlSqlGenerator::new();
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
        // MySQLではCHANGE COLUMN構文を使用（new_columnの定義を使用）
        assert_eq!(
            sql[0],
            "ALTER TABLE `users` CHANGE COLUMN `name` `user_name` VARCHAR(100) NOT NULL"
        );
    }

    #[test]
    fn test_generate_rename_column_down() {
        // Down方向：new_name → old_name（CHANGE COLUMN構文でロールバック）
        let generator = MysqlSqlGenerator::new();
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
        // Down方向ではold_columnの定義を使用してロールバック
        assert_eq!(
            sql[0],
            "ALTER TABLE `users` CHANGE COLUMN `user_name` `name` VARCHAR(100) NOT NULL"
        );
    }

    #[test]
    fn test_generate_rename_column_with_type_change() {
        // リネームと同時に型変更がある場合
        let generator = MysqlSqlGenerator::new();
        let table = create_test_table();

        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 200 }, // 型変更
            true,                                // nullable変更
        );
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![], // changesは別途処理されるため空でも可
        };

        let sql = generator.generate_rename_column(&table, &renamed, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        // 新しい定義（VARCHAR(200)、nullable）でリネーム
        assert_eq!(
            sql[0],
            "ALTER TABLE `users` CHANGE COLUMN `name` `user_name` VARCHAR(200)"
        );
    }

    // ==========================================
    // 制約メソッドのテスト
    // ==========================================

    #[test]
    fn test_generate_add_constraint_for_existing_table_foreign_key() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        };

        let sql = generator.generate_add_constraint_for_existing_table("posts", &constraint);

        assert_eq!(
            sql,
            "ALTER TABLE `posts` ADD CONSTRAINT `fk_posts_user_id_users` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`)"
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_composite_foreign_key() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["org_id".to_string(), "user_id".to_string()],
            referenced_table: "org_users".to_string(),
            referenced_columns: vec!["organization_id".to_string(), "user_id".to_string()],
            on_delete: None,
            on_update: None,
        };

        let sql = generator.generate_add_constraint_for_existing_table("posts", &constraint);

        assert_eq!(
            sql,
            "ALTER TABLE `posts` ADD CONSTRAINT `fk_posts_org_id_user_id_org_users` FOREIGN KEY (`org_id`, `user_id`) REFERENCES `org_users` (`organization_id`, `user_id`)"
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_unique() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let sql = generator.generate_add_constraint_for_existing_table("users", &constraint);

        assert_eq!(
            sql,
            "ALTER TABLE `users` ADD CONSTRAINT `uq_users_email` UNIQUE (`email`)"
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_unique_composite() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["first_name".to_string(), "last_name".to_string()],
        };

        let sql = generator.generate_add_constraint_for_existing_table("users", &constraint);

        assert_eq!(
            sql,
            "ALTER TABLE `users` ADD CONSTRAINT `uq_users_first_name_last_name` UNIQUE (`first_name`, `last_name`)"
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_check() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        };

        let sql = generator.generate_add_constraint_for_existing_table("products", &constraint);

        assert_eq!(
            sql,
            "ALTER TABLE `products` ADD CONSTRAINT `ck_products_price` CHECK (price >= 0)"
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_primary_key_returns_empty() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let sql = generator.generate_add_constraint_for_existing_table("users", &constraint);

        assert!(sql.is_empty());
    }

    #[test]
    fn test_generate_drop_constraint_for_existing_table_foreign_key() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        };

        let sql = generator.generate_drop_constraint_for_existing_table("posts", &constraint);

        // MySQLではDROP FOREIGN KEYを使用
        assert_eq!(
            sql,
            "ALTER TABLE `posts` DROP FOREIGN KEY `fk_posts_user_id_users`"
        );
    }

    #[test]
    fn test_generate_drop_constraint_for_existing_table_unique() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let sql = generator.generate_drop_constraint_for_existing_table("users", &constraint);

        // MySQLではUNIQUE制約はDROP INDEXで削除
        assert_eq!(sql, "ALTER TABLE `users` DROP INDEX `uq_users_email`");
    }

    #[test]
    fn test_generate_drop_constraint_for_existing_table_check() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        };

        let sql = generator.generate_drop_constraint_for_existing_table("products", &constraint);

        // MySQLではCHECK制約はDROP CHECKで削除
        assert_eq!(sql, "ALTER TABLE `products` DROP CHECK `ck_products_price`");
    }

    #[test]
    fn test_generate_drop_constraint_for_existing_table_primary_key_returns_empty() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let sql = generator.generate_drop_constraint_for_existing_table("users", &constraint);

        assert!(sql.is_empty());
    }
}
