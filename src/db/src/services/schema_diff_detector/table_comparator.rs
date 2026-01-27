// テーブルレベルの差分検出

use crate::core::error::ValidationWarning;
use crate::core::schema_diff::TableDiff;

use super::SchemaDiffDetector;

impl SchemaDiffDetector {
    /// テーブル差分を検出
    pub(crate) fn detect_table_diff(
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

    /// テーブル差分を検出（警告付き）
    pub(crate) fn detect_table_diff_with_warnings(
        &self,
        old_table: &crate::core::schema::Table,
        new_table: &crate::core::schema::Table,
    ) -> (TableDiff, Vec<ValidationWarning>) {
        let mut table_diff = TableDiff::new(old_table.name.clone());
        let mut warnings = Vec::new();

        // カラムの差分を検出（警告付き）
        self.detect_column_diff_with_warnings(old_table, new_table, &mut table_diff, &mut warnings);

        // インデックスの差分を検出
        self.detect_index_diff(old_table, new_table, &mut table_diff);

        // 制約の差分を検出
        self.detect_constraint_diff(old_table, new_table, &mut table_diff);

        (table_diff, warnings)
    }
}
