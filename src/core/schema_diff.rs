// スキーマ差分ドメインモデル
//
// スキーマ間の差分を表現する型システム。
// テーブル、カラム、インデックス、制約の追加、削除、変更を表現します。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

    /// 外部キー制約による依存関係を考慮して、追加テーブルをトポロジカルソート
    ///
    /// 被参照テーブルが先に作成されるように並び替えます。
    /// 循環参照がある場合はエラーを返します。
    ///
    /// # Returns
    ///
    /// ソートされたテーブルのリスト、または循環参照エラー
    pub fn sort_added_tables_by_dependency(&self) -> Result<Vec<Table>, String> {
        if self.added_tables.is_empty() {
            return Ok(Vec::new());
        }

        // テーブル名 -> テーブルのマッピング
        let table_map: HashMap<&str, &Table> = self
            .added_tables
            .iter()
            .map(|t| (t.name.as_str(), t))
            .collect();

        // 追加されるテーブル名のセット
        let added_table_names: HashSet<&str> = table_map.keys().copied().collect();

        // 依存関係グラフを構築（テーブル名 -> このテーブルが依存している（参照している）テーブル名のリスト）
        let mut dependencies: HashMap<&str, Vec<&str>> = HashMap::new();

        for table in &self.added_tables {
            let mut deps = Vec::new();
            for constraint in &table.constraints {
                if let Constraint::FOREIGN_KEY {
                    referenced_table, ..
                } = constraint
                {
                    // 参照先が追加されるテーブルに含まれる場合のみ依存関係として登録
                    if added_table_names.contains(referenced_table.as_str()) {
                        deps.push(referenced_table.as_str());
                    }
                }
            }
            dependencies.insert(table.name.as_str(), deps);
        }

        // トポロジカルソート（Kahnのアルゴリズム）
        // 入次数 = このテーブルが依存しているテーブルの数
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        for table_name in added_table_names.iter() {
            in_degree.insert(*table_name, 0);
        }

        // 入次数を計算（各テーブルの依存先の数）
        for (table_name, deps) in &dependencies {
            in_degree.insert(*table_name, deps.len());
        }

        // 入次数が0のノードをキューに追加（依存先がないテーブル = 先に作成すべきテーブル）
        let mut queue: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(&name, _)| name)
            .collect();

        // 安定したソートのためにアルファベット順にソート
        queue.sort();

        let mut sorted: Vec<Table> = Vec::new();

        while let Some(table_name) = queue.pop() {
            if let Some(table) = table_map.get(table_name) {
                sorted.push((*table).clone());
            }

            // このテーブルを参照している（このテーブルに依存している）テーブルの入次数を減らす
            for (other_table, deps) in &dependencies {
                if deps.contains(&table_name) {
                    if let Some(degree) = in_degree.get_mut(other_table) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push(other_table);
                            // 安定したソートのために再ソート
                            queue.sort();
                        }
                    }
                }
            }
        }

        // 循環参照のチェック
        if sorted.len() != self.added_tables.len() {
            let remaining: Vec<&str> = in_degree
                .iter()
                .filter(|(_, &degree)| degree > 0)
                .map(|(&name, _)| name)
                .collect();
            return Err(format!(
                "Circular reference detected. The following tables have circular references: {:?}",
                remaining
            ));
        }

        Ok(sorted)
    }

    /// 外部キー制約による依存関係を考慮して、削除テーブルを逆順にソート
    ///
    /// 参照元テーブルが先に削除されるように並び替えます。
    /// 追加テーブルの逆順になります。
    ///
    /// # Arguments
    ///
    /// * `all_tables` - 全テーブル情報（削除されるテーブルの定義を含む）
    ///
    /// # Returns
    ///
    /// ソートされたテーブル名のリスト
    pub fn sort_removed_tables_by_dependency(
        &self,
        all_tables: &HashMap<String, Table>,
    ) -> Vec<String> {
        if self.removed_tables.is_empty() {
            return Vec::new();
        }

        // 削除されるテーブル名のセット
        let removed_table_names: HashSet<&str> =
            self.removed_tables.iter().map(|s| s.as_str()).collect();

        // 依存関係グラフを構築
        let mut dependencies: HashMap<&str, Vec<&str>> = HashMap::new();

        for table_name in &self.removed_tables {
            if let Some(table) = all_tables.get(table_name) {
                let mut deps = Vec::new();
                for constraint in &table.constraints {
                    if let Constraint::FOREIGN_KEY {
                        referenced_table, ..
                    } = constraint
                    {
                        // 参照先が削除されるテーブルに含まれる場合のみ
                        if removed_table_names.contains(referenced_table.as_str()) {
                            deps.push(referenced_table.as_str());
                        }
                    }
                }
                dependencies.insert(table_name.as_str(), deps);
            } else {
                dependencies.insert(table_name.as_str(), Vec::new());
            }
        }

        // トポロジカルソート
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        for table_name in removed_table_names.iter() {
            in_degree.insert(*table_name, 0);
        }

        for deps in dependencies.values() {
            for dep in deps {
                if let Some(degree) = in_degree.get_mut(dep) {
                    *degree += 1;
                }
            }
        }

        let mut queue: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(&name, _)| name)
            .collect();
        queue.sort();

        let mut sorted: Vec<String> = Vec::new();

        while let Some(table_name) = queue.pop() {
            sorted.push(table_name.to_string());

            for (dependent, deps) in &dependencies {
                if deps.contains(&table_name) {
                    if let Some(degree) = in_degree.get_mut(dependent) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push(dependent);
                            queue.sort();
                        }
                    }
                }
            }
        }

        // 削除は逆順（参照元を先に削除）
        sorted.reverse();
        sorted
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
        assert!(matches!(diff.changes[0], ColumnChange::TypeChanged { .. }));
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

    #[test]
    fn test_sort_added_tables_no_dependencies() {
        let mut diff = SchemaDiff::new();
        diff.added_tables.push(Table::new("users".to_string()));
        diff.added_tables.push(Table::new("posts".to_string()));

        let sorted = diff.sort_added_tables_by_dependency().unwrap();

        assert_eq!(sorted.len(), 2);
        // 依存関係がない場合はアルファベット順
        assert_eq!(sorted[0].name, "users");
        assert_eq!(sorted[1].name, "posts");
    }

    #[test]
    fn test_sort_added_tables_with_foreign_key() {
        let mut diff = SchemaDiff::new();

        // usersテーブル（参照先）
        let users_table = Table::new("users".to_string());

        // postsテーブル（usersを参照）
        let mut posts_table = Table::new("posts".to_string());
        posts_table.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
        });

        // postsを先に追加（依存関係解決前の順序）
        diff.added_tables.push(posts_table);
        diff.added_tables.push(users_table);

        let sorted = diff.sort_added_tables_by_dependency().unwrap();

        assert_eq!(sorted.len(), 2);
        // usersが先に作成される必要がある
        assert_eq!(sorted[0].name, "users");
        assert_eq!(sorted[1].name, "posts");
    }

    #[test]
    fn test_sort_added_tables_chain_dependency() {
        let mut diff = SchemaDiff::new();

        // A -> B -> C の依存関係（CがBを参照、BがAを参照）
        let table_a = Table::new("a".to_string());

        let mut table_b = Table::new("b".to_string());
        table_b.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["a_id".to_string()],
            referenced_table: "a".to_string(),
            referenced_columns: vec!["id".to_string()],
        });

        let mut table_c = Table::new("c".to_string());
        table_c.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["b_id".to_string()],
            referenced_table: "b".to_string(),
            referenced_columns: vec!["id".to_string()],
        });

        // 逆順で追加
        diff.added_tables.push(table_c);
        diff.added_tables.push(table_b);
        diff.added_tables.push(table_a);

        let sorted = diff.sort_added_tables_by_dependency().unwrap();

        assert_eq!(sorted.len(), 3);
        // A -> B -> C の順序で作成される
        assert_eq!(sorted[0].name, "a");
        assert_eq!(sorted[1].name, "b");
        assert_eq!(sorted[2].name, "c");
    }

    #[test]
    fn test_sort_added_tables_circular_dependency() {
        let mut diff = SchemaDiff::new();

        // 循環参照: A -> B -> A
        let mut table_a = Table::new("a".to_string());
        table_a.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["b_id".to_string()],
            referenced_table: "b".to_string(),
            referenced_columns: vec!["id".to_string()],
        });

        let mut table_b = Table::new("b".to_string());
        table_b.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["a_id".to_string()],
            referenced_table: "a".to_string(),
            referenced_columns: vec!["id".to_string()],
        });

        diff.added_tables.push(table_a);
        diff.added_tables.push(table_b);

        let result = diff.sort_added_tables_by_dependency();

        // Circular reference should result in an error
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Circular reference"));
    }

    #[test]
    fn test_sort_added_tables_external_reference() {
        let mut diff = SchemaDiff::new();

        // postsがusersを参照するが、usersは追加されるテーブルに含まれない
        // （既存のテーブルを参照している場合）
        let mut posts_table = Table::new("posts".to_string());
        posts_table.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(), // usersは追加テーブルに含まれない
            referenced_columns: vec!["id".to_string()],
        });

        diff.added_tables.push(posts_table);

        let sorted = diff.sort_added_tables_by_dependency().unwrap();

        // 外部参照は依存関係として扱わないのでそのまま追加される
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].name, "posts");
    }
}
