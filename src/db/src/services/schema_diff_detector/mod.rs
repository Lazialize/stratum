// スキーマ差分検出サービス
//
// 2つのスキーマ間の差分を検出するサービス。
// テーブル、カラム、インデックス、制約の追加、削除、変更を検出します。

mod column_comparator;
mod constraint_comparator;
mod enum_comparator;
mod index_comparator;
mod table_comparator;

use crate::core::error::ValidationWarning;
use crate::core::schema::Schema;
use crate::core::schema_diff::{RenamedTable, SchemaDiff};
use std::collections::HashSet;

/// スキーマ差分検出サービス
#[derive(Debug, Clone)]
pub struct SchemaDiffDetectorService {}

impl SchemaDiffDetectorService {
    /// 新しいSchemaDiffDetectorServiceを作成
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

        diff.enum_recreate_allowed = new_schema.enum_recreate_allowed;

        self.detect_enum_diff(old_schema, new_schema, &mut diff);

        let old_table_names: HashSet<&String> = old_schema.tables.keys().collect();
        let new_table_names: HashSet<&String> = new_schema.tables.keys().collect();

        // リネームされたテーブルの旧名を追跡
        let mut renamed_old_names: HashSet<String> = HashSet::new();

        // 追加されたテーブル（リネームを含む可能性）
        for table_name in new_table_names.difference(&old_table_names) {
            if let Some(table) = new_schema.tables.get(*table_name) {
                // renamed_from がある場合はリネームとして処理
                if let Some(ref old_name) = table.renamed_from {
                    if old_schema.tables.contains_key(old_name) {
                        diff.renamed_tables.push(RenamedTable {
                            old_name: old_name.clone(),
                            new_table: table.clone(),
                        });
                        renamed_old_names.insert(old_name.clone());
                        continue;
                    }
                }
                diff.added_tables.push(table.clone());
            }
        }

        // 削除されたテーブル（リネームされたものを除外）
        for table_name in old_table_names.difference(&new_table_names) {
            if !renamed_old_names.contains(*table_name) {
                diff.removed_tables.push((*table_name).clone());
            }
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

    /// スキーマ差分を検出（警告付き）
    ///
    /// # Arguments
    ///
    /// * `old_schema` - 変更前のスキーマ
    /// * `new_schema` - 変更後のスキーマ
    ///
    /// # Returns
    ///
    /// スキーマ差分と警告のタプル
    pub fn detect_diff_with_warnings(
        &self,
        old_schema: &Schema,
        new_schema: &Schema,
    ) -> (SchemaDiff, Vec<ValidationWarning>) {
        let mut diff = SchemaDiff::new();
        let mut warnings = Vec::new();

        diff.enum_recreate_allowed = new_schema.enum_recreate_allowed;

        self.detect_enum_diff(old_schema, new_schema, &mut diff);

        let old_table_names: HashSet<&String> = old_schema.tables.keys().collect();
        let new_table_names: HashSet<&String> = new_schema.tables.keys().collect();

        // リネームされたテーブルの旧名を追跡
        let mut renamed_old_names: HashSet<String> = HashSet::new();

        // 追加されたテーブル（リネームを含む可能性）
        for table_name in new_table_names.difference(&old_table_names) {
            if let Some(table) = new_schema.tables.get(*table_name) {
                // renamed_from がある場合はリネームとして処理
                if let Some(ref old_name) = table.renamed_from {
                    if old_schema.tables.contains_key(old_name) {
                        diff.renamed_tables.push(RenamedTable {
                            old_name: old_name.clone(),
                            new_table: table.clone(),
                        });
                        renamed_old_names.insert(old_name.clone());
                        continue;
                    }
                }
                diff.added_tables.push(table.clone());
            }
        }

        // 削除されたテーブル（リネームされたものを除外）
        for table_name in old_table_names.difference(&new_table_names) {
            if !renamed_old_names.contains(*table_name) {
                diff.removed_tables.push((*table_name).clone());
            }
        }

        // 変更されたテーブル（警告付き）
        for table_name in old_table_names.intersection(&new_table_names) {
            if let (Some(old_table), Some(new_table)) = (
                old_schema.tables.get(*table_name),
                new_schema.tables.get(*table_name),
            ) {
                let (table_diff, table_warnings) =
                    self.detect_table_diff_with_warnings(old_table, new_table);
                if !table_diff.is_empty() {
                    diff.modified_tables.push(table_diff);
                }
                warnings.extend(table_warnings);
            }
        }

        (diff, warnings)
    }
}

impl Default for SchemaDiffDetectorService {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::services::traits::SchemaDiffDetector for SchemaDiffDetectorService {
    fn detect_diff_with_warnings(
        &self,
        old_schema: &Schema,
        new_schema: &Schema,
    ) -> (SchemaDiff, Vec<ValidationWarning>) {
        self.detect_diff_with_warnings(old_schema, new_schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{Column, ColumnType, Table};

    #[test]
    fn test_new_service() {
        let service = SchemaDiffDetectorService::new();
        assert!(format!("{:?}", service).contains("SchemaDiffDetectorService"));
    }

    #[test]
    fn test_detect_diff_empty_schemas() {
        let service = SchemaDiffDetectorService::new();
        let schema1 = Schema::new("1.0".to_string());
        let schema2 = Schema::new("1.0".to_string());

        let diff = service.detect_diff(&schema1, &schema2);

        assert!(diff.is_empty());
    }

    #[test]
    fn test_detect_table_added() {
        let service = SchemaDiffDetectorService::new();
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
        let service = SchemaDiffDetectorService::new();

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
    fn test_detect_table_modified() {
        let service = SchemaDiffDetectorService::new();

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
            ColumnType::VARCHAR { length: 255 },
            true,
        ));
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);
        assert_eq!(diff.modified_tables.len(), 1);
        assert_eq!(diff.modified_tables[0].table_name, "users");
    }

    #[test]
    fn test_detect_table_renamed() {
        let service = SchemaDiffDetectorService::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
        old_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(old_table);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut new_table = Table::new("accounts".to_string());
        new_table.renamed_from = Some("users".to_string());
        new_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(new_table);

        let diff = service.detect_diff(&schema1, &schema2);
        assert_eq!(diff.renamed_tables.len(), 1);
        assert_eq!(diff.renamed_tables[0].old_name, "users");
        assert_eq!(diff.renamed_tables[0].new_table.name, "accounts");
        // Old table should NOT appear in removed
        assert!(diff.removed_tables.is_empty());
    }

    #[test]
    fn test_detect_diff_with_warnings() {
        let service = SchemaDiffDetectorService::new();
        let schema1 = Schema::new("1.0".to_string());

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(table);

        let (diff, _warnings) = service.detect_diff_with_warnings(&schema1, &schema2);
        assert_eq!(diff.added_tables.len(), 1);
    }

    #[test]
    fn test_detect_diff_with_warnings_modified_table() {
        let service = SchemaDiffDetectorService::new();

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
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            true,
        ));
        schema2.add_table(table2);

        let (diff, _warnings) = service.detect_diff_with_warnings(&schema1, &schema2);
        assert_eq!(diff.modified_tables.len(), 1);
    }

    #[test]
    fn test_detect_diff_with_warnings_renamed_table() {
        let service = SchemaDiffDetectorService::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut old_table = Table::new("users".to_string());
        old_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(old_table);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut new_table = Table::new("accounts".to_string());
        new_table.renamed_from = Some("users".to_string());
        new_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(new_table);

        let (diff, _warnings) = service.detect_diff_with_warnings(&schema1, &schema2);
        assert_eq!(diff.renamed_tables.len(), 1);
        assert!(diff.removed_tables.is_empty());
    }

    #[test]
    fn test_default_impl() {
        let service = SchemaDiffDetectorService::default();
        let s1 = Schema::new("1.0".to_string());
        let s2 = Schema::new("1.0".to_string());
        let diff = service.detect_diff(&s1, &s2);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_trait_impl() {
        use crate::services::traits::SchemaDiffDetector;
        let service = SchemaDiffDetectorService::new();
        let s1 = Schema::new("1.0".to_string());
        let s2 = Schema::new("1.0".to_string());
        let (diff, warnings) = SchemaDiffDetector::detect_diff_with_warnings(&service, &s1, &s2);
        assert!(diff.is_empty());
        assert!(warnings.is_empty());
    }
}
