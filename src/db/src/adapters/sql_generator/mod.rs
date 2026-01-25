// SQL生成アダプター
//
// スキーマ定義から各データベース方言用のDDL文を生成するアダプター層。

pub mod mysql;
pub mod postgres;
pub mod sqlite;
pub mod sqlite_table_recreator;

use crate::core::schema::{Column, ColumnType, EnumDefinition, Index, Table};
use crate::core::schema_diff::{ColumnDiff, EnumDiff, RenamedColumn};

/// カラム定義の共通組み立てヘルパー
pub(crate) fn build_column_definition(
    column: &Column,
    type_str: String,
    extra_parts: &[&str],
) -> String {
    let mut parts = Vec::new();

    parts.push(column.name.clone());
    parts.push(type_str);

    if !column.nullable {
        parts.push("NOT NULL".to_string());
    }

    for part in extra_parts {
        if !part.is_empty() {
            parts.push((*part).to_string());
        }
    }

    if let Some(ref default_value) = column.default_value {
        parts.push(format!("DEFAULT {}", default_value));
    }

    parts.join(" ")
}

/// マイグレーション方向
///
/// マイグレーションの適用方向を表現します。
/// up/down SQLの生成時に使用されます。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationDirection {
    /// 順方向のマイグレーション（up.sql）
    Up,
    /// 逆方向のマイグレーション（down.sql）
    Down,
}

impl MigrationDirection {
    /// 対象の型を取得
    ///
    /// Up方向では新しい型、Down方向では古い型を返します。
    pub fn target_type<'a>(
        &self,
        old_type: &'a ColumnType,
        new_type: &'a ColumnType,
    ) -> &'a ColumnType {
        match self {
            MigrationDirection::Up => new_type,
            MigrationDirection::Down => old_type,
        }
    }

    /// ソースの型を取得
    ///
    /// Up方向では古い型、Down方向では新しい型を返します。
    pub fn source_type<'a>(
        &self,
        old_type: &'a ColumnType,
        new_type: &'a ColumnType,
    ) -> &'a ColumnType {
        match self {
            MigrationDirection::Up => old_type,
            MigrationDirection::Down => new_type,
        }
    }
}

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

    /// ALTER TABLE ADD COLUMN文を生成
    ///
    /// # Arguments
    ///
    /// * `table_name` - テーブル名
    /// * `column` - 追加するカラム
    ///
    /// # Returns
    ///
    /// ALTER TABLE ADD COLUMN文のSQL文字列
    fn generate_add_column(&self, table_name: &str, column: &Column) -> String;

    /// ALTER TABLE DROP COLUMN文を生成
    ///
    /// # Arguments
    ///
    /// * `table_name` - テーブル名
    /// * `column_name` - 削除するカラム名
    fn generate_drop_column(&self, table_name: &str, column_name: &str) -> String {
        format!("ALTER TABLE {} DROP COLUMN {}", table_name, column_name)
    }

    /// DROP TABLE文を生成
    ///
    /// # Arguments
    ///
    /// * `table_name` - テーブル名
    fn generate_drop_table(&self, table_name: &str) -> String {
        format!("DROP TABLE {}", table_name)
    }

    /// DROP INDEX文を生成
    ///
    /// # Arguments
    ///
    /// * `table_name` - テーブル名（MySQL向け）
    /// * `index` - インデックス定義
    fn generate_drop_index(&self, _table_name: &str, index: &Index) -> String {
        format!("DROP INDEX {}", index.name)
    }

    /// DOWN時に復元が必要なテーブルの注意コメントを生成
    fn generate_missing_table_notice(&self, table_name: &str) -> String {
        format!(
            "-- NOTE: Manually add CREATE TABLE statement for '{}' if rollback is needed",
            table_name
        )
    }

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

    /// カラム型変更のALTER TABLE文を生成
    ///
    /// # Arguments
    ///
    /// * `table` - 対象テーブルの完全な定義（direction=Upなら新定義、Downなら旧定義）
    /// * `column_diff` - カラム差分情報
    /// * `direction` - マイグレーション方向（Up/Down）
    ///
    /// # Returns
    ///
    /// ALTER TABLE文のベクター（SQLiteは複数文）
    fn generate_alter_column_type(
        &self,
        _table: &Table,
        _column_diff: &ColumnDiff,
        _direction: MigrationDirection,
    ) -> Vec<String> {
        // デフォルト実装：空のベクター
        // 各方言の実装でオーバーライド
        Vec::new()
    }

    /// カラム型変更のALTER TABLE文を生成（旧テーブル情報付き）
    ///
    /// SQLiteなど、カラム追加/削除を伴うテーブル再作成が必要な場合に使用します。
    /// 旧テーブルのカラム情報を基に、列交差ロジックでデータコピーSQLを生成します。
    ///
    /// # Arguments
    ///
    /// * `table` - 対象テーブルの新しい定義（direction=Upなら新定義、Downなら旧定義）
    /// * `old_table` - 対象テーブルの古い定義（列交差のための参照）
    /// * `column_diff` - カラム差分情報
    /// * `direction` - マイグレーション方向（Up/Down）
    ///
    /// # Returns
    ///
    /// ALTER TABLE文のベクター（SQLiteは複数文）
    fn generate_alter_column_type_with_old_table(
        &self,
        table: &Table,
        _old_table: Option<&Table>,
        column_diff: &ColumnDiff,
        direction: MigrationDirection,
    ) -> Vec<String> {
        // デフォルト実装：old_tableを無視して通常のgenerate_alter_column_typeを呼び出す
        self.generate_alter_column_type(table, column_diff, direction)
    }

    /// カラムリネームのALTER TABLE文を生成
    ///
    /// # Arguments
    ///
    /// * `table` - 対象テーブル
    /// * `renamed_column` - リネームされたカラム情報
    /// * `direction` - マイグレーション方向（Up/Down）
    ///
    /// # Returns
    ///
    /// ALTER TABLE RENAME COLUMN文のベクター
    fn generate_rename_column(
        &self,
        _table: &Table,
        _renamed_column: &RenamedColumn,
        _direction: MigrationDirection,
    ) -> Vec<String> {
        // デフォルト実装：空のベクター
        // 各方言の実装でオーバーライド
        Vec::new()
    }

    /// ENUM型の作成（PostgreSQL専用）
    fn generate_create_enum_type(&self, _enum_def: &EnumDefinition) -> Vec<String> {
        Vec::new()
    }

    /// ENUM値追加（PostgreSQL専用）
    fn generate_add_enum_value(&self, _enum_name: &str, _value: &str) -> Vec<String> {
        Vec::new()
    }

    /// ENUM再作成（PostgreSQL専用）
    fn generate_recreate_enum_type(&self, _enum_diff: &EnumDiff) -> Vec<String> {
        Vec::new()
    }

    /// ENUM削除（PostgreSQL専用）
    fn generate_drop_enum_type(&self, _enum_name: &str) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::Column;
    use crate::core::schema_diff::RenamedColumn;

    // ダミー実装（デフォルト実装のテスト用）
    struct DummySqlGenerator;

    impl SqlGenerator for DummySqlGenerator {
        fn generate_create_table(&self, _table: &Table) -> String {
            String::new()
        }

        fn generate_create_index(&self, _table: &Table, _index: &Index) -> String {
            String::new()
        }

        fn generate_add_column(&self, _table_name: &str, _column: &Column) -> String {
            String::new()
        }

        fn generate_alter_table_add_constraint(
            &self,
            _table: &Table,
            _constraint_index: usize,
        ) -> String {
            String::new()
        }
    }

    #[test]
    fn test_generate_rename_column_default_returns_empty() {
        // デフォルト実装は空のベクターを返す
        let generator = DummySqlGenerator;
        let table = Table::new("users".to_string());
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
        let renamed_column = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let result =
            generator.generate_rename_column(&table, &renamed_column, MigrationDirection::Up);
        assert!(result.is_empty());

        let result =
            generator.generate_rename_column(&table, &renamed_column, MigrationDirection::Down);
        assert!(result.is_empty());
    }

    #[test]
    fn test_migration_direction_target_type() {
        let old_type = ColumnType::INTEGER { precision: None };
        let new_type = ColumnType::VARCHAR { length: 255 };

        // Up方向では新しい型が対象
        assert_eq!(
            MigrationDirection::Up.target_type(&old_type, &new_type),
            &new_type
        );

        // Down方向では古い型が対象
        assert_eq!(
            MigrationDirection::Down.target_type(&old_type, &new_type),
            &old_type
        );
    }

    #[test]
    fn test_migration_direction_source_type() {
        let old_type = ColumnType::INTEGER { precision: None };
        let new_type = ColumnType::VARCHAR { length: 255 };

        // Up方向では古い型がソース
        assert_eq!(
            MigrationDirection::Up.source_type(&old_type, &new_type),
            &old_type
        );

        // Down方向では新しい型がソース
        assert_eq!(
            MigrationDirection::Down.source_type(&old_type, &new_type),
            &new_type
        );
    }
}
