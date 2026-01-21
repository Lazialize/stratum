// SQL生成アダプター
//
// スキーマ定義から各データベース方言用のDDL文を生成するアダプター層。

pub mod mysql;
pub mod postgres;
pub mod sqlite;

use crate::core::schema::{Index, Table};

/// SQLジェネレータートレイト
///
/// 各データベース方言用のSQLジェネレーターが実装すべきインターフェース。
pub trait SqlGenerator {
    /// CREATE TABLE文を生成
    ///
    /// # Arguments
    ///
    /// * `table` - テーブル定義
    ///
    /// # Returns
    ///
    /// CREATE TABLE文のSQL文字列
    fn generate_create_table(&self, table: &Table) -> String;

    /// CREATE INDEX文を生成
    ///
    /// # Arguments
    ///
    /// * `table` - テーブル定義
    /// * `index` - インデックス定義
    ///
    /// # Returns
    ///
    /// CREATE INDEX文のSQL文字列
    fn generate_create_index(&self, table: &Table, index: &Index) -> String;

    /// ALTER TABLE文（制約追加）を生成
    ///
    /// # Arguments
    ///
    /// * `table` - テーブル定義
    /// * `constraint_index` - 追加する制約のインデックス
    ///
    /// # Returns
    ///
    /// ALTER TABLE文のSQL文字列
    fn generate_alter_table_add_constraint(&self, table: &Table, constraint_index: usize)
        -> String;
}
