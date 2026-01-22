// SQLite用SQLジェネレーター
//
// スキーマ定義からSQLite用のDDL文を生成します。
// SQLiteはALTER TABLEの機能が制限されているため、制約はCREATE TABLE内で定義します。

use crate::adapters::sql_generator::SqlGenerator;
use crate::core::schema::{Column, ColumnType, Constraint, Index, Table};

/// SQLite用SQLジェネレーター
#[derive(Debug, Clone)]
pub struct SqliteSqlGenerator {}

impl SqliteSqlGenerator {
    /// 新しいSqliteSqlGeneratorを作成
    pub fn new() -> Self {
        Self {}
    }

    /// カラム定義のSQL文字列を生成
    fn generate_column_definition(&self, column: &Column) -> String {
        let mut parts = Vec::new();

        // カラム名
        parts.push(column.name.clone());

        // データ型
        let type_str = self.map_column_type(&column.column_type);
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

    /// ColumnTypeをSQLiteの型文字列にマッピング
    fn map_column_type(&self, column_type: &ColumnType) -> String {
        match column_type {
            ColumnType::INTEGER { .. } => {
                // SQLiteではすべての整数型をINTEGERとして扱う
                "INTEGER".to_string()
            }
            ColumnType::VARCHAR { .. } => {
                // SQLiteではVARCHARはTEXTとして扱われる
                "TEXT".to_string()
            }
            ColumnType::TEXT => "TEXT".to_string(),
            ColumnType::BOOLEAN => {
                // SQLiteにはBOOLEAN型がないため、INTEGER (0/1)で表現
                "INTEGER".to_string()
            }
            ColumnType::TIMESTAMP { .. } => {
                // SQLiteではタイムスタンプをTEXTまたはINTEGERで保存
                // ISO 8601形式のTEXTを使用
                "TEXT".to_string()
            }
            ColumnType::JSON => {
                // SQLiteではJSONをTEXTとして保存
                "TEXT".to_string()
            }
            ColumnType::DECIMAL { .. } => "TEXT".to_string(), // 精度保証のためTEXTを使用
            ColumnType::FLOAT | ColumnType::DOUBLE => "REAL".to_string(),
            ColumnType::CHAR { .. } => "TEXT".to_string(),
            ColumnType::DATE => "TEXT".to_string(), // ISO 8601形式
            ColumnType::TIME { .. } => "TEXT".to_string(), // ISO 8601形式
            ColumnType::BLOB => "BLOB".to_string(),
            ColumnType::UUID => "TEXT".to_string(),
            ColumnType::JSONB => "TEXT".to_string(), // TEXTへフォールバック
            // 方言固有型はformat_dialect_specific_typeでフォーマット
            ColumnType::DialectSpecific { kind, params } => {
                self.format_dialect_specific_type(kind, params)
            }
        }
    }

    /// 方言固有型のフォーマット（SQLite）
    ///
    /// SQLiteは型システムが柔軟なため、基本的にkindをそのまま出力します。
    fn format_dialect_specific_type(&self, kind: &str, _params: &serde_json::Value) -> String {
        // SQLiteは型アフィニティによる柔軟な型システムを持つため、
        // 方言固有型はそのまま出力（パラメータは無視）
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
            Constraint::FOREIGN_KEY {
                columns,
                referenced_table,
                referenced_columns,
            } => {
                // SQLiteではFOREIGN KEYをCREATE TABLE内で定義
                format!(
                    "FOREIGN KEY ({}) REFERENCES {} ({})",
                    columns.join(", "),
                    referenced_table,
                    referenced_columns.join(", ")
                )
            }
        }
    }
}

impl SqlGenerator for SqliteSqlGenerator {
    fn generate_create_table(&self, table: &Table) -> String {
        let mut parts = Vec::new();

        parts.push(format!("CREATE TABLE {}", table.name));
        parts.push("(".to_string());

        let mut elements = Vec::new();

        // カラム定義
        for column in &table.columns {
            elements.push(format!("    {}", self.generate_column_definition(column)));
        }

        // テーブル制約（すべての制約をCREATE TABLE内で定義）
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
        _table: &Table,
        _constraint_index: usize,
    ) -> String {
        // SQLiteはALTER TABLE ADD CONSTRAINTをサポートしていない
        // すべての制約はCREATE TABLE内で定義する必要がある
        String::new()
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
        assert_eq!(def, "name TEXT NOT NULL");
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
        assert_eq!(def, "bio TEXT");
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
        assert_eq!(def, "status TEXT NOT NULL DEFAULT 'active'");
    }

    #[test]
    fn test_generate_constraint_primary_key() {
        let generator = SqliteSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "PRIMARY KEY (id)");
    }

    #[test]
    fn test_generate_constraint_unique() {
        let generator = SqliteSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "UNIQUE (email)");
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
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "FOREIGN KEY (user_id) REFERENCES users (id)");
    }

    #[test]
    fn test_generate_alter_table_returns_empty() {
        let generator = SqliteSqlGenerator::new();
        let table = Table::new("test".to_string());

        // SQLiteはALTER TABLE ADD CONSTRAINTをサポートしていない
        let sql = generator.generate_alter_table_add_constraint(&table, 0);
        assert_eq!(sql, "");
    }
}
