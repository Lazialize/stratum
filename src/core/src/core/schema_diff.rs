// スキーマ差分ドメインモデル
//
// スキーマ間の差分を表現する型システム。
// テーブル、カラム、インデックス、制約の追加、削除、変更を表現します。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::core::error::ValidationError;
use crate::core::schema::{Column, Constraint, EnumDefinition, Index, Table};

/// FK制約から依存関係グラフを構築
///
/// 各ノード（テーブル名）について、そのテーブルが参照しているテーブル名のリストを返します。
/// `target_names` に含まれるテーブル間の依存関係のみを抽出します。
fn build_dependency_graph<'a, F>(
    target_names: &HashSet<&'a str>,
    get_constraints: F,
) -> HashMap<&'a str, Vec<&'a str>>
where
    F: Fn(&&'a str) -> Option<&'a Vec<Constraint>>,
{
    let mut dependencies: HashMap<&'a str, Vec<&'a str>> = HashMap::new();

    for &table_name in target_names {
        let mut deps = Vec::new();
        if let Some(constraints) = get_constraints(&table_name) {
            for constraint in constraints {
                if let Constraint::FOREIGN_KEY {
                    referenced_table, ..
                } = constraint
                {
                    if target_names.contains(referenced_table.as_str()) {
                        deps.push(referenced_table.as_str());
                    }
                }
            }
        }
        dependencies.insert(table_name, deps);
    }

    dependencies
}

/// Kahnのアルゴリズムによるトポロジカルソート
///
/// 依存先（参照されるテーブル）が先に来るように並び替えます。
/// 循環参照がある場合、残余ノードのリストを返します。
///
/// # Returns
///
/// (ソート済みノード名, 循環に含まれる残余ノード名)
fn topological_sort_kahn<'a>(
    nodes: &HashSet<&'a str>,
    dependencies: &HashMap<&'a str, Vec<&'a str>>,
) -> (Vec<&'a str>, Vec<&'a str>) {
    // 入次数 = このテーブルが依存しているテーブルの数（未処理の依存先カウント）
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    for &node in nodes {
        let deg = dependencies.get(node).map_or(0, |deps| deps.len());
        in_degree.insert(node, deg);
    }

    // 入次数が0のノードをキューに追加（依存先がないテーブル = 先に処理すべき）
    let mut queue: Vec<&str> = in_degree
        .iter()
        .filter(|(_, &degree)| degree == 0)
        .map(|(&name, _)| name)
        .collect();
    queue.sort();

    let mut sorted: Vec<&str> = Vec::new();

    while let Some(node) = queue.pop() {
        sorted.push(node);

        // このノードに依存しているノードの入次数を減らす
        for (&other, deps) in dependencies {
            if deps.contains(&node) {
                if let Some(degree) = in_degree.get_mut(other) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push(other);
                        queue.sort();
                    }
                }
            }
        }
    }

    // 循環参照チェック: 処理されなかったノード
    let remaining: Vec<&str> = in_degree
        .iter()
        .filter(|(_, &degree)| degree > 0)
        .map(|(&name, _)| name)
        .collect();

    (sorted, remaining)
}

/// スキーマ差分
///
/// 2つのスキーマ間の差分を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaDiff {
    /// ENUM再作成の許可フラグ
    pub enum_recreate_allowed: bool,

    /// 追加されたENUM定義
    pub added_enums: Vec<EnumDefinition>,

    /// 削除されたENUM定義
    pub removed_enums: Vec<String>,

    /// 変更されたENUM定義
    pub modified_enums: Vec<EnumDiff>,

    /// 追加されたテーブル
    pub added_tables: Vec<Table>,

    /// 削除されたテーブル
    pub removed_tables: Vec<String>,

    /// 変更されたテーブル
    pub modified_tables: Vec<TableDiff>,

    /// リネームされたテーブル
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub renamed_tables: Vec<RenamedTable>,
}

/// リネームされたテーブル
///
/// テーブル名の変更を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenamedTable {
    /// 旧テーブル名
    pub old_name: String,

    /// 新テーブル定義
    pub new_table: Table,
}

impl SchemaDiff {
    /// 新しいスキーマ差分を作成
    pub fn new() -> Self {
        Self {
            enum_recreate_allowed: false,
            added_enums: Vec::new(),
            removed_enums: Vec::new(),
            modified_enums: Vec::new(),
            added_tables: Vec::new(),
            removed_tables: Vec::new(),
            modified_tables: Vec::new(),
            renamed_tables: Vec::new(),
        }
    }

    /// 差分が空かどうか
    pub fn is_empty(&self) -> bool {
        self.added_enums.is_empty()
            && self.removed_enums.is_empty()
            && self.modified_enums.is_empty()
            && self.added_tables.is_empty()
            && self.removed_tables.is_empty()
            && self.modified_tables.is_empty()
            && self.renamed_tables.is_empty()
    }

    /// 差分の項目数を取得
    pub fn count(&self) -> usize {
        self.added_enums.len()
            + self.removed_enums.len()
            + self.modified_enums.len()
            + self.added_tables.len()
            + self.removed_tables.len()
            + self.renamed_tables.len()
            + self.modified_tables.len()
    }

    /// 外部キー制約による依存関係を考慮して、追加テーブルをトポロジカルソート
    ///
    /// 被参照テーブルが先に作成されるように並び替えます。
    /// 循環参照がある場合はエラーを返します。
    ///
    /// # Returns
    ///
    /// ソートされたテーブルのリスト、または循環参照エラー
    pub fn sort_added_tables_by_dependency(&self) -> Result<Vec<Table>, ValidationError> {
        if self.added_tables.is_empty() {
            return Ok(Vec::new());
        }

        // テーブル名 -> テーブルのマッピング
        let table_map: HashMap<&str, &Table> = self
            .added_tables
            .iter()
            .map(|t| (t.name.as_str(), t))
            .collect();

        let table_names: HashSet<&str> = table_map.keys().copied().collect();

        let dependencies = build_dependency_graph(&table_names, |name| {
            table_map.get(name).map(|t| &t.constraints)
        });

        let (sorted_names, remaining) = topological_sort_kahn(&table_names, &dependencies);

        if !remaining.is_empty() {
            return Err(ValidationError::Reference {
                message: format!(
                    "Circular reference detected. The following tables have circular references: {:?}",
                    remaining
                ),
                location: None,
                suggestion: Some("Remove or refactor circular foreign key dependencies".to_string()),
            });
        }

        let sorted = sorted_names
            .into_iter()
            .filter_map(|name| table_map.get(name).map(|t| (*t).clone()))
            .collect();

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

        let removed_table_names: HashSet<&str> =
            self.removed_tables.iter().map(|s| s.as_str()).collect();

        let dependencies = build_dependency_graph(&removed_table_names, |name| {
            all_tables.get(*name).map(|t| &t.constraints)
        });

        let (mut sorted, _) = topological_sort_kahn(&removed_table_names, &dependencies);

        // 作成順の逆 = 参照元テーブルを先に削除
        sorted.reverse();
        sorted.into_iter().map(|s| s.to_string()).collect()
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

    /// リネームされたカラム（旧名→新カラム定義）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub renamed_columns: Vec<RenamedColumn>,

    /// 追加されたインデックス
    pub added_indexes: Vec<Index>,

    /// 削除されたインデックス
    pub removed_indexes: Vec<String>,

    /// 変更されたインデックス
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modified_indexes: Vec<IndexDiff>,

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
            renamed_columns: Vec::new(),
            added_indexes: Vec::new(),
            removed_indexes: Vec::new(),
            modified_indexes: Vec::new(),
            added_constraints: Vec::new(),
            removed_constraints: Vec::new(),
        }
    }

    /// 差分が空かどうか
    pub fn is_empty(&self) -> bool {
        self.added_columns.is_empty()
            && self.removed_columns.is_empty()
            && self.modified_columns.is_empty()
            && self.renamed_columns.is_empty()
            && self.added_indexes.is_empty()
            && self.removed_indexes.is_empty()
            && self.modified_indexes.is_empty()
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

/// リネームされたカラム
///
/// カラム名の変更と同時に行われた属性変更を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenamedColumn {
    /// 旧カラム名
    pub old_name: String,

    /// 旧カラム定義（MySQL Down方向で必要）
    pub old_column: Column,

    /// 新カラム定義
    pub new_column: Column,

    /// リネームと同時に変更された属性
    pub changes: Vec<ColumnChange>,
}

/// ENUM差分
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumDiff {
    /// ENUM名
    pub enum_name: String,

    /// 変更前の値
    pub old_values: Vec<String>,

    /// 変更後の値
    pub new_values: Vec<String>,

    /// 追加された値
    pub added_values: Vec<String>,

    /// 削除された値
    pub removed_values: Vec<String>,

    /// 変更種別
    pub change_kind: EnumChangeKind,

    /// 参照カラム
    pub columns: Vec<EnumColumnRef>,
}

/// ENUM変更種別
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EnumChangeKind {
    /// 追加のみ
    AddOnly,
    /// 再作成が必要
    Recreate,
}

/// ENUM参照カラム
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumColumnRef {
    pub table_name: String,
    pub column_name: String,
}

/// インデックス差分
///
/// インデックスの変更内容を表現します。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexDiff {
    /// インデックス名
    pub index_name: String,

    /// 変更前のインデックス定義
    pub old_index: Index,

    /// 変更後のインデックス定義
    pub new_index: Index,
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

    /// カラム名の変更
    Renamed { old_name: String, new_name: String },
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
            on_delete: None,
            on_update: None,
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
            on_delete: None,
            on_update: None,
        });

        let mut table_c = Table::new("c".to_string());
        table_c.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["b_id".to_string()],
            referenced_table: "b".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
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
            on_delete: None,
            on_update: None,
        });

        let mut table_b = Table::new("b".to_string());
        table_b.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["a_id".to_string()],
            referenced_table: "a".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });

        diff.added_tables.push(table_a);
        diff.added_tables.push(table_b);

        let result = diff.sort_added_tables_by_dependency();

        // Circular reference should result in an error
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular reference"));
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
            on_delete: None,
            on_update: None,
        });

        diff.added_tables.push(posts_table);

        let sorted = diff.sort_added_tables_by_dependency().unwrap();

        // 外部参照は依存関係として扱わないのでそのまま追加される
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].name, "posts");
    }

    #[test]
    fn test_column_change_renamed() {
        // Renamedバリアントの生成と比較
        let change = ColumnChange::Renamed {
            old_name: "name".to_string(),
            new_name: "user_name".to_string(),
        };

        if let ColumnChange::Renamed { old_name, new_name } = &change {
            assert_eq!(old_name, "name");
            assert_eq!(new_name, "user_name");
        } else {
            panic!("Expected ColumnChange::Renamed");
        }
    }

    #[test]
    fn test_column_change_renamed_equality() {
        let change1 = ColumnChange::Renamed {
            old_name: "name".to_string(),
            new_name: "user_name".to_string(),
        };
        let change2 = ColumnChange::Renamed {
            old_name: "name".to_string(),
            new_name: "user_name".to_string(),
        };
        let change3 = ColumnChange::Renamed {
            old_name: "name".to_string(),
            new_name: "full_name".to_string(),
        };

        assert_eq!(change1, change2);
        assert_ne!(change1, change3);
    }

    #[test]
    fn test_column_change_renamed_serialization() {
        let change = ColumnChange::Renamed {
            old_name: "name".to_string(),
            new_name: "user_name".to_string(),
        };

        let json = serde_json::to_string(&change).unwrap();
        assert!(json.contains("Renamed"));
        assert!(json.contains("old_name"));
        assert!(json.contains("new_name"));

        let deserialized: ColumnChange = serde_json::from_str(&json).unwrap();
        assert_eq!(change, deserialized);
    }

    #[test]
    fn test_renamed_column_creation() {
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
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![],
        };

        assert_eq!(renamed.old_name, "name");
        assert_eq!(renamed.old_column.name, "name");
        assert_eq!(renamed.new_column.name, "user_name");
        assert!(renamed.changes.is_empty());
    }

    #[test]
    fn test_renamed_column_with_type_change() {
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 50 },
            false,
        );
        let mut new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        new_column.renamed_from = Some("name".to_string());

        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![ColumnChange::TypeChanged {
                old_type: "VARCHAR(50)".to_string(),
                new_type: "VARCHAR(100)".to_string(),
            }],
        };

        assert_eq!(renamed.changes.len(), 1);
        if let ColumnChange::TypeChanged { old_type, new_type } = &renamed.changes[0] {
            assert_eq!(old_type, "VARCHAR(50)");
            assert_eq!(new_type, "VARCHAR(100)");
        } else {
            panic!("Expected TypeChanged");
        }
    }

    #[test]
    fn test_renamed_column_equality() {
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

        let renamed1 = RenamedColumn {
            old_name: "name".to_string(),
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![],
        };
        let renamed2 = RenamedColumn {
            old_name: "name".to_string(),
            old_column: old_column.clone(),
            new_column: new_column.clone(),
            changes: vec![],
        };

        assert_eq!(renamed1, renamed2);
    }

    #[test]
    fn test_renamed_column_serialization() {
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

        let json = serde_json::to_string(&renamed).unwrap();
        assert!(json.contains("old_name"));
        assert!(json.contains("old_column"));
        assert!(json.contains("new_column"));

        let deserialized: RenamedColumn = serde_json::from_str(&json).unwrap();
        assert_eq!(renamed, deserialized);
    }

    #[test]
    fn test_table_diff_renamed_columns_default() {
        let table_diff = TableDiff::new("users".to_string());

        // デフォルトでrenamed_columnsは空
        assert!(table_diff.renamed_columns.is_empty());
        assert!(table_diff.is_empty());
    }

    #[test]
    fn test_table_diff_with_renamed_columns() {
        let mut table_diff = TableDiff::new("users".to_string());

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

        table_diff.renamed_columns.push(RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        });

        assert_eq!(table_diff.renamed_columns.len(), 1);
        assert!(!table_diff.is_empty());
    }

    #[test]
    fn test_table_diff_renamed_columns_serialization() {
        let mut table_diff = TableDiff::new("users".to_string());

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

        table_diff.renamed_columns.push(RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        });

        let json = serde_json::to_string(&table_diff).unwrap();
        assert!(json.contains("renamed_columns"));
        assert!(json.contains("old_name"));

        let deserialized: TableDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(table_diff, deserialized);
    }

    #[test]
    fn test_table_diff_empty_renamed_columns_not_serialized() {
        let table_diff = TableDiff::new("users".to_string());

        let json = serde_json::to_string(&table_diff).unwrap();
        // 空のrenamed_columnsはシリアライズされない
        assert!(!json.contains("renamed_columns"));
    }

    #[test]
    fn test_sort_removed_tables_chain_dependency() {
        let mut diff = SchemaDiff::new();

        // A -> B -> C の依存関係（CがBを参照、BがAを参照）
        // 削除順: C, B, A（参照元を先に削除）
        diff.removed_tables = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        let mut all_tables: HashMap<String, Table> = HashMap::new();

        let table_a = Table::new("a".to_string());

        let mut table_b = Table::new("b".to_string());
        table_b.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["a_id".to_string()],
            referenced_table: "a".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });

        let mut table_c = Table::new("c".to_string());
        table_c.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["b_id".to_string()],
            referenced_table: "b".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });

        all_tables.insert("a".to_string(), table_a);
        all_tables.insert("b".to_string(), table_b);
        all_tables.insert("c".to_string(), table_c);

        let sorted = diff.sort_removed_tables_by_dependency(&all_tables);

        assert_eq!(sorted.len(), 3);
        // 削除順: c（参照元）→ b → a（被参照先）
        assert_eq!(sorted[0], "c");
        assert_eq!(sorted[1], "b");
        assert_eq!(sorted[2], "a");
    }

    #[test]
    fn test_sort_removed_tables_with_foreign_key() {
        let mut diff = SchemaDiff::new();

        diff.removed_tables = vec!["users".to_string(), "posts".to_string()];

        let mut all_tables: HashMap<String, Table> = HashMap::new();

        let users_table = Table::new("users".to_string());
        let mut posts_table = Table::new("posts".to_string());
        posts_table.constraints.push(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        });

        all_tables.insert("users".to_string(), users_table);
        all_tables.insert("posts".to_string(), posts_table);

        let sorted = diff.sort_removed_tables_by_dependency(&all_tables);

        assert_eq!(sorted.len(), 2);
        // posts（参照元）が先に削除される
        assert_eq!(sorted[0], "posts");
        assert_eq!(sorted[1], "users");
    }

    #[test]
    fn test_sort_removed_tables_no_dependencies() {
        let mut diff = SchemaDiff::new();

        diff.removed_tables = vec!["users".to_string(), "posts".to_string()];

        let mut all_tables: HashMap<String, Table> = HashMap::new();
        all_tables.insert("users".to_string(), Table::new("users".to_string()));
        all_tables.insert("posts".to_string(), Table::new("posts".to_string()));

        let sorted = diff.sort_removed_tables_by_dependency(&all_tables);

        assert_eq!(sorted.len(), 2);
    }

    // ==========================================
    // IndexDiff テスト
    // ==========================================

    #[test]
    fn test_index_diff_new() {
        let old_index = Index::new(
            "idx_users_email".to_string(),
            vec!["email".to_string()],
            false,
        );
        let new_index = Index::new(
            "idx_users_email".to_string(),
            vec!["email".to_string(), "name".to_string()],
            true,
        );

        let diff = IndexDiff {
            index_name: "idx_users_email".to_string(),
            old_index: old_index.clone(),
            new_index: new_index.clone(),
        };

        assert_eq!(diff.index_name, "idx_users_email");
        assert_eq!(diff.old_index.columns.len(), 1);
        assert_eq!(diff.new_index.columns.len(), 2);
        assert!(!diff.old_index.unique);
        assert!(diff.new_index.unique);
    }

    #[test]
    fn test_table_diff_with_modified_indexes() {
        let mut table_diff = TableDiff::new("users".to_string());

        let old_index = Index::new("idx_email".to_string(), vec!["email".to_string()], false);
        let new_index = Index::new("idx_email".to_string(), vec!["email".to_string()], true);

        table_diff.modified_indexes.push(IndexDiff {
            index_name: "idx_email".to_string(),
            old_index,
            new_index,
        });

        assert!(!table_diff.is_empty());
        assert_eq!(table_diff.modified_indexes.len(), 1);
    }

    #[test]
    fn test_table_diff_modified_indexes_serialization() {
        let mut table_diff = TableDiff::new("users".to_string());

        let old_index = Index::new("idx_email".to_string(), vec!["email".to_string()], false);
        let new_index = Index::new("idx_email".to_string(), vec!["email".to_string()], true);

        table_diff.modified_indexes.push(IndexDiff {
            index_name: "idx_email".to_string(),
            old_index,
            new_index,
        });

        let json = serde_json::to_string(&table_diff).unwrap();
        assert!(json.contains("modified_indexes"));
        assert!(json.contains("idx_email"));
    }

    #[test]
    fn test_table_diff_empty_modified_indexes_not_serialized() {
        let table_diff = TableDiff::new("users".to_string());

        let json = serde_json::to_string(&table_diff).unwrap();
        // skip_serializing_if により空の場合はシリアライズされない
        assert!(!json.contains("modified_indexes"));
    }
}
