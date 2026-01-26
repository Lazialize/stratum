// PostgreSQL用SQLジェネレーター
//
// スキーマ定義からPostgreSQL用のDDL文を生成します。

use crate::adapters::sql_generator::{
    build_column_definition, generate_fk_constraint_name, MigrationDirection, SqlGenerator,
};
use crate::adapters::type_mapping::TypeMappingService;
use crate::core::config::Dialect;
use crate::core::schema::{Column, ColumnType, Constraint, EnumDefinition, Index, Table};
use crate::core::schema_diff::{ColumnDiff, EnumDiff, RenamedColumn};
use crate::core::type_category::TypeCategory;

/// PostgreSQL用SQLジェネレーター
#[derive(Debug, Clone)]
pub struct PostgresSqlGenerator {}

impl PostgresSqlGenerator {
    /// 新しいPostgresSqlGeneratorを作成
    pub fn new() -> Self {
        Self {}
    }

    /// カラム定義のSQL文字列を生成
    fn generate_column_definition(&self, column: &Column) -> String {
        let type_str = self.map_column_type(&column.column_type, column.auto_increment);
        build_column_definition(column, type_str, &[])
    }

    /// ColumnTypeをPostgreSQLの型文字列にマッピング
    ///
    /// TypeMappingServiceに委譲して型変換を行います。
    fn map_column_type(&self, column_type: &ColumnType, auto_increment: Option<bool>) -> String {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        service.to_sql_type_with_auto_increment(column_type, auto_increment)
    }

    /// 制約定義のSQL文字列を生成
    fn generate_constraint_definition(&self, constraint: &Constraint) -> String {
        match constraint {
            Constraint::PRIMARY_KEY { columns } => {
                format!("PRIMARY KEY ({})", columns.join(", "))
            }
            Constraint::UNIQUE { columns } => {
                format!("UNIQUE ({})", columns.join(", "))
            }
            Constraint::CHECK {
                check_expression, ..
            } => {
                format!("CHECK ({})", check_expression)
            }
            Constraint::FOREIGN_KEY { .. } => {
                // FOREIGN KEY制約はALTER TABLEで追加するため、ここでは空文字列を返す
                String::new()
            }
        }
    }

    /// テーブル制約として追加する制約かどうかを判定
    fn should_add_as_table_constraint(&self, constraint: &Constraint) -> bool {
        !matches!(constraint, Constraint::FOREIGN_KEY { .. })
    }

    /// ENUM値をフォーマット
    fn format_enum_values(&self, values: &[String]) -> String {
        values
            .iter()
            .map(|value| format!("'{}'", self.escape_enum_value(value)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// ENUM値をエスケープ
    fn escape_enum_value(&self, value: &str) -> String {
        value.replace('\'', "''")
    }

    /// USING句が必要かどうかを判定
    ///
    /// TypeCategoryベースでUSING句の自動生成を判定します。
    /// design.mdの「USING句生成ルール」に基づく実装。
    fn needs_using_clause(&self, source_type: &ColumnType, target_type: &ColumnType) -> bool {
        let source_category = TypeCategory::from_column_type(source_type);
        let target_category = TypeCategory::from_column_type(target_type);

        use TypeCategory::*;

        match (source_category, target_category) {
            // 同一カテゴリ内: 不要
            (Numeric, Numeric)
            | (String, String)
            | (DateTime, DateTime)
            | (Binary, Binary)
            | (Json, Json)
            | (Boolean, Boolean)
            | (Uuid, Uuid) => false,

            // String → Numeric/Boolean/DateTime/Json: 必要
            (String, Numeric) | (String, Boolean) | (String, DateTime) | (String, Json) => true,

            // Numeric → String: 不要（暗黙変換）
            (Numeric, String) => false,

            // DateTime → String: 不要（暗黙変換）
            (DateTime, String) => false,

            // Boolean → Numeric/String: 不要（暗黙変換）
            (Boolean, Numeric) | (Boolean, String) => false,

            // Uuid → String: 不要（暗黙変換）
            (Uuid, String) => false,

            // Json → String: 不要（暗黙変換）
            (Json, String) => false,

            // Binary → String: 不要（暗黙変換）
            (Binary, String) => false,

            // Otherカテゴリ: 安全のためUSING句を付与
            (Other, _) | (_, Other) => true,

            // その他の変換: 安全のためUSING句を付与
            _ => true,
        }
    }
}

impl SqlGenerator for PostgresSqlGenerator {
    fn generate_add_column(&self, table_name: &str, column: &Column) -> String {
        format!(
            "ALTER TABLE {} ADD COLUMN {}",
            table_name,
            self.generate_column_definition(column)
        )
    }

    fn generate_drop_column(&self, table_name: &str, column_name: &str) -> String {
        format!("ALTER TABLE {} DROP COLUMN {}", table_name, column_name)
    }

    fn generate_drop_table(&self, table_name: &str) -> String {
        format!("DROP TABLE {}", table_name)
    }

    fn generate_drop_index(&self, _table_name: &str, index: &Index) -> String {
        format!("DROP INDEX {}", index.name)
    }

    fn generate_create_enum_type(&self, enum_def: &EnumDefinition) -> Vec<String> {
        let values = self.format_enum_values(&enum_def.values);
        vec![format!(
            "CREATE TYPE {} AS ENUM ({})",
            enum_def.name, values
        )]
    }

    fn generate_add_enum_value(&self, enum_name: &str, value: &str) -> Vec<String> {
        vec![format!(
            "ALTER TYPE {} ADD VALUE '{}'",
            enum_name,
            self.escape_enum_value(value)
        )]
    }

    fn generate_recreate_enum_type(&self, enum_diff: &EnumDiff) -> Vec<String> {
        let old_name = format!("{}_old", enum_diff.enum_name);
        let values = self.format_enum_values(&enum_diff.new_values);
        let mut statements = Vec::new();

        statements.push(format!(
            "ALTER TYPE {} RENAME TO {}",
            enum_diff.enum_name, old_name
        ));
        statements.push(format!(
            "CREATE TYPE {} AS ENUM ({})",
            enum_diff.enum_name, values
        ));

        for column in &enum_diff.columns {
            statements.push(format!(
                "ALTER TABLE {} ALTER COLUMN {} TYPE {} USING {}::text::{}",
                column.table_name,
                column.column_name,
                enum_diff.enum_name,
                column.column_name,
                enum_diff.enum_name
            ));
        }

        statements.push(format!("DROP TYPE {}", old_name));
        statements
    }

    fn generate_drop_enum_type(&self, enum_name: &str) -> Vec<String> {
        vec![format!("DROP TYPE {}", enum_name)]
    }

    fn generate_alter_column_type(
        &self,
        table: &Table,
        column_diff: &ColumnDiff,
        direction: MigrationDirection,
    ) -> Vec<String> {
        let column_name = &column_diff.column_name;

        // 方向に応じて対象の型とauto_incrementフラグを決定
        let (source_type, target_type, source_auto_increment, target_auto_increment) =
            match direction {
                MigrationDirection::Up => (
                    &column_diff.old_column.column_type,
                    &column_diff.new_column.column_type,
                    column_diff.old_column.auto_increment,
                    column_diff.new_column.auto_increment,
                ),
                MigrationDirection::Down => (
                    &column_diff.new_column.column_type,
                    &column_diff.old_column.column_type,
                    column_diff.new_column.auto_increment,
                    column_diff.old_column.auto_increment,
                ),
            };

        let mut statements = Vec::new();

        // auto_incrementの変更を検出
        let source_is_auto = source_auto_increment.unwrap_or(false);
        let target_is_auto = target_auto_increment.unwrap_or(false);

        // 型変更の処理
        // auto_incrementの変更と同時に型変更がある場合も処理する（例: INTEGER→BIGSERIAL）
        // 型変更はシーケンス作成より先に実行（型の不一致を避けるため）
        let has_type_change = source_type != target_type;
        if has_type_change {
            // auto_incrementがtrueの場合、SERIAL系の型名ではなく基底の整数型を使用
            // （シーケンス設定は下記で別途処理）
            let target_type_str = if target_is_auto {
                // SERIAL変換時は基底型（INTEGER/BIGINT/SMALLINT）で型変更
                self.map_column_type(target_type, Some(false))
            } else {
                self.map_column_type(target_type, target_auto_increment)
            };

            let needs_using = self.needs_using_clause(source_type, target_type);

            let sql = if needs_using {
                format!(
                    "ALTER TABLE {} ALTER COLUMN {} TYPE {} USING {}::{}",
                    table.name, column_name, target_type_str, column_name, target_type_str
                )
            } else {
                format!(
                    "ALTER TABLE {} ALTER COLUMN {} TYPE {}",
                    table.name, column_name, target_type_str
                )
            };
            statements.push(sql);
        }

        // INTEGER → SERIAL (auto_increment: false → true)
        // PostgreSQLではALTER COLUMN TYPE SERIALは使用できないため、
        // シーケンスの作成とDEFAULT設定で対応
        if !source_is_auto && target_is_auto {
            let sequence_name = format!("{}_{}_seq", table.name, column_name);
            statements.push(format!("CREATE SEQUENCE IF NOT EXISTS {}", sequence_name));
            // 既存データがある場合に備えてシーケンスを最大値に初期化
            // COALESCE(..., 0) により空テーブルでは nextval() が 1 を返す
            // 第3引数 true により次の nextval() は max+1 を返す
            statements.push(format!(
                "SELECT setval('{}', COALESCE((SELECT MAX({}) FROM {}), 0), true)",
                sequence_name, column_name, table.name
            ));
            statements.push(format!(
                "ALTER TABLE {} ALTER COLUMN {} SET DEFAULT nextval('{}')",
                table.name, column_name, sequence_name
            ));
            statements.push(format!(
                "ALTER SEQUENCE {} OWNED BY {}.{}",
                sequence_name, table.name, column_name
            ));
        }

        // SERIAL → INTEGER (auto_increment: true → false)
        // シーケンスはこのカラム専用として作成されたものと仮定し、
        // DROP SEQUENCE IF EXISTS CASCADE で安全に削除を試みる
        if source_is_auto && !target_is_auto {
            statements.push(format!(
                "ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT",
                table.name, column_name
            ));
            let sequence_name = format!("{}_{}_seq", table.name, column_name);
            statements.push(format!("DROP SEQUENCE IF EXISTS {} CASCADE", sequence_name));
        }

        statements
    }

    fn generate_create_table(&self, table: &Table) -> String {
        let mut parts = Vec::new();

        parts.push(format!("CREATE TABLE {}", table.name));
        parts.push("(".to_string());

        let mut elements = Vec::new();

        // カラム定義
        for column in &table.columns {
            elements.push(format!("    {}", self.generate_column_definition(column)));
        }

        // テーブル制約（FOREIGN KEY以外）
        for constraint in &table.constraints {
            if self.should_add_as_table_constraint(constraint) {
                let constraint_def = self.generate_constraint_definition(constraint);
                if !constraint_def.is_empty() {
                    elements.push(format!("    {}", constraint_def));
                }
            }
        }

        parts.push(elements.join(",\n"));
        parts.push(")".to_string());

        parts.join("\n")
    }

    fn generate_create_index(&self, table: &Table, index: &Index) -> String {
        let index_type = if index.unique {
            "UNIQUE INDEX"
        } else {
            "INDEX"
        };

        format!(
            "CREATE {} {} ON {} ({})",
            index_type,
            index.name,
            table.name,
            index.columns.join(", ")
        )
    }

    fn generate_alter_table_add_constraint(
        &self,
        table: &Table,
        constraint_index: usize,
    ) -> String {
        if let Some(constraint) = table.constraints.get(constraint_index) {
            match constraint {
                Constraint::FOREIGN_KEY {
                    columns,
                    referenced_table,
                    referenced_columns,
                } => {
                    let constraint_name =
                        generate_fk_constraint_name(&table.name, columns, referenced_table);

                    format!(
                        "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
                        table.name,
                        constraint_name,
                        columns.join(", "),
                        referenced_table,
                        referenced_columns.join(", ")
                    )
                }
                _ => {
                    // FOREIGN KEY以外の制約はCREATE TABLEで定義されるため、ここでは空文字列
                    String::new()
                }
            }
        } else {
            String::new()
        }
    }

    fn generate_rename_column(
        &self,
        table: &Table,
        renamed_column: &RenamedColumn,
        direction: MigrationDirection,
    ) -> Vec<String> {
        let (from_name, to_name) = match direction {
            MigrationDirection::Up => (&renamed_column.old_name, &renamed_column.new_column.name),
            MigrationDirection::Down => (&renamed_column.new_column.name, &renamed_column.old_name),
        };

        vec![format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {}",
            table.name, from_name, to_name
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
            } => {
                let constraint_name =
                    generate_fk_constraint_name(table_name, columns, referenced_table);

                format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
                    table_name,
                    constraint_name,
                    columns.join(", "),
                    referenced_table,
                    referenced_columns.join(", ")
                )
            }
            _ => {
                // FOREIGN KEY以外の制約は現時点ではサポートしない
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

                format!(
                    "ALTER TABLE {} DROP CONSTRAINT IF EXISTS {}",
                    table_name, constraint_name
                )
            }
            _ => {
                // FOREIGN KEY以外の制約は現時点ではサポートしない
                String::new()
            }
        }
    }
}

impl Default for PostgresSqlGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_generator() {
        let generator = PostgresSqlGenerator::new();
        assert!(format!("{:?}", generator).contains("PostgresSqlGenerator"));
    }

    #[test]
    fn test_map_column_type_integer() {
        let generator = PostgresSqlGenerator::new();
        let col_type = ColumnType::INTEGER { precision: None };
        assert_eq!(generator.map_column_type(&col_type, None), "INTEGER");
    }

    #[test]
    fn test_map_column_type_serial() {
        let generator = PostgresSqlGenerator::new();
        let col_type = ColumnType::INTEGER { precision: None };
        assert_eq!(generator.map_column_type(&col_type, Some(true)), "SERIAL");
    }

    #[test]
    fn test_map_column_type_varchar() {
        let generator = PostgresSqlGenerator::new();
        let col_type = ColumnType::VARCHAR { length: 255 };
        assert_eq!(generator.map_column_type(&col_type, None), "VARCHAR(255)");
    }

    #[test]
    fn test_map_column_type_timestamp_with_tz() {
        let generator = PostgresSqlGenerator::new();
        let col_type = ColumnType::TIMESTAMP {
            with_time_zone: Some(true),
        };
        assert_eq!(
            generator.map_column_type(&col_type, None),
            "TIMESTAMP WITH TIME ZONE"
        );
    }

    #[test]
    fn test_generate_column_definition() {
        let generator = PostgresSqlGenerator::new();
        let column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, "name VARCHAR(100) NOT NULL");
    }

    #[test]
    fn test_generate_column_definition_nullable() {
        let generator = PostgresSqlGenerator::new();
        let column = Column::new(
            "bio".to_string(),
            ColumnType::TEXT,
            true, // nullable
        );

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, "bio TEXT");
    }

    #[test]
    fn test_generate_column_definition_with_default() {
        let generator = PostgresSqlGenerator::new();
        let mut column = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 20 },
            false,
        );
        column.default_value = Some("'active'".to_string());

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, "status VARCHAR(20) NOT NULL DEFAULT 'active'");
    }

    #[test]
    fn test_generate_constraint_primary_key() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "PRIMARY KEY (id)");
    }

    #[test]
    fn test_generate_constraint_unique() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "UNIQUE (email)");
    }

    #[test]
    fn test_generate_constraint_check() {
        let generator = PostgresSqlGenerator::new();
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
    fn test_alter_column_type_same_category_no_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // INTEGER → BIGINT（同じNumericカテゴリ内）
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
        assert_eq!(sql[0], "ALTER TABLE users ALTER COLUMN id TYPE BIGINT");
    }

    #[test]
    fn test_alter_column_type_numeric_to_string_no_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // INTEGER → TEXT（暗黙変換可能）
        let old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let new_column = Column::new("id".to_string(), ColumnType::TEXT, false);
        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(sql[0], "ALTER TABLE users ALTER COLUMN id TYPE TEXT");
    }

    #[test]
    fn test_alter_column_type_string_to_numeric_with_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // TEXT → INTEGER（USING句が必要）
        let old_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let new_column = Column::new(
            "name".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            "ALTER TABLE users ALTER COLUMN name TYPE INTEGER USING name::INTEGER"
        );
    }

    #[test]
    fn test_alter_column_type_string_to_boolean_with_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // VARCHAR → BOOLEAN（USING句が必要）
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 10 },
            false,
        );
        let new_column = Column::new("name".to_string(), ColumnType::BOOLEAN, false);
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            "ALTER TABLE users ALTER COLUMN name TYPE BOOLEAN USING name::BOOLEAN"
        );
    }

    #[test]
    fn test_alter_column_type_string_to_json_with_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // TEXT → JSONB（USING句が必要）
        let old_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let new_column = Column::new("name".to_string(), ColumnType::JSONB, false);
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            "ALTER TABLE users ALTER COLUMN name TYPE JSONB USING name::JSONB"
        );
    }

    #[test]
    fn test_alter_column_type_down_direction() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // Down方向: old_columnの型に戻す
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
        // Down方向なので old_type (INTEGER) に戻す
        assert_eq!(sql[0], "ALTER TABLE users ALTER COLUMN id TYPE INTEGER");
    }

    #[test]
    fn test_alter_column_type_datetime_to_string_no_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // TIMESTAMP → TEXT（暗黙変換可能）
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::TIMESTAMP {
                with_time_zone: None,
            },
            false,
        );
        let new_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(sql[0], "ALTER TABLE users ALTER COLUMN name TYPE TEXT");
    }

    #[test]
    fn test_alter_column_type_string_to_datetime_with_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // TEXT → TIMESTAMP（USING句が必要）
        let old_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let new_column = Column::new(
            "name".to_string(),
            ColumnType::TIMESTAMP {
                with_time_zone: None,
            },
            false,
        );
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            "ALTER TABLE users ALTER COLUMN name TYPE TIMESTAMP USING name::TIMESTAMP"
        );
    }

    // ==========================================
    // generate_rename_column のテスト
    // ==========================================

    use crate::core::schema_diff::RenamedColumn;

    #[test]
    fn test_generate_rename_column_up() {
        // Up方向：old_name → new_name
        let generator = PostgresSqlGenerator::new();
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
        assert_eq!(sql[0], "ALTER TABLE users RENAME COLUMN name TO user_name");
    }

    #[test]
    fn test_generate_rename_column_down() {
        // Down方向：new_name → old_name（逆リネーム）
        let generator = PostgresSqlGenerator::new();
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
        assert_eq!(sql[0], "ALTER TABLE users RENAME COLUMN user_name TO name");
    }

    // ==========================================
    // SERIAL変換のテスト
    // ==========================================

    #[test]
    fn test_alter_column_integer_to_serial() {
        // INTEGER → SERIAL: 型は同じだがauto_incrementが変わる
        // 型変更SQLは生成されず、シーケンス関連のSQLのみ生成される
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(false);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        new_column.auto_increment = Some(true);

        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        // 型変更SQLは含まれない（INTEGERのまま）
        // シーケンス関連のSQLが4つ生成される
        assert_eq!(sql.len(), 4);
        assert!(sql[0].contains("CREATE SEQUENCE"));
        assert!(sql[1].contains("setval"));
        assert!(sql[2].contains("SET DEFAULT nextval"));
        assert!(sql[3].contains("OWNED BY"));

        // 型変更SQLがないことを確認
        assert!(!sql.iter().any(|s| s.contains("ALTER COLUMN id TYPE")));
    }

    #[test]
    fn test_alter_column_integer_to_bigserial() {
        // INTEGER → BIGSERIAL: 型もauto_incrementも変わる
        // 型変更SQLが1回だけ生成され、その後にシーケンス関連SQLが続く
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(false);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) }, // BIGINT
            false,
        );
        new_column.auto_increment = Some(true);

        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        // 型変更SQL(1) + シーケンス関連SQL(4) = 5
        assert_eq!(sql.len(), 5);

        // 最初は型変更SQL（BIGINT、SERIALではない）
        assert_eq!(sql[0], "ALTER TABLE users ALTER COLUMN id TYPE BIGINT");

        // シーケンス関連SQL
        assert!(sql[1].contains("CREATE SEQUENCE"));
        assert!(sql[2].contains("setval"));
        assert!(sql[3].contains("SET DEFAULT nextval"));
        assert!(sql[4].contains("OWNED BY"));

        // 型変更SQLは1回だけであることを確認
        let type_change_count = sql
            .iter()
            .filter(|s| s.contains("ALTER COLUMN id TYPE"))
            .count();
        assert_eq!(type_change_count, 1);
    }

    #[test]
    fn test_alter_column_serial_to_integer() {
        // SERIAL → INTEGER: auto_incrementがtrueからfalseに
        // DEFAULTドロップとシーケンス削除が生成される
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(true);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        new_column.auto_increment = Some(false);

        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        // DEFAULTドロップ + シーケンス削除 = 2
        assert_eq!(sql.len(), 2);
        assert!(sql[0].contains("DROP DEFAULT"));
        assert!(sql[1].contains("DROP SEQUENCE IF EXISTS"));
        assert!(sql[1].contains("CASCADE"));
    }

    #[test]
    fn test_alter_column_bigserial_to_integer() {
        // BIGSERIAL → INTEGER: 型もauto_incrementも変わる
        // 型変更SQL + DEFAULTドロップ + シーケンス削除
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) }, // BIGINT
            false,
        );
        old_column.auto_increment = Some(true);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        new_column.auto_increment = Some(false);

        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        // 型変更SQL(1) + DEFAULTドロップ(1) + シーケンス削除(1) = 3
        assert_eq!(sql.len(), 3);
        assert_eq!(sql[0], "ALTER TABLE users ALTER COLUMN id TYPE INTEGER");
        assert!(sql[1].contains("DROP DEFAULT"));
        assert!(sql[2].contains("DROP SEQUENCE IF EXISTS"));
    }

    // ==========================================
    // 制約メソッドのテスト
    // ==========================================

    #[test]
    fn test_generate_add_constraint_for_existing_table_foreign_key() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
        };

        let sql = generator.generate_add_constraint_for_existing_table("posts", &constraint);

        assert_eq!(
            sql,
            "ALTER TABLE posts ADD CONSTRAINT fk_posts_user_id_users FOREIGN KEY (user_id) REFERENCES users (id)"
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_composite_foreign_key() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["org_id".to_string(), "user_id".to_string()],
            referenced_table: "org_users".to_string(),
            referenced_columns: vec!["organization_id".to_string(), "user_id".to_string()],
        };

        let sql = generator.generate_add_constraint_for_existing_table("posts", &constraint);

        assert_eq!(
            sql,
            "ALTER TABLE posts ADD CONSTRAINT fk_posts_org_id_user_id_org_users FOREIGN KEY (org_id, user_id) REFERENCES org_users (organization_id, user_id)"
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_non_fk_returns_empty() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let sql = generator.generate_add_constraint_for_existing_table("users", &constraint);

        assert!(sql.is_empty());
    }

    #[test]
    fn test_generate_drop_constraint_for_existing_table_foreign_key() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
        };

        let sql = generator.generate_drop_constraint_for_existing_table("posts", &constraint);

        assert_eq!(
            sql,
            "ALTER TABLE posts DROP CONSTRAINT IF EXISTS fk_posts_user_id_users"
        );
    }

    #[test]
    fn test_generate_drop_constraint_for_existing_table_non_fk_returns_empty() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let sql = generator.generate_drop_constraint_for_existing_table("users", &constraint);

        assert!(sql.is_empty());
    }
}
