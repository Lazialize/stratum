use crate::core::destructive_change_report::{
    DestructiveChangeReport, DroppedColumn, RenamedColumnInfo,
};
use crate::core::schema_diff::{EnumChangeKind, SchemaDiff};

/// 破壊的変更の検出サービス
#[derive(Debug, Default)]
pub struct DestructiveChangeDetector;

impl DestructiveChangeDetector {
    /// 新しいDetectorを作成
    pub fn new() -> Self {
        Self
    }

    /// スキーマ差分から破壊的変更を検出
    pub fn detect(&self, schema_diff: &SchemaDiff) -> DestructiveChangeReport {
        let mut report = DestructiveChangeReport::new();

        report.tables_dropped = schema_diff.removed_tables.clone();
        report.enums_dropped = schema_diff.removed_enums.clone();
        report.views_dropped = schema_diff.removed_views.clone();
        report.views_modified = schema_diff
            .modified_views
            .iter()
            .map(|v| v.view_name.clone())
            .collect();

        for table_diff in &schema_diff.modified_tables {
            if !table_diff.removed_columns.is_empty() {
                report.columns_dropped.push(DroppedColumn {
                    table: table_diff.table_name.clone(),
                    columns: table_diff.removed_columns.clone(),
                });
            }

            for renamed in &table_diff.renamed_columns {
                report.columns_renamed.push(RenamedColumnInfo {
                    table: table_diff.table_name.clone(),
                    old_name: renamed.old_name.clone(),
                    new_name: renamed.new_column.name.clone(),
                });
            }
        }

        for enum_diff in &schema_diff.modified_enums {
            if matches!(enum_diff.change_kind, EnumChangeKind::Recreate) {
                report.enums_recreated.push(enum_diff.enum_name.clone());
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::DestructiveChangeDetector;
    use crate::core::destructive_change_report::{
        DestructiveChangeReport, DroppedColumn, RenamedColumnInfo,
    };
    use crate::core::schema::{Column, ColumnType};
    use crate::core::schema_diff::{
        EnumChangeKind, EnumColumnRef, EnumDiff, RenamedColumn, SchemaDiff, TableDiff,
    };

    fn integer_column(name: &str) -> Column {
        Column::new(
            name.to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        )
    }

    #[test]
    fn detect_empty_diff_returns_empty_report() {
        let detector = DestructiveChangeDetector::new();
        let diff = SchemaDiff::new();

        let report = detector.detect(&diff);

        assert_eq!(report, DestructiveChangeReport::new());
    }

    #[test]
    fn detect_all_destructive_change_types() {
        let detector = DestructiveChangeDetector::new();
        let mut diff = SchemaDiff::new();

        diff.removed_tables = vec!["old_users".to_string()];
        diff.removed_enums = vec!["old_status".to_string()];

        let mut table_diff = TableDiff::new("products".to_string());
        table_diff.removed_columns = vec!["legacy_field".to_string()];
        table_diff.renamed_columns = vec![RenamedColumn {
            old_name: "old_name".to_string(),
            old_column: integer_column("old_name"),
            new_column: integer_column("new_name"),
            changes: Vec::new(),
        }];
        diff.modified_tables.push(table_diff);

        diff.modified_enums.push(EnumDiff {
            enum_name: "priority".to_string(),
            old_values: vec!["low".to_string()],
            new_values: vec!["low".to_string(), "high".to_string()],
            added_values: vec!["high".to_string()],
            removed_values: Vec::new(),
            change_kind: EnumChangeKind::Recreate,
            columns: vec![EnumColumnRef {
                table_name: "tasks".to_string(),
                column_name: "priority".to_string(),
            }],
        });

        let report = detector.detect(&diff);

        assert_eq!(report.tables_dropped, vec!["old_users".to_string()]);
        assert_eq!(report.enums_dropped, vec!["old_status".to_string()]);
        assert_eq!(
            report.columns_dropped,
            vec![DroppedColumn {
                table: "products".to_string(),
                columns: vec!["legacy_field".to_string()],
            }]
        );
        assert_eq!(
            report.columns_renamed,
            vec![RenamedColumnInfo {
                table: "products".to_string(),
                old_name: "old_name".to_string(),
                new_name: "new_name".to_string(),
            }]
        );
        assert_eq!(report.enums_recreated, vec!["priority".to_string()]);
    }

    #[test]
    fn detect_multiple_tables_and_columns() {
        let detector = DestructiveChangeDetector::new();
        let mut diff = SchemaDiff::new();

        let mut table_one = TableDiff::new("products".to_string());
        table_one.removed_columns = vec!["legacy_field".to_string(), "unused".to_string()];

        let mut table_two = TableDiff::new("orders".to_string());
        table_two.removed_columns = vec!["old_status".to_string()];

        diff.modified_tables.push(table_one);
        diff.modified_tables.push(table_two);

        let report = detector.detect(&diff);

        assert_eq!(
            report.columns_dropped,
            vec![
                DroppedColumn {
                    table: "products".to_string(),
                    columns: vec!["legacy_field".to_string(), "unused".to_string()],
                },
                DroppedColumn {
                    table: "orders".to_string(),
                    columns: vec!["old_status".to_string()],
                },
            ]
        );
    }

    #[test]
    fn detect_is_idempotent() {
        let detector = DestructiveChangeDetector::new();
        let mut diff = SchemaDiff::new();
        diff.removed_tables = vec!["old_users".to_string()];

        let first = detector.detect(&diff);
        let second = detector.detect(&diff);

        assert_eq!(first, second);
    }
}
