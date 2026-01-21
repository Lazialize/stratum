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
}

impl ColumnType {
    /// SQLの型名を取得（PostgreSQL方言）
    pub fn to_sql_type(&self, dialect: &crate::core::config::Dialect) -> String {
        use crate::core::config::Dialect;

        match (self, dialect) {
            (ColumnType::INTEGER { precision }, Dialect::PostgreSQL) => {
                match precision {
                    Some(2) => "SMALLINT".to_string(),
                    Some(4) => "INTEGER".to_string(),
                    Some(8) => "BIGINT".to_string(),
                    _ => "INTEGER".to_string(),
                }
            }
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
        let index = Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            true,
        );
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
}
