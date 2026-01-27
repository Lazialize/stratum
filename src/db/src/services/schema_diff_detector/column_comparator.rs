// カラムレベルの差分検出

use crate::core::error::{ErrorLocation, ValidationWarning, WarningKind};
use crate::core::schema::Column;
use crate::core::schema_diff::{ColumnChange, ColumnDiff, RenamedColumn, TableDiff};
use std::collections::{HashMap, HashSet};

use super::SchemaDiffDetector;

impl SchemaDiffDetector {
    /// カラム差分を検出
    pub(crate) fn detect_column_diff(
        &self,
        old_table: &crate::core::schema::Table,
        new_table: &crate::core::schema::Table,
        table_diff: &mut TableDiff,
    ) {
        let old_col_map: HashMap<&str, &Column> = old_table
            .columns
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();
        let new_col_map: HashMap<&str, &Column> = new_table
            .columns
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();
        let old_column_names: HashSet<&String> =
            old_table.columns.iter().map(|c| &c.name).collect();
        let new_column_names: HashSet<&String> =
            new_table.columns.iter().map(|c| &c.name).collect();

        // リネームされたカラムを検出し、対象の旧カラム名を収集
        let mut renamed_old_names: HashSet<String> = HashSet::new();
        let mut renamed_new_names: HashSet<String> = HashSet::new();

        for new_column in &new_table.columns {
            if let Some(ref old_name) = new_column.renamed_from {
                // 旧テーブルに該当カラムが存在するか確認 (O(1) lookup)
                if let Some(old_column) = old_col_map.get(old_name.as_str()) {
                    // リネームとして検出
                    let changes = self.detect_column_changes(old_column, new_column);
                    table_diff.renamed_columns.push(RenamedColumn {
                        old_name: old_name.clone(),
                        old_column: (*old_column).clone(),
                        new_column: new_column.clone(),
                        changes,
                    });

                    renamed_old_names.insert(old_name.clone());
                    renamed_new_names.insert(new_column.name.clone());
                }
                // 旧カラムが存在しない場合は警告として扱い、
                // 通常のadded処理に含める（detect_diff_with_warningsで警告を収集）
            }
        }

        // 追加されたカラム（リネームを除く）
        for column_name in new_column_names.difference(&old_column_names) {
            // リネーム済みは除外
            if renamed_new_names.contains(*column_name) {
                continue;
            }
            if let Some(column) = new_col_map.get(column_name.as_str()) {
                table_diff.added_columns.push((*column).clone());
            }
        }

        // 削除されたカラム（リネームを除く）
        for column_name in old_column_names.difference(&new_column_names) {
            // リネーム済みは除外
            if renamed_old_names.contains(*column_name) {
                continue;
            }
            table_diff.removed_columns.push((*column_name).clone());
        }

        // 変更されたカラム (O(1) lookups via HashMap)
        for column_name in old_column_names.intersection(&new_column_names) {
            if let (Some(old_column), Some(new_column)) = (
                old_col_map.get(column_name.as_str()),
                new_col_map.get(column_name.as_str()),
            ) {
                // カラムの定義が変更されているか確認
                if old_column != new_column {
                    let column_diff = ColumnDiff::new(
                        (*column_name).clone(),
                        (*old_column).clone(),
                        (*new_column).clone(),
                    );
                    if !column_diff.changes.is_empty() {
                        table_diff.modified_columns.push(column_diff);
                    }
                }
            }
        }
    }

    /// カラム差分を検出（警告付き）
    pub(crate) fn detect_column_diff_with_warnings(
        &self,
        old_table: &crate::core::schema::Table,
        new_table: &crate::core::schema::Table,
        table_diff: &mut TableDiff,
        warnings: &mut Vec<ValidationWarning>,
    ) {
        let old_col_map: HashMap<&str, &Column> = old_table
            .columns
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();
        let new_col_map: HashMap<&str, &Column> = new_table
            .columns
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();
        let old_column_names: HashSet<&String> =
            old_table.columns.iter().map(|c| &c.name).collect();
        let new_column_names: HashSet<&String> =
            new_table.columns.iter().map(|c| &c.name).collect();

        // リネームされたカラムを検出し、対象の旧カラム名を収集
        let mut renamed_old_names: HashSet<String> = HashSet::new();
        let mut renamed_new_names: HashSet<String> = HashSet::new();

        for new_column in &new_table.columns {
            if let Some(ref old_name) = new_column.renamed_from {
                // 旧テーブルに該当カラムが存在するか確認 (O(1) lookup)
                if let Some(old_column) = old_col_map.get(old_name.as_str()) {
                    // リネームとして検出
                    let changes = self.detect_column_changes(old_column, new_column);
                    table_diff.renamed_columns.push(RenamedColumn {
                        old_name: old_name.clone(),
                        old_column: (*old_column).clone(),
                        new_column: new_column.clone(),
                        changes,
                    });

                    renamed_old_names.insert(old_name.clone());
                    renamed_new_names.insert(new_column.name.clone());
                } else {
                    // 旧カラムが存在しない場合は警告
                    warnings.push(ValidationWarning::new(
                        format!(
                            "Table '{}': renamed_from '{}' for column '{}' references a non-existent column. The renamed_from attribute will be ignored.",
                            old_table.name,
                            old_name,
                            new_column.name
                        ),
                        Some(ErrorLocation::with_table_and_column(&old_table.name, &new_column.name)),
                        WarningKind::OldColumnNotFound,
                    ));
                }
            }
        }

        // 追加されたカラム（リネームを除く）
        for column_name in new_column_names.difference(&old_column_names) {
            // リネーム済みは除外
            if renamed_new_names.contains(*column_name) {
                continue;
            }
            if let Some(column) = new_col_map.get(column_name.as_str()) {
                table_diff.added_columns.push((*column).clone());
            }
        }

        // 削除されたカラム（リネームを除く）
        for column_name in old_column_names.difference(&new_column_names) {
            // リネーム済みは除外
            if renamed_old_names.contains(*column_name) {
                continue;
            }
            table_diff.removed_columns.push((*column_name).clone());
        }

        // 変更されたカラム (O(1) lookups via HashMap)
        for column_name in old_column_names.intersection(&new_column_names) {
            if let (Some(old_column), Some(new_column)) = (
                old_col_map.get(column_name.as_str()),
                new_col_map.get(column_name.as_str()),
            ) {
                // カラムの定義が変更されているか確認
                if old_column != new_column {
                    let column_diff = ColumnDiff::new(
                        (*column_name).clone(),
                        (*old_column).clone(),
                        (*new_column).clone(),
                    );
                    if !column_diff.changes.is_empty() {
                        table_diff.modified_columns.push(column_diff);
                    }
                }
            }
        }
    }

    /// カラム間の変更を検出
    pub(crate) fn detect_column_changes(
        &self,
        old_column: &crate::core::schema::Column,
        new_column: &crate::core::schema::Column,
    ) -> Vec<ColumnChange> {
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

        changes
    }
}

#[cfg(test)]
mod tests {
    use crate::core::schema::{Column, ColumnType, Schema, Table};
    use crate::services::schema_diff_detector::SchemaDiffDetector;

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

    // リネーム検出のテスト

    #[test]
    fn test_detect_column_rename_simple() {
        // 単純なリネーム検出
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        let mut renamed_col = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        renamed_col.renamed_from = Some("name".to_string());
        table2.add_column(renamed_col);
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];

        // リネームされたカラムはrenamed_columnsに含まれる
        assert_eq!(table_diff.renamed_columns.len(), 1);
        assert_eq!(table_diff.renamed_columns[0].old_name, "name");
        assert_eq!(table_diff.renamed_columns[0].new_column.name, "user_name");

        // リネームされたカラムはadded/removedには含まれない
        assert!(table_diff.added_columns.is_empty());
        assert!(table_diff.removed_columns.is_empty());
    }

    #[test]
    fn test_detect_column_rename_with_type_change() {
        // リネーム+型変更の同時検出
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 50 },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        let mut renamed_col = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 }, // 型も変更
            false,
        );
        renamed_col.renamed_from = Some("name".to_string());
        table2.add_column(renamed_col);
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];

        assert_eq!(table_diff.renamed_columns.len(), 1);
        let renamed = &table_diff.renamed_columns[0];
        assert_eq!(renamed.old_name, "name");
        assert_eq!(renamed.new_column.name, "user_name");

        // 型変更がchangesに含まれる
        assert!(!renamed.changes.is_empty());
        assert!(renamed.changes.iter().any(|c| matches!(
            c,
            crate::core::schema_diff::ColumnChange::TypeChanged { .. }
        )));
    }

    #[test]
    fn test_detect_column_rename_with_nullable_change() {
        // リネーム+NULL制約変更の同時検出
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false, // NOT NULL
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        let mut renamed_col = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            true, // NULL許可に変更
        );
        renamed_col.renamed_from = Some("name".to_string());
        table2.add_column(renamed_col);
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        let table_diff = &diff.modified_tables[0];
        let renamed = &table_diff.renamed_columns[0];

        assert!(renamed.changes.iter().any(|c| matches!(
            c,
            crate::core::schema_diff::ColumnChange::NullableChanged { .. }
        )));
    }

    #[test]
    fn test_detect_multiple_column_renames() {
        // 複数カラムのリネーム検出
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        table1.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());

        let mut renamed_col1 = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        renamed_col1.renamed_from = Some("name".to_string());

        let mut renamed_col2 = Column::new(
            "user_email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        renamed_col2.renamed_from = Some("email".to_string());

        table2.add_column(renamed_col1);
        table2.add_column(renamed_col2);
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.renamed_columns.len(), 2);

        // 両方のリネームが検出される
        let old_names: Vec<&str> = table_diff
            .renamed_columns
            .iter()
            .map(|r| r.old_name.as_str())
            .collect();
        assert!(old_names.contains(&"name"));
        assert!(old_names.contains(&"email"));
    }

    #[test]
    fn test_detect_column_rename_old_column_not_exists() {
        // 旧カラム不存在時は通常のaddedとして処理
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
        let mut renamed_col = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        // 存在しないカラム名を指定
        renamed_col.renamed_from = Some("nonexistent_column".to_string());
        table2.add_column(renamed_col);
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        let table_diff = &diff.modified_tables[0];

        // リネームとしては検出されない
        assert!(table_diff.renamed_columns.is_empty());

        // 代わりに追加されたカラムとして扱われる
        assert_eq!(table_diff.added_columns.len(), 1);
        assert_eq!(table_diff.added_columns[0].name, "user_name");
    }

    #[test]
    fn test_detect_column_rename_preserves_old_column() {
        // RenamedColumn.old_columnが正しく設定される
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        let mut old_col = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 50 },
            false,
        );
        old_col.default_value = Some("'default'".to_string());
        table1.add_column(old_col);
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        let mut renamed_col = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        renamed_col.renamed_from = Some("name".to_string());
        table2.add_column(renamed_col);
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        let renamed = &diff.modified_tables[0].renamed_columns[0];

        // old_columnが旧カラムの定義を保持
        assert_eq!(renamed.old_column.name, "name");
        assert_eq!(
            renamed.old_column.default_value,
            Some("'default'".to_string())
        );
    }

    // detect_diff_with_warningsのテスト

    #[test]
    fn test_detect_diff_with_warnings_no_warnings() {
        // 警告がない場合
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        let mut renamed_col = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        renamed_col.renamed_from = Some("name".to_string());
        table2.add_column(renamed_col);
        schema2.add_table(table2);

        let (diff, warnings) = service.detect_diff_with_warnings(&schema1, &schema2);

        assert!(!diff.modified_tables.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_detect_diff_with_warnings_old_column_not_found() {
        // 旧カラムが存在しない場合の警告
        use crate::core::error::WarningKind;

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
        let mut renamed_col = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        // 存在しないカラム名を指定
        renamed_col.renamed_from = Some("nonexistent_column".to_string());
        table2.add_column(renamed_col);
        schema2.add_table(table2);

        let (diff, warnings) = service.detect_diff_with_warnings(&schema1, &schema2);

        // 警告が生成される
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, WarningKind::OldColumnNotFound);
        assert!(warnings[0].message.contains("nonexistent_column"));
        assert!(warnings[0].message.contains("users"));

        // カラムはaddedとして扱われる
        assert_eq!(diff.modified_tables[0].added_columns.len(), 1);
    }

    #[test]
    fn test_detect_diff_with_warnings_multiple_warnings() {
        // 複数の警告がある場合
        use crate::core::error::WarningKind;

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

        let mut renamed_col1 = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        renamed_col1.renamed_from = Some("nonexistent1".to_string());

        let mut renamed_col2 = Column::new(
            "user_email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        renamed_col2.renamed_from = Some("nonexistent2".to_string());

        table2.add_column(renamed_col1);
        table2.add_column(renamed_col2);
        schema2.add_table(table2);

        let (_, warnings) = service.detect_diff_with_warnings(&schema1, &schema2);

        // 2つの警告が生成される
        assert_eq!(warnings.len(), 2);
        assert!(warnings
            .iter()
            .all(|w| w.kind == WarningKind::OldColumnNotFound));
    }
}
