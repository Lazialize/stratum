// MySQL用SQLジェネレーター
//
// スキーマ定義からMySQL用のDDL文を生成します。

use crate::adapters::sql_generator::SqlGenerator;
use crate::core::schema::{Column, ColumnType, Constraint, Index, Table};

/// MySQL用SQLジェネレーター
#[derive(Debug, Clone)]
pub struct MysqlSqlGenerator {}

impl MysqlSqlGenerator {
    /// 新しいMysqlSqlGeneratorを作成
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

        // AUTO_INCREMENT（MySQLではデータ型の後に指定）
        if column.auto_increment.unwrap_or(false) {
            parts.push("AUTO_INCREMENT".to_string());
        }

        // デフォルト値
        if let Some(ref default_value) = column.default_value {
            parts.push(format!("DEFAULT {}", default_value));
        }

        parts.join(" ")
    }

    /// ColumnTypeをMySQLの型文字列にマッピング
    fn map_column_type(&self, column_type: &ColumnType, _auto_increment: Option<bool>) -> String {
        match column_type {
            ColumnType::INTEGER { precision } => match precision {
                Some(2) => "SMALLINT".to_string(),
                Some(8) => "BIGINT".to_string(),
                _ => "INT".to_string(),
            },
            ColumnType::VARCHAR { length } => format!("VARCHAR({})", length),
            ColumnType::TEXT => "TEXT".to_string(),
            ColumnType::BOOLEAN => "BOOLEAN".to_string(), // MySQLではTINYINT(1)のエイリアス
            ColumnType::TIMESTAMP { .. } => {
                // MySQLのTIMESTAMPはタイムゾーンを持たない
                "TIMESTAMP".to_string()
            }
            ColumnType::JSON => "JSON".to_string(),
            ColumnType::DECIMAL { precision, scale } => {
                format!("DECIMAL({}, {})", precision, scale)
            }
            ColumnType::FLOAT => "FLOAT".to_string(),
            ColumnType::DOUBLE => "DOUBLE".to_string(),
            ColumnType::CHAR { length } => format!("CHAR({})", length),
            ColumnType::DATE => "DATE".to_string(),
            ColumnType::TIME { .. } => "TIME".to_string(),
            ColumnType::BLOB => "BLOB".to_string(),
            ColumnType::UUID => "CHAR(36)".to_string(),
            ColumnType::JSONB => "JSON".to_string(), // JSONへフォールバック
            // 方言固有型はformat_dialect_specific_typeでフォーマット
            ColumnType::DialectSpecific { kind, params } => {
                self.format_dialect_specific_type(kind, params)
            }
        }
    }

    /// 方言固有型のフォーマット（MySQL）
    ///
    /// パラメータに応じて適切なSQL型文字列を生成します。
    fn format_dialect_specific_type(&self, kind: &str, params: &serde_json::Value) -> String {
        // valuesパラメータがある場合（例: ENUM('a', 'b', 'c') または SET('a', 'b', 'c')）
        if let Some(values) = params.get("values").and_then(|v| v.as_array()) {
            let values_str = values
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("'{}'", s))
                .collect::<Vec<_>>()
                .join(", ");
            return format!("{}({})", kind, values_str);
        }

        // lengthパラメータがある場合（例: VARCHAR(255)）
        if let Some(length) = params.get("length").and_then(|v| v.as_u64()) {
            return format!("{}({})", kind, length);
        }

        // unsignedパラメータがtrueの場合（例: TINYINT UNSIGNED）
        if params
            .get("unsigned")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return format!("{} UNSIGNED", kind);
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
                // MySQL 8.0.16以降でCHECK制約がサポートされる
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
}

impl SqlGenerator for MysqlSqlGenerator {
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
        assert_eq!(def, "name VARCHAR(100) NOT NULL");
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
        assert_eq!(def, "bio TEXT");
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
        assert_eq!(def, "status VARCHAR(20) NOT NULL DEFAULT 'active'");
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
        assert_eq!(def, "id INT NOT NULL AUTO_INCREMENT");
    }

    #[test]
    fn test_generate_constraint_primary_key() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "PRIMARY KEY (id)");
    }

    #[test]
    fn test_generate_constraint_unique() {
        let generator = MysqlSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "UNIQUE (email)");
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
}
