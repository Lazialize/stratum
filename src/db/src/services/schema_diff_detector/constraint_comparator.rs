// 制約差分検出

use crate::core::schema_diff::TableDiff;
use std::collections::HashSet;

use super::SchemaDiffDetector;

impl SchemaDiffDetector {
    /// 制約差分を検出
    pub(crate) fn detect_constraint_diff(
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
