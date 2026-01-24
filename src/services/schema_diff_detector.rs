// スキーマ差分検出サービス
//
// 2つのスキーマ間の差分を検出するサービス。
// テーブル、カラム、インデックス、制約の追加、削除、変更を検出します。

use crate::core::error::{ErrorLocation, ValidationWarning, WarningKind};
use crate::core::schema::{EnumDefinition, Schema};
use crate::core::schema_diff::{
    ColumnChange, ColumnDiff, EnumChangeKind, EnumColumnRef, EnumDiff, RenamedColumn, SchemaDiff,
    TableDiff,
};
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

        diff.enum_recreate_allowed = new_schema.enum_recreate_allowed;

        self.detect_enum_diff(old_schema, new_schema, &mut diff);

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

    fn detect_enum_diff(&self, old_schema: &Schema, new_schema: &Schema, diff: &mut SchemaDiff) {
        let old_enum_names: HashSet<&String> = old_schema.enums.keys().collect();
        let new_enum_names: HashSet<&String> = new_schema.enums.keys().collect();

        for enum_name in new_enum_names.difference(&old_enum_names) {
            if let Some(enum_def) = new_schema.enums.get(*enum_name) {
                diff.added_enums.push(enum_def.clone());
            }
        }

        for enum_name in old_enum_names.difference(&new_enum_names) {
            diff.removed_enums.push((*enum_name).clone());
        }

        for enum_name in old_enum_names.intersection(&new_enum_names) {
            let old_enum = old_schema.enums.get(*enum_name).unwrap();
            let new_enum = new_schema.enums.get(*enum_name).unwrap();
            if old_enum.values != new_enum.values {
                let enum_diff = self.build_enum_diff(old_enum, new_enum, new_schema);
                diff.modified_enums.push(enum_diff);
            }
        }
    }

    fn build_enum_diff(
        &self,
        old_enum: &EnumDefinition,
        new_enum: &EnumDefinition,
        schema: &Schema,
    ) -> EnumDiff {
        let old_set: HashSet<&String> = old_enum.values.iter().collect();
        let new_set: HashSet<&String> = new_enum.values.iter().collect();

        let added_values: Vec<String> = new_enum
            .values
            .iter()
            .filter(|v| !old_set.contains(*v))
            .cloned()
            .collect();
        let removed_values: Vec<String> = old_enum
            .values
            .iter()
            .filter(|v| !new_set.contains(*v))
            .cloned()
            .collect();

        let is_subsequence = {
            let mut idx = 0usize;
            for value in &new_enum.values {
                if idx < old_enum.values.len() && value == &old_enum.values[idx] {
                    idx += 1;
                }
            }
            idx == old_enum.values.len()
        };

        let change_kind = if removed_values.is_empty() && is_subsequence {
            EnumChangeKind::AddOnly
        } else {
            EnumChangeKind::Recreate
        };

        let columns = Self::collect_enum_columns(schema, &new_enum.name);

        EnumDiff {
            enum_name: old_enum.name.clone(),
            old_values: old_enum.values.clone(),
            new_values: new_enum.values.clone(),
            added_values,
            removed_values,
            change_kind,
            columns,
        }
    }

    fn collect_enum_columns(schema: &Schema, enum_name: &str) -> Vec<EnumColumnRef> {
        let mut refs = Vec::new();
        for (table_name, table) in &schema.tables {
            for column in &table.columns {
                if let crate::core::schema::ColumnType::Enum { name } = &column.column_type {
                    if name == enum_name {
                        refs.push(EnumColumnRef {
                            table_name: table_name.clone(),
                            column_name: column.name.clone(),
                        });
                    }
                }
            }
        }
        refs
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

    /// テーブル差分を検出（警告付き）
    fn detect_table_diff_with_warnings(
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

        // リネームされたカラムを検出し、対象の旧カラム名を収集
        let mut renamed_old_names: HashSet<String> = HashSet::new();
        let mut renamed_new_names: HashSet<String> = HashSet::new();

        for new_column in &new_table.columns {
            if let Some(ref old_name) = new_column.renamed_from {
                // 旧テーブルに該当カラムが存在するか確認
                if let Some(old_column) = old_table.columns.iter().find(|c| &c.name == old_name) {
                    // リネームとして検出
                    let changes = self.detect_column_changes(old_column, new_column);
                    table_diff.renamed_columns.push(RenamedColumn {
                        old_name: old_name.clone(),
                        old_column: old_column.clone(),
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
            if let Some(column) = new_table.columns.iter().find(|c| &c.name == *column_name) {
                table_diff.added_columns.push(column.clone());
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

        // 変更されたカラム
        for column_name in old_column_names.intersection(&new_column_names) {
            if let (Some(old_column), Some(new_column)) = (
                old_table.columns.iter().find(|c| &c.name == *column_name),
                new_table.columns.iter().find(|c| &c.name == *column_name),
            ) {
                // カラムの定義が変更されているか確認
                if old_column != new_column {
                    let column_diff = ColumnDiff::new(
                        (*column_name).clone(),
                        old_column.clone(),
                        new_column.clone(),
                    );
                    if !column_diff.changes.is_empty() {
                        table_diff.modified_columns.push(column_diff);
                    }
                }
            }
        }
    }

    /// カラム差分を検出（警告付き）
    fn detect_column_diff_with_warnings(
        &self,
        old_table: &crate::core::schema::Table,
        new_table: &crate::core::schema::Table,
        table_diff: &mut TableDiff,
        warnings: &mut Vec<ValidationWarning>,
    ) {
        let old_column_names: HashSet<&String> =
            old_table.columns.iter().map(|c| &c.name).collect();
        let new_column_names: HashSet<&String> =
            new_table.columns.iter().map(|c| &c.name).collect();

        // リネームされたカラムを検出し、対象の旧カラム名を収集
        let mut renamed_old_names: HashSet<String> = HashSet::new();
        let mut renamed_new_names: HashSet<String> = HashSet::new();

        for new_column in &new_table.columns {
            if let Some(ref old_name) = new_column.renamed_from {
                // 旧テーブルに該当カラムが存在するか確認
                if let Some(old_column) = old_table.columns.iter().find(|c| &c.name == old_name) {
                    // リネームとして検出
                    let changes = self.detect_column_changes(old_column, new_column);
                    table_diff.renamed_columns.push(RenamedColumn {
                        old_name: old_name.clone(),
                        old_column: old_column.clone(),
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
                        Some(ErrorLocation {
                            table: Some(old_table.name.clone()),
                            column: Some(new_column.name.clone()),
                            line: None,
                        }),
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
            if let Some(column) = new_table.columns.iter().find(|c| &c.name == *column_name) {
                table_diff.added_columns.push(column.clone());
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

        // 変更されたカラム
        for column_name in old_column_names.intersection(&new_column_names) {
            if let (Some(old_column), Some(new_column)) = (
                old_table.columns.iter().find(|c| &c.name == *column_name),
                new_table.columns.iter().find(|c| &c.name == *column_name),
            ) {
                // カラムの定義が変更されているか確認
                if old_column != new_column {
                    let column_diff = ColumnDiff::new(
                        (*column_name).clone(),
                        old_column.clone(),
                        new_column.clone(),
                    );
                    if !column_diff.changes.is_empty() {
                        table_diff.modified_columns.push(column_diff);
                    }
                }
            }
        }
    }

    /// カラム間の変更を検出
    fn detect_column_changes(
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
    use crate::core::schema::{Column, ColumnType, EnumDefinition, Table};

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

    #[test]
    fn test_detect_enum_added() {
        let service = SchemaDiffDetector::new();
        let schema1 = Schema::new("1.0".to_string());

        let mut schema2 = Schema::new("1.0".to_string());
        schema2.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string()],
        });

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.added_enums.len(), 1);
        assert_eq!(diff.added_enums[0].name, "status");
    }

    #[test]
    fn test_detect_enum_removed() {
        let service = SchemaDiffDetector::new();
        let mut schema1 = Schema::new("1.0".to_string());
        schema1.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string()],
        });

        let schema2 = Schema::new("1.0".to_string());

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.removed_enums.len(), 1);
        assert_eq!(diff.removed_enums[0], "status");
    }

    #[test]
    fn test_detect_enum_add_only_change() {
        let service = SchemaDiffDetector::new();
        let mut schema1 = Schema::new("1.0".to_string());
        schema1.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string()],
        });

        let mut schema2 = Schema::new("1.0".to_string());
        schema2.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_enums.len(), 1);
        assert!(matches!(
            diff.modified_enums[0].change_kind,
            crate::core::schema_diff::EnumChangeKind::AddOnly
        ));
    }

    #[test]
    fn test_detect_enum_recreate_change() {
        let service = SchemaDiffDetector::new();
        let mut schema1 = Schema::new("1.0".to_string());
        schema1.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        let mut schema2 = Schema::new("1.0".to_string());
        schema2.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["inactive".to_string(), "active".to_string()],
        });

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_enums.len(), 1);
        assert!(matches!(
            diff.modified_enums[0].change_kind,
            crate::core::schema_diff::EnumChangeKind::Recreate
        ));
    }

    #[test]
    fn test_detect_enum_recreate_opt_in_flag() {
        let service = SchemaDiffDetector::new();
        let schema1 = Schema::new("1.0".to_string());

        let mut schema2 = Schema::new("1.0".to_string());
        schema2.enum_recreate_allowed = true;

        let diff = service.detect_diff(&schema1, &schema2);

        assert!(diff.enum_recreate_allowed);
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
