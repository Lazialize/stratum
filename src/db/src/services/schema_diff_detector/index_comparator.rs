// インデックス差分検出

use crate::core::schema_diff::TableDiff;
use std::collections::HashSet;

use super::SchemaDiffDetector;

impl SchemaDiffDetector {
    /// インデックス差分を検出
    pub(crate) fn detect_index_diff(
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
}
