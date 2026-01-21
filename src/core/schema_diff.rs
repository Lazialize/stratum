// スキーマ差分ドメインモデル
//
// スキーマ間の差分を表現する型システム。
// テーブル、カラム、インデックス、制約の追加、削除、変更を表現します。

use serde::{Deserialize, Serialize};

use crate::core::schema::{Column, Constraint, Index, Table};

/// スキーマ差分
///
/// 2つのスキーマ間の差分を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaDiff {
    /// 追加されたテーブル
    pub added_tables: Vec<Table>,

    /// 削除されたテーブル
    pub removed_tables: Vec<String>,

    /// 変更されたテーブル
    pub modified_tables: Vec<TableDiff>,
}

impl SchemaDiff {
    /// 新しいスキーマ差分を作成
    pub fn new() -> Self {
        Self {
            added_tables: Vec::new(),
            removed_tables: Vec::new(),
            modified_tables: Vec::new(),
        }
    }

    /// 差分が空かどうか
    pub fn is_empty(&self) -> bool {
        self.added_tables.is_empty()
            && self.removed_tables.is_empty()
            && self.modified_tables.is_empty()
    }

    /// 差分の項目数を取得
    pub fn count(&self) -> usize {
        self.added_tables.len() + self.removed_tables.len() + self.modified_tables.len()
    }
}

impl Default for SchemaDiff {
    fn default() -> Self {
        Self::new()
    }
}

/// テーブル差分
///
/// テーブルの変更内容を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableDiff {
    /// テーブル名
    pub table_name: String,

    /// 追加されたカラム
    pub added_columns: Vec<Column>,

    /// 削除されたカラム
    pub removed_columns: Vec<String>,

    /// 変更されたカラム
    pub modified_columns: Vec<ColumnDiff>,

    /// 追加されたインデックス
    pub added_indexes: Vec<Index>,

    /// 削除されたインデックス
    pub removed_indexes: Vec<String>,

    /// 追加された制約
    pub added_constraints: Vec<Constraint>,

    /// 削除された制約
    pub removed_constraints: Vec<Constraint>,
}

impl TableDiff {
    /// 新しいテーブル差分を作成
    pub fn new(table_name: String) -> Self {
        Self {
            table_name,
            added_columns: Vec::new(),
            removed_columns: Vec::new(),
            modified_columns: Vec::new(),
            added_indexes: Vec::new(),
            removed_indexes: Vec::new(),
            added_constraints: Vec::new(),
            removed_constraints: Vec::new(),
        }
    }

    /// 差分が空かどうか
    pub fn is_empty(&self) -> bool {
        self.added_columns.is_empty()
            && self.removed_columns.is_empty()
            && self.modified_columns.is_empty()
            && self.added_indexes.is_empty()
            && self.removed_indexes.is_empty()
            && self.added_constraints.is_empty()
            && self.removed_constraints.is_empty()
    }
}

/// カラム差分
///
/// カラムの変更内容を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnDiff {
    /// カラム名
    pub column_name: String,

    /// 変更前のカラム定義
    pub old_column: Column,

    /// 変更後のカラム定義
    pub new_column: Column,

    /// 変更された属性
    pub changes: Vec<ColumnChange>,
}

impl ColumnDiff {
    /// 新しいカラム差分を作成
    pub fn new(column_name: String, old_column: Column, new_column: Column) -> Self {
        let mut changes = Vec::new();

        // 型の変更を検出
        if old_column.column_type != new_column.column_type {
            changes.push(ColumnChange::TypeChanged {
                old_type: format!("{:?}", old_column.column_type),
                new_type: format!("{:?}", new_column.column_type),
            });
        }

        // NULL制約の変更を検出
        if old_column.nullable != new_column.nullable {
            changes.push(ColumnChange::NullableChanged {
                old_nullable: old_column.nullable,
                new_nullable: new_column.nullable,
            });
        }

        // デフォルト値の変更を検出
        if old_column.default_value != new_column.default_value {
            changes.push(ColumnChange::DefaultValueChanged {
                old_default: old_column.default_value.clone(),
                new_default: new_column.default_value.clone(),
            });
        }

        // AUTO_INCREMENTの変更を検出
        if old_column.auto_increment != new_column.auto_increment {
            changes.push(ColumnChange::AutoIncrementChanged {
                old_auto_increment: old_column.auto_increment,
                new_auto_increment: new_column.auto_increment,
            });
        }

        Self {
            column_name,
            old_column,
            new_column,
            changes,
        }
    }
}

/// カラム変更
///
/// カラムの変更内容の種類を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColumnChange {
    /// 型の変更
    TypeChanged { old_type: String, new_type: String },

    /// NULL制約の変更
    NullableChanged {
        old_nullable: bool,
        new_nullable: bool,
    },

    /// デフォルト値の変更
    DefaultValueChanged {
        old_default: Option<String>,
        new_default: Option<String>,
    },

    /// AUTO_INCREMENTの変更
    AutoIncrementChanged {
        old_auto_increment: Option<bool>,
        new_auto_increment: Option<bool>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::ColumnType;

    #[test]
    fn test_schema_diff_new() {
        let diff = SchemaDiff::new();
        assert!(diff.is_empty());
        assert_eq!(diff.count(), 0);
    }

    #[test]
    fn test_schema_diff_not_empty() {
        let mut diff = SchemaDiff::new();
        diff.added_tables.push(Table::new("users".to_string()));

        assert!(!diff.is_empty());
        assert_eq!(diff.count(), 1);
    }

    #[test]
    fn test_table_diff_new() {
        let diff = TableDiff::new("users".to_string());
        assert_eq!(diff.table_name, "users");
        assert!(diff.is_empty());
    }

    #[test]
    fn test_column_diff_type_change() {
        let old_column = Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let new_column = Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: Some(8) },
            false,
        );

        let diff = ColumnDiff::new("age".to_string(), old_column, new_column);

        assert_eq!(diff.changes.len(), 1);
        assert!(matches!(
            diff.changes[0],
            ColumnChange::TypeChanged { .. }
        ));
    }

    #[test]
    fn test_column_diff_nullable_change() {
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            true,
        );

        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        assert_eq!(diff.changes.len(), 1);
        assert!(matches!(
            diff.changes[0],
            ColumnChange::NullableChanged { .. }
        ));
    }

    #[test]
    fn test_column_diff_no_change() {
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );

        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        assert_eq!(diff.changes.len(), 0);
    }
}
