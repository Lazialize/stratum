// PostgreSQL用SQLジェネレーター
//
// スキーマ定義からPostgreSQL用のDDL文を生成します。

use crate::adapters::sql_generator::{MigrationDirection, SqlGenerator};
use crate::core::schema::{Column, ColumnType, Constraint, Index, Table};
use crate::core::schema_diff::ColumnDiff;
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
        let mut parts = Vec::new();

        // カラム名
        parts.push(column.name.clone());

        // データ型
        let type_str = self.map_column_type(&column.column_type, column.auto_increment);
        parts.push(type_str);

        // NULL制約
        if !column.nullable {
            parts.push("NOT NULL".to_string());
        }

        // デフォルト値
        if let Some(ref default_value) = column.default_value {
            parts.push(format!("DEFAULT {}", default_value));
        }

        parts.join(" ")
    }

    /// ColumnTypeをPostgreSQLの型文字列にマッピング
    fn map_column_type(&self, column_type: &ColumnType, auto_increment: Option<bool>) -> String {
        match column_type {
            ColumnType::INTEGER { precision } => {
                if auto_increment.unwrap_or(false) {
                    match precision {
                        Some(8) => "BIGSERIAL".to_string(),
                        Some(2) => "SMALLSERIAL".to_string(),
                        _ => "SERIAL".to_string(),
                    }
                } else {
                    match precision {
                        Some(2) => "SMALLINT".to_string(),
                        Some(8) => "BIGINT".to_string(),
                        _ => "INTEGER".to_string(),
                    }
                }
            }
            ColumnType::VARCHAR { length } => format!("VARCHAR({})", length),
            ColumnType::TEXT => "TEXT".to_string(),
            ColumnType::BOOLEAN => "BOOLEAN".to_string(),
            ColumnType::TIMESTAMP { with_time_zone } => {
                if with_time_zone.unwrap_or(false) {
                    "TIMESTAMP WITH TIME ZONE".to_string()
                } else {
                    "TIMESTAMP".to_string()
                }
            }
            ColumnType::JSON => "JSON".to_string(),
            ColumnType::DECIMAL { precision, scale } => {
                format!("NUMERIC({}, {})", precision, scale)
            }
            ColumnType::FLOAT => "REAL".to_string(),
            ColumnType::DOUBLE => "DOUBLE PRECISION".to_string(),
            ColumnType::CHAR { length } => format!("CHAR({})", length),
            ColumnType::DATE => "DATE".to_string(),
            ColumnType::TIME { with_time_zone } => {
                if with_time_zone.unwrap_or(false) {
                    "TIME WITH TIME ZONE".to_string()
                } else {
                    "TIME".to_string()
                }
            }
            ColumnType::BLOB => "BYTEA".to_string(),
            ColumnType::UUID => "UUID".to_string(),
            ColumnType::JSONB => "JSONB".to_string(),
            ColumnType::Enum { name } => name.clone(),
            // 方言固有型はformat_dialect_specific_typeでフォーマット
            ColumnType::DialectSpecific { kind, params } => {
                self.format_dialect_specific_type(kind, params)
            }
        }
    }

    /// 方言固有型のフォーマット（PostgreSQL）
    ///
    /// パラメータに応じて適切なSQL型文字列を生成します。
    fn format_dialect_specific_type(&self, kind: &str, params: &serde_json::Value) -> String {
        // lengthパラメータがある場合（例: VARBIT(16)）
        if let Some(length) = params.get("length").and_then(|v| v.as_u64()) {
            return format!("{}({})", kind, length);
        }

        // arrayパラメータがtrueの場合（例: TEXT[]）
        if params
            .get("array")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return format!("{}[]", kind);
        }

        // valuesパラメータがある場合（例: ENUM('a', 'b', 'c')）
        if let Some(values) = params.get("values").and_then(|v| v.as_array()) {
            let values_str = values
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("'{}'", s))
                .collect::<Vec<_>>()
                .join(", ");
            return format!("{}({})", kind, values_str);
        }

        // パラメータなし、またはnullの場合はkindをそのまま出力
        kind.to_string()
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
    fn generate_alter_column_type(
        &self,
        table: &Table,
        column_diff: &ColumnDiff,
        direction: MigrationDirection,
    ) -> Vec<String> {
        let column_name = &column_diff.column_name;

        // 方向に応じて対象の型を決定
        let (source_type, target_type) = match direction {
            MigrationDirection::Up => (
                &column_diff.old_column.column_type,
                &column_diff.new_column.column_type,
            ),
            MigrationDirection::Down => (
                &column_diff.new_column.column_type,
                &column_diff.old_column.column_type,
            ),
        };

        // 対象の型をPostgreSQL型文字列にマッピング
        let target_type_str = self.map_column_type(target_type, None);

        // USING句が必要かどうかを判定
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

        vec![sql]
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
                    let constraint_name = format!(
                        "fk_{}_{}_{}",
                        table.name,
                        columns.join("_"),
                        referenced_table
                    );

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
}
