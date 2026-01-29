// スキーマドメインモデル
//
// データベーススキーマの定義を表現する型システム。
// Schema, Table, Column, Index, Constraint などの構造体を提供します。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// スキーマ定義
///
/// データベース全体のスキーマを表現します。
/// 複数のテーブル定義を保持します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    /// スキーマのバージョン
    pub version: String,

    /// ENUM再作成の許可フラグ（デフォルト: false）
    #[serde(default, skip_serializing_if = "is_false")]
    pub enum_recreate_allowed: bool,

    /// ENUM定義のマップ（型名 -> EnumDefinition）
    #[serde(default)]
    pub enums: BTreeMap<String, EnumDefinition>,

    /// テーブル定義のマップ（テーブル名 -> Table）
    pub tables: BTreeMap<String, Table>,
}

impl Schema {
    /// 新しいスキーマを作成
    pub fn new(version: String) -> Self {
        Self {
            version,
            enum_recreate_allowed: false,
            enums: BTreeMap::new(),
            tables: BTreeMap::new(),
        }
    }

    /// ENUM定義を追加
    pub fn add_enum(&mut self, enum_def: EnumDefinition) {
        let enum_name = enum_def.name.clone();
        self.enums.insert(enum_name, enum_def);
    }

    /// 指定されたENUM定義が存在するか確認
    pub fn has_enum(&self, enum_name: &str) -> bool {
        self.enums.contains_key(enum_name)
    }

    /// 指定されたENUM定義を取得
    pub fn get_enum(&self, enum_name: &str) -> Option<&EnumDefinition> {
        self.enums.get(enum_name)
    }

    /// ENUM定義数を取得
    pub fn enum_count(&self) -> usize {
        self.enums.len()
    }

    /// テーブルを追加
    pub fn add_table(&mut self, table: Table) {
        let table_name = table.name.clone();
        self.tables.insert(table_name, table);
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

    /// リネーム元のテーブル名（オプショナル）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
}

impl Table {
    /// 新しいテーブルを作成
    pub fn new(name: String) -> Self {
        Self {
            name,
            columns: Vec::new(),
            indexes: Vec::new(),
            constraints: Vec::new(),
            renamed_from: None,
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

    /// リネーム元のカラム名（オプショナル）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
}

/// ENUM定義
///
/// PostgreSQLのENUM型を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumDefinition {
    /// ENUM型名
    pub name: String,

    /// ENUM値（順序を保持）
    pub values: Vec<String>,
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
            renamed_from: None,
        }
    }

    /// 自動増分カラムかどうか
    pub fn is_auto_increment(&self) -> bool {
        self.auto_increment.unwrap_or(false)
    }
}

fn is_false(value: &bool) -> bool {
    !*value
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

    /// ENUM参照型（PostgreSQL専用）
    #[serde(rename = "ENUM")]
    Enum {
        /// 参照するENUM型名
        name: String,
    },

    /// 方言固有型
    ///
    /// データベース方言固有の型を直接指定する際に使用します。
    /// Strata内部では検証せず、SQL生成時にそのまま出力します。
    /// 型の妥当性はデータベース実行時に検証されます。
    #[serde(untagged)]
    DialectSpecific {
        /// 型名（例: "SERIAL", "ENUM", "TINYINT"）
        kind: String,
        /// 型パラメータ（任意、例: ENUM の values、VARBIT の length）
        #[serde(flatten)]
        params: serde_json::Value,
    },
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

/// 参照アクション
///
/// FOREIGN KEY制約のON DELETE / ON UPDATE句で使用するアクションを表現します。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReferentialAction {
    /// 何もしない（デフォルト）
    #[default]
    NoAction,
    /// 参照先の変更に追従して削除/更新
    Cascade,
    /// 参照先の削除/更新時にNULLに設定
    SetNull,
    /// 参照先の削除/更新時にデフォルト値に設定
    SetDefault,
    /// 参照先の削除/更新を制限
    Restrict,
}

impl ReferentialAction {
    /// SQL句として出力する文字列を返す
    pub fn as_sql(&self) -> &'static str {
        match self {
            ReferentialAction::NoAction => "NO ACTION",
            ReferentialAction::Cascade => "CASCADE",
            ReferentialAction::SetNull => "SET NULL",
            ReferentialAction::SetDefault => "SET DEFAULT",
            ReferentialAction::Restrict => "RESTRICT",
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

        /// 参照先レコード削除時のアクション
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_delete: Option<ReferentialAction>,

        /// 参照先レコード更新時のアクション
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_update: Option<ReferentialAction>,
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
        assert!(!schema.enum_recreate_allowed);
        assert_eq!(schema.enum_count(), 0);
        assert_eq!(schema.table_count(), 0);
    }

    #[test]
    fn test_schema_add_enum() {
        let mut schema = Schema::new("1.0".to_string());
        let enum_def = EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        };

        schema.add_enum(enum_def);

        assert!(schema.has_enum("status"));
        let stored = schema.get_enum("status").unwrap();
        assert_eq!(stored.values.len(), 2);
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
            on_delete: None,
            on_update: None,
        };
        assert_eq!(fk.kind(), "FOREIGN_KEY");
    }

    #[test]
    fn test_referential_action_as_sql() {
        assert_eq!(ReferentialAction::NoAction.as_sql(), "NO ACTION");
        assert_eq!(ReferentialAction::Cascade.as_sql(), "CASCADE");
        assert_eq!(ReferentialAction::SetNull.as_sql(), "SET NULL");
        assert_eq!(ReferentialAction::SetDefault.as_sql(), "SET DEFAULT");
        assert_eq!(ReferentialAction::Restrict.as_sql(), "RESTRICT");
    }

    #[test]
    fn test_referential_action_default() {
        let action: ReferentialAction = Default::default();
        assert_eq!(action, ReferentialAction::NoAction);
    }

    #[test]
    fn test_foreign_key_with_referential_actions() {
        let fk = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: Some(ReferentialAction::Cascade),
            on_update: Some(ReferentialAction::SetNull),
        };

        if let Constraint::FOREIGN_KEY {
            on_delete,
            on_update,
            ..
        } = fk
        {
            assert_eq!(on_delete, Some(ReferentialAction::Cascade));
            assert_eq!(on_update, Some(ReferentialAction::SetNull));
        } else {
            panic!("Expected FOREIGN_KEY constraint");
        }
    }

    #[test]
    fn test_foreign_key_serialization_with_actions() {
        let fk = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: Some(ReferentialAction::Cascade),
            on_update: Some(ReferentialAction::Restrict),
        };

        let json = serde_json::to_string(&fk).unwrap();
        assert!(json.contains("on_delete"));
        assert!(json.contains("CASCADE"));
        assert!(json.contains("on_update"));
        assert!(json.contains("RESTRICT"));
    }

    #[test]
    fn test_foreign_key_serialization_without_actions() {
        let fk = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        };

        let json = serde_json::to_string(&fk).unwrap();
        // skip_serializing_if によりNoneは出力されない
        assert!(!json.contains("on_delete"));
        assert!(!json.contains("on_update"));
    }

    #[test]
    fn test_foreign_key_deserialization_with_actions() {
        let json = r#"{
            "type": "FOREIGN_KEY",
            "columns": ["user_id"],
            "referenced_table": "users",
            "referenced_columns": ["id"],
            "on_delete": "CASCADE",
            "on_update": "SET_NULL"
        }"#;

        let fk: Constraint = serde_json::from_str(json).unwrap();
        if let Constraint::FOREIGN_KEY {
            on_delete,
            on_update,
            ..
        } = fk
        {
            assert_eq!(on_delete, Some(ReferentialAction::Cascade));
            assert_eq!(on_update, Some(ReferentialAction::SetNull));
        } else {
            panic!("Expected FOREIGN_KEY constraint");
        }
    }

    #[test]
    fn test_foreign_key_deserialization_without_actions() {
        let json = r#"{
            "type": "FOREIGN_KEY",
            "columns": ["user_id"],
            "referenced_table": "users",
            "referenced_columns": ["id"]
        }"#;

        let fk: Constraint = serde_json::from_str(json).unwrap();
        if let Constraint::FOREIGN_KEY {
            on_delete,
            on_update,
            ..
        } = fk
        {
            assert!(on_delete.is_none());
            assert!(on_update.is_none());
        } else {
            panic!("Expected FOREIGN_KEY constraint");
        }
    }

    #[test]
    fn test_column_renamed_from_field() {
        let mut column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        // デフォルトではrenamed_fromはNone
        assert!(column.renamed_from.is_none());

        // renamed_fromを設定
        column.renamed_from = Some("name".to_string());
        assert_eq!(column.renamed_from, Some("name".to_string()));
    }

    #[test]
    fn test_column_renamed_from_serialization() {
        // renamed_fromがある場合のシリアライズ
        let mut column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        column.renamed_from = Some("name".to_string());

        let yaml = serde_json::to_string(&column).unwrap();
        assert!(yaml.contains("renamed_from"));
        assert!(yaml.contains("name"));
    }

    #[test]
    fn test_column_renamed_from_none_not_serialized() {
        // renamed_fromがNoneの場合はYAML出力から除外される
        let column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );

        let yaml = serde_json::to_string(&column).unwrap();
        assert!(!yaml.contains("renamed_from"));
    }

    #[test]
    fn test_column_renamed_from_deserialization() {
        // renamed_from付きのJSONをデシリアライズ
        let json = r#"{
            "name": "user_name",
            "type": {"kind": "VARCHAR", "length": 100},
            "nullable": false,
            "renamed_from": "name"
        }"#;

        let column: Column = serde_json::from_str(json).unwrap();
        assert_eq!(column.name, "user_name");
        assert_eq!(column.renamed_from, Some("name".to_string()));
    }

    #[test]
    fn test_column_without_renamed_from_deserialization() {
        // renamed_fromなしのJSONをデシリアライズ
        let json = r#"{
            "name": "user_name",
            "type": {"kind": "VARCHAR", "length": 100},
            "nullable": false
        }"#;

        let column: Column = serde_json::from_str(json).unwrap();
        assert_eq!(column.name, "user_name");
        assert!(column.renamed_from.is_none());
    }
}
