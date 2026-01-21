// スキーマ差分検出サービス
//
// 2つのスキーマ間の差分を検出するサービス。
// テーブル、カラム、インデックス、制約の追加、削除、変更を検出します。

use crate::core::schema::Schema;
use crate::core::schema_diff::{ColumnDiff, SchemaDiff, TableDiff};
use std::collections::HashSet;

/// スキーマ差分検出サービス
#[derive(Debug, Clone)]
pub struct SchemaDiffDetector {}

impl SchemaDiffDetector {
    /// 新しいSchemaDiffDetectorを作成
    pub fn new() -> Self {
        Self {}
    }

    /// スキーマ差分を検出
    ///
    /// # Arguments
    ///
    /// * `old_schema` - 変更前のスキーマ
    /// * `new_schema` - 変更後のスキーマ
    ///
    /// # Returns
    ///
    /// スキーマ差分
    pub fn detect_diff(&self, old_schema: &Schema, new_schema: &Schema) -> SchemaDiff {
        let mut diff = SchemaDiff::new();

        let old_table_names: HashSet<&String> = old_schema.tables.keys().collect();
        let new_table_names: HashSet<&String> = new_schema.tables.keys().collect();

        // 追加されたテーブル
        for table_name in new_table_names.difference(&old_table_names) {
            if let Some(table) = new_schema.tables.get(*table_name) {
                diff.added_tables.push(table.clone());
            }
        }

        // 削除されたテーブル
        for table_name in old_table_names.difference(&new_table_names) {
            diff.removed_tables.push((*table_name).clone());
        }

        // 変更されたテーブル
        for table_name in old_table_names.intersection(&new_table_names) {
            if let (Some(old_table), Some(new_table)) = (
                old_schema.tables.get(*table_name),
                new_schema.tables.get(*table_name),
            ) {
                let table_diff = self.detect_table_diff(old_table, new_table);
                if !table_diff.is_empty() {
                    diff.modified_tables.push(table_diff);
                }
            }
        }

        diff
    }

    /// テーブル差分を検出
    fn detect_table_diff(
        &self,
        old_table: &crate::core::schema::Table,
        new_table: &crate::core::schema::Table,
    ) -> TableDiff {
        let mut table_diff = TableDiff::new(old_table.name.clone());

        // カラムの差分を検出
        self.detect_column_diff(old_table, new_table, &mut table_diff);

        // インデックスの差分を検出
        self.detect_index_diff(old_table, new_table, &mut table_diff);

        // 制約の差分を検出
        self.detect_constraint_diff(old_table, new_table, &mut table_diff);

        table_diff
    }

    /// カラム差分を検出
    fn detect_column_diff(
        &self,
        old_table: &crate::core::schema::Table,
        new_table: &crate::core::schema::Table,
        table_diff: &mut TableDiff,
    ) {
        let old_column_names: HashSet<&String> =
            old_table.columns.iter().map(|c| &c.name).collect();
        let new_column_names: HashSet<&String> =
            new_table.columns.iter().map(|c| &c.name).collect();

        // 追加されたカラム
        for column_name in new_column_names.difference(&old_column_names) {
            if let Some(column) = new_table.columns.iter().find(|c| &c.name == *column_name) {
                table_diff.added_columns.push(column.clone());
            }
        }

        // 削除されたカラム
        for column_name in old_column_names.difference(&new_column_names) {
            table_diff.removed_columns.push((*column_name).clone());
        }

        // 変更されたカラム
        for column_name in old_column_names.intersection(&new_column_names) {
            if let (Some(old_column), Some(new_column)) = (
                old_table.columns.iter().find(|c| &c.name == *column_name),
                new_table.columns.iter().find(|c| &c.name == *column_name),
            ) {
                // カラムの定義が変更されているか確認
                if old_column != new_column {
                    let column_diff =
                        ColumnDiff::new((*column_name).clone(), old_column.clone(), new_column.clone());
                    if !column_diff.changes.is_empty() {
                        table_diff.modified_columns.push(column_diff);
                    }
                }
            }
        }
    }

    /// インデックス差分を検出
    fn detect_index_diff(
        &self,
        old_table: &crate::core::schema::Table,
        new_table: &crate::core::schema::Table,
        table_diff: &mut TableDiff,
    ) {
        let old_index_names: HashSet<&String> = old_table.indexes.iter().map(|i| &i.name).collect();
        let new_index_names: HashSet<&String> = new_table.indexes.iter().map(|i| &i.name).collect();

        // 追加されたインデックス
        for index_name in new_index_names.difference(&old_index_names) {
            if let Some(index) = new_table.indexes.iter().find(|i| &i.name == *index_name) {
                table_diff.added_indexes.push(index.clone());
            }
        }

        // 削除されたインデックス
        for index_name in old_index_names.difference(&new_index_names) {
            table_diff.removed_indexes.push((*index_name).clone());
        }
    }

    /// 制約差分を検出
    fn detect_constraint_diff(
        &self,
        old_table: &crate::core::schema::Table,
        new_table: &crate::core::schema::Table,
        table_diff: &mut TableDiff,
    ) {
        // 制約は名前がないため、内容で比較
        let old_constraints: HashSet<_> = old_table.constraints.iter().collect();
        let new_constraints: HashSet<_> = new_table.constraints.iter().collect();

        // 追加された制約
        for constraint in new_constraints.difference(&old_constraints) {
            table_diff.added_constraints.push((*constraint).clone());
        }

        // 削除された制約
        for constraint in old_constraints.difference(&new_constraints) {
            table_diff.removed_constraints.push((*constraint).clone());
        }
    }
}

impl Default for SchemaDiffDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{Column, ColumnType, Table};

    #[test]
    fn test_new_service() {
        let service = SchemaDiffDetector::new();
        assert!(format!("{:?}", service).contains("SchemaDiffDetector"));
    }

    #[test]
    fn test_detect_diff_empty_schemas() {
        let service = SchemaDiffDetector::new();
        let schema1 = Schema::new("1.0".to_string());
        let schema2 = Schema::new("1.0".to_string());

        let diff = service.detect_diff(&schema1, &schema2);

        assert!(diff.is_empty());
    }

    #[test]
    fn test_detect_table_added() {
        let service = SchemaDiffDetector::new();
        let schema1 = Schema::new("1.0".to_string());

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(table);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.added_tables.len(), 1);
        assert_eq!(diff.added_tables[0].name, "users");
    }

    #[test]
    fn test_detect_table_removed() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(table);

        let schema2 = Schema::new("1.0".to_string());

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.removed_tables.len(), 1);
        assert_eq!(diff.removed_tables[0], "users");
    }

    #[test]
    fn test_detect_column_added() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        table2.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table2.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_tables.len(), 1);
        assert_eq!(diff.modified_tables[0].added_columns.len(), 1);
        assert_eq!(diff.modified_tables[0].added_columns[0].name, "name");
    }
}
