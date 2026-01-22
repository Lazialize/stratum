// スキーマドメインモデル
//
// データベーススキーマの定義を表現する型システム。
// Schema, Table, Column, Index, Constraint などの構造体を提供します。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// スキーマ定義
///
/// データベース全体のスキーマを表現します。
/// 複数のテーブル定義を保持します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    /// スキーマのバージョン
    pub version: String,

    /// テーブル定義のマップ（テーブル名 -> Table）
    pub tables: HashMap<String, Table>,
}

impl Schema {
    /// 新しいスキーマを作成
    pub fn new(version: String) -> Self {
        Self {
            version,
            tables: HashMap::new(),
        }
    }

    /// テーブルを追加
    pub fn add_table(&mut self, table: Table) {
        self.tables.insert(table.name.clone(), table);
    }

    /// 指定されたテーブルが存在するか確認
    pub fn has_table(&self, table_name: &str) -> bool {
        self.tables.contains_key(table_name)
    }

    /// 指定されたテーブルを取得
    pub fn get_table(&self, table_name: &str) -> Option<&Table> {
        self.tables.get(table_name)
    }

    /// テーブル数を取得
    pub fn table_count(&self) -> usize {
        self.tables.len()
    }
}

/// テーブル定義
///
/// 単一のテーブルの構造を表現します。
/// カラム、インデックス、制約の定義を保持します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    /// テーブル名
    pub name: String,

    /// カラム定義のリスト
    pub columns: Vec<Column>,

    /// インデックス定義のリスト
    pub indexes: Vec<Index>,

    /// 制約定義のリスト
    pub constraints: Vec<Constraint>,
}

impl Table {
    /// 新しいテーブルを作成
    pub fn new(name: String) -> Self {
        Self {
            name,
            columns: Vec::new(),
            indexes: Vec::new(),
            constraints: Vec::new(),
        }
    }

    /// カラムを追加
    pub fn add_column(&mut self, column: Column) {
        self.columns.push(column);
    }

    /// インデックスを追加
    pub fn add_index(&mut self, index: Index) {
        self.indexes.push(index);
    }

    /// 制約を追加
    pub fn add_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint);
    }

    /// プライマリキーのカラム名を取得
    pub fn get_primary_key_columns(&self) -> Option<Vec<String>> {
        for constraint in &self.constraints {
            if let Constraint::PRIMARY_KEY { columns } = constraint {
                return Some(columns.clone());
            }
        }
        None
    }

    /// 指定されたカラムを取得
    pub fn get_column(&self, column_name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == column_name)
    }
}

/// カラム定義
///
/// テーブル内の単一カラムの構造を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    /// カラム名
    pub name: String,

    /// カラム型
    #[serde(rename = "type")]
    pub column_type: ColumnType,

    /// NULL許可フラグ
    pub nullable: bool,

    /// デフォルト値
    pub default_value: Option<String>,

    /// 自動増分フラグ
    pub auto_increment: Option<bool>,
}

impl Column {
    /// 新しいカラムを作成
    pub fn new(name: String, column_type: ColumnType, nullable: bool) -> Self {
        Self {
            name,
            column_type,
            nullable,
            default_value: None,
            auto_increment: None,
        }
    }

    /// 自動増分カラムかどうか
    pub fn is_auto_increment(&self) -> bool {
        self.auto_increment.unwrap_or(false)
    }
}

/// カラム型
///
/// サポートされるデータ型を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ColumnType {
    /// 整数型
    INTEGER {
        /// 精度（ビット数）
        precision: Option<u32>,
    },

    /// 可変長文字列型
    VARCHAR {
        /// 最大長
        length: u32,
    },

    /// テキスト型（長文）
    TEXT,

    /// 真偽値型
    BOOLEAN,

    /// タイムスタンプ型
    TIMESTAMP {
        /// タイムゾーン付きかどうか
        with_time_zone: Option<bool>,
    },

    /// JSON型
    JSON,

    /// 固定小数点数型
    DECIMAL {
        /// 全体の桁数 (1-65 for MySQL, 1-1000 for PostgreSQL)
        precision: u32,
        /// 小数点以下の桁数 (0 <= scale <= precision)
        scale: u32,
    },

    /// 単精度浮動小数点型
    FLOAT,

    /// 倍精度浮動小数点型
    DOUBLE,

    /// 固定長文字列型
    CHAR {
        /// 固定長 (1-255)
        length: u32,
    },

    /// 日付型
    DATE,

    /// 時刻型
    TIME {
        /// タイムゾーン付きかどうか (PostgreSQL only)
        with_time_zone: Option<bool>,
    },

    /// バイナリラージオブジェクト型
    BLOB,

    /// UUID型
    UUID,

    /// バイナリJSON型 (PostgreSQL専用)
    JSONB,
}

impl ColumnType {
    /// SQLの型名を取得（PostgreSQL方言）
    pub fn to_sql_type(&self, dialect: &crate::core::config::Dialect) -> String {
        use crate::core::config::Dialect;

        match (self, dialect) {
            (ColumnType::INTEGER { precision }, Dialect::PostgreSQL) => match precision {
                Some(2) => "SMALLINT".to_string(),
                Some(4) => "INTEGER".to_string(),
                Some(8) => "BIGINT".to_string(),
                _ => "INTEGER".to_string(),
            },
            (ColumnType::INTEGER { .. }, Dialect::MySQL) => "INT".to_string(),
            (ColumnType::INTEGER { .. }, Dialect::SQLite) => "INTEGER".to_string(),

            (ColumnType::VARCHAR { length }, _) => format!("VARCHAR({})", length),

            (ColumnType::TEXT, _) => "TEXT".to_string(),

            (ColumnType::BOOLEAN, Dialect::PostgreSQL) => "BOOLEAN".to_string(),
            (ColumnType::BOOLEAN, Dialect::MySQL) => "TINYINT(1)".to_string(),
            (ColumnType::BOOLEAN, Dialect::SQLite) => "INTEGER".to_string(),

            (ColumnType::TIMESTAMP { with_time_zone }, Dialect::PostgreSQL) => {
                if with_time_zone.unwrap_or(false) {
                    "TIMESTAMP WITH TIME ZONE".to_string()
                } else {
                    "TIMESTAMP".to_string()
                }
            }
            (ColumnType::TIMESTAMP { .. }, Dialect::MySQL) => "TIMESTAMP".to_string(),
            (ColumnType::TIMESTAMP { .. }, Dialect::SQLite) => "TEXT".to_string(),

            (ColumnType::JSON, Dialect::PostgreSQL) => "JSON".to_string(),
            (ColumnType::JSON, Dialect::MySQL) => "JSON".to_string(),
            (ColumnType::JSON, Dialect::SQLite) => "TEXT".to_string(),

            // DECIMAL
            (ColumnType::DECIMAL { precision, scale }, Dialect::PostgreSQL) => {
                format!("NUMERIC({}, {})", precision, scale)
            }
            (ColumnType::DECIMAL { precision, scale }, Dialect::MySQL) => {
                format!("DECIMAL({}, {})", precision, scale)
            }
            (ColumnType::DECIMAL { .. }, Dialect::SQLite) => "TEXT".to_string(),

            // FLOAT
            (ColumnType::FLOAT, Dialect::PostgreSQL) => "REAL".to_string(),
            (ColumnType::FLOAT, Dialect::MySQL) => "FLOAT".to_string(),
            (ColumnType::FLOAT, Dialect::SQLite) => "REAL".to_string(),

            // DOUBLE
            (ColumnType::DOUBLE, Dialect::PostgreSQL) => "DOUBLE PRECISION".to_string(),
            (ColumnType::DOUBLE, Dialect::MySQL) => "DOUBLE".to_string(),
            (ColumnType::DOUBLE, Dialect::SQLite) => "REAL".to_string(),

            // CHAR
            (ColumnType::CHAR { length }, Dialect::PostgreSQL) => {
                format!("CHAR({})", length)
            }
            (ColumnType::CHAR { length }, Dialect::MySQL) => {
                format!("CHAR({})", length)
            }
            (ColumnType::CHAR { .. }, Dialect::SQLite) => "TEXT".to_string(),

            // DATE
            (ColumnType::DATE, Dialect::PostgreSQL) => "DATE".to_string(),
            (ColumnType::DATE, Dialect::MySQL) => "DATE".to_string(),
            (ColumnType::DATE, Dialect::SQLite) => "TEXT".to_string(),

            // TIME
            (ColumnType::TIME { with_time_zone }, Dialect::PostgreSQL) => {
                if with_time_zone.unwrap_or(false) {
                    "TIME WITH TIME ZONE".to_string()
                } else {
                    "TIME".to_string()
                }
            }
            (ColumnType::TIME { .. }, Dialect::MySQL) => "TIME".to_string(),
            (ColumnType::TIME { .. }, Dialect::SQLite) => "TEXT".to_string(),

            // BLOB
            (ColumnType::BLOB, Dialect::PostgreSQL) => "BYTEA".to_string(),
            (ColumnType::BLOB, Dialect::MySQL) => "BLOB".to_string(),
            (ColumnType::BLOB, Dialect::SQLite) => "BLOB".to_string(),

            // UUID
            (ColumnType::UUID, Dialect::PostgreSQL) => "UUID".to_string(),
            (ColumnType::UUID, Dialect::MySQL) => "CHAR(36)".to_string(),
            (ColumnType::UUID, Dialect::SQLite) => "TEXT".to_string(),

            // JSONB
            (ColumnType::JSONB, Dialect::PostgreSQL) => "JSONB".to_string(),
            (ColumnType::JSONB, Dialect::MySQL) => "JSON".to_string(),
            (ColumnType::JSONB, Dialect::SQLite) => "TEXT".to_string(),
        }
    }
}

/// インデックス定義
///
/// テーブルのインデックスを表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Index {
    /// インデックス名
    pub name: String,

    /// インデックス対象のカラム名リスト
    pub columns: Vec<String>,

    /// ユニークインデックスかどうか
    pub unique: bool,
}

impl Index {
    /// 新しいインデックスを作成
    pub fn new(name: String, columns: Vec<String>, unique: bool) -> Self {
        Self {
            name,
            columns,
            unique,
        }
    }
}

/// 制約定義
///
/// テーブルの制約（PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK）を表現します。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum Constraint {
    /// プライマリキー制約
    PRIMARY_KEY {
        /// 対象カラム
        columns: Vec<String>,
    },

    /// 外部キー制約
    FOREIGN_KEY {
        /// 対象カラム
        columns: Vec<String>,

        /// 参照先テーブル
        referenced_table: String,

        /// 参照先カラム
        referenced_columns: Vec<String>,
    },

    /// ユニーク制約
    UNIQUE {
        /// 対象カラム
        columns: Vec<String>,
    },

    /// チェック制約
    CHECK {
        /// 対象カラム
        columns: Vec<String>,

        /// チェック式
        check_expression: String,
    },
}

impl Constraint {
    /// 制約の種類を文字列で取得
    pub fn kind(&self) -> &'static str {
        match self {
            Constraint::PRIMARY_KEY { .. } => "PRIMARY_KEY",
            Constraint::FOREIGN_KEY { .. } => "FOREIGN_KEY",
            Constraint::UNIQUE { .. } => "UNIQUE",
            Constraint::CHECK { .. } => "CHECK",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_new() {
        let schema = Schema::new("1.0".to_string());
        assert_eq!(schema.version, "1.0");
        assert_eq!(schema.table_count(), 0);
    }

    #[test]
    fn test_table_new() {
        let table = Table::new("users".to_string());
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 0);
    }

    #[test]
    fn test_column_new() {
        let column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        assert_eq!(column.name, "id");
        assert!(!column.nullable);
        assert!(!column.is_auto_increment());
    }

    #[test]
    fn test_index_new() {
        let index = Index::new("idx_email".to_string(), vec!["email".to_string()], true);
        assert_eq!(index.name, "idx_email");
        assert!(index.unique);
    }

    #[test]
    fn test_constraint_kind() {
        let pk = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };
        assert_eq!(pk.kind(), "PRIMARY_KEY");

        let fk = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
        };
        assert_eq!(fk.kind(), "FOREIGN_KEY");
    }

    // 新規データ型のテスト
    #[test]
    fn test_decimal_type() {
        use crate::core::config::Dialect;

        let col_type = ColumnType::DECIMAL {
            precision: 10,
            scale: 2,
        };
        assert_eq!(col_type.to_sql_type(&Dialect::PostgreSQL), "NUMERIC(10, 2)");
        assert_eq!(col_type.to_sql_type(&Dialect::MySQL), "DECIMAL(10, 2)");
        assert_eq!(col_type.to_sql_type(&Dialect::SQLite), "TEXT");
    }

    #[test]
    fn test_float_type() {
        use crate::core::config::Dialect;

        let col_type = ColumnType::FLOAT;
        assert_eq!(col_type.to_sql_type(&Dialect::PostgreSQL), "REAL");
        assert_eq!(col_type.to_sql_type(&Dialect::MySQL), "FLOAT");
        assert_eq!(col_type.to_sql_type(&Dialect::SQLite), "REAL");
    }

    #[test]
    fn test_double_type() {
        use crate::core::config::Dialect;

        let col_type = ColumnType::DOUBLE;
        assert_eq!(
            col_type.to_sql_type(&Dialect::PostgreSQL),
            "DOUBLE PRECISION"
        );
        assert_eq!(col_type.to_sql_type(&Dialect::MySQL), "DOUBLE");
        assert_eq!(col_type.to_sql_type(&Dialect::SQLite), "REAL");
    }

    #[test]
    fn test_char_type() {
        use crate::core::config::Dialect;

        let col_type = ColumnType::CHAR { length: 10 };
        assert_eq!(col_type.to_sql_type(&Dialect::PostgreSQL), "CHAR(10)");
        assert_eq!(col_type.to_sql_type(&Dialect::MySQL), "CHAR(10)");
        assert_eq!(col_type.to_sql_type(&Dialect::SQLite), "TEXT");
    }

    #[test]
    fn test_date_type() {
        use crate::core::config::Dialect;

        let col_type = ColumnType::DATE;
        assert_eq!(col_type.to_sql_type(&Dialect::PostgreSQL), "DATE");
        assert_eq!(col_type.to_sql_type(&Dialect::MySQL), "DATE");
        assert_eq!(col_type.to_sql_type(&Dialect::SQLite), "TEXT");
    }

    #[test]
    fn test_time_type() {
        use crate::core::config::Dialect;

        let col_type = ColumnType::TIME {
            with_time_zone: Some(false),
        };
        assert_eq!(col_type.to_sql_type(&Dialect::PostgreSQL), "TIME");
        assert_eq!(col_type.to_sql_type(&Dialect::MySQL), "TIME");
        assert_eq!(col_type.to_sql_type(&Dialect::SQLite), "TEXT");

        let col_type_tz = ColumnType::TIME {
            with_time_zone: Some(true),
        };
        assert_eq!(
            col_type_tz.to_sql_type(&Dialect::PostgreSQL),
            "TIME WITH TIME ZONE"
        );
    }

    #[test]
    fn test_blob_type() {
        use crate::core::config::Dialect;

        let col_type = ColumnType::BLOB;
        assert_eq!(col_type.to_sql_type(&Dialect::PostgreSQL), "BYTEA");
        assert_eq!(col_type.to_sql_type(&Dialect::MySQL), "BLOB");
        assert_eq!(col_type.to_sql_type(&Dialect::SQLite), "BLOB");
    }

    #[test]
    fn test_uuid_type() {
        use crate::core::config::Dialect;

        let col_type = ColumnType::UUID;
        assert_eq!(col_type.to_sql_type(&Dialect::PostgreSQL), "UUID");
        assert_eq!(col_type.to_sql_type(&Dialect::MySQL), "CHAR(36)");
        assert_eq!(col_type.to_sql_type(&Dialect::SQLite), "TEXT");
    }

    #[test]
    fn test_jsonb_type() {
        use crate::core::config::Dialect;

        let col_type = ColumnType::JSONB;
        assert_eq!(col_type.to_sql_type(&Dialect::PostgreSQL), "JSONB");
        assert_eq!(col_type.to_sql_type(&Dialect::MySQL), "JSON");
        assert_eq!(col_type.to_sql_type(&Dialect::SQLite), "TEXT");
    }
}
