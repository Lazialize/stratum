use super::GenerateCommandHandler;

impl GenerateCommandHandler {
    /// 差分から変更サマリを生成
    pub(super) fn format_change_summary(
        &self,
        diff: &crate::core::schema_diff::SchemaDiff,
        verbose: bool,
    ) -> String {
        let mut lines = Vec::new();

        for table in &diff.added_tables {
            lines.push(format!("  + ADD TABLE {}", table.name));
            if verbose {
                for col in &table.columns {
                    let nullable = if col.nullable { "NULL" } else { "NOT NULL" };
                    lines.push(format!(
                        "      {} {:?} {}",
                        col.name, col.column_type, nullable
                    ));
                }
            }
        }

        for table_name in &diff.removed_tables {
            lines.push(format!("  - DROP TABLE {}", table_name));
        }

        for table_diff in &diff.modified_tables {
            for col in &table_diff.added_columns {
                lines.push(format!(
                    "  + ADD COLUMN {}.{}",
                    table_diff.table_name, col.name
                ));
            }
            for col_name in &table_diff.removed_columns {
                lines.push(format!(
                    "  - DROP COLUMN {}.{}",
                    table_diff.table_name, col_name
                ));
            }
            for col_diff in &table_diff.modified_columns {
                lines.push(format!(
                    "  ~ MODIFY COLUMN {}.{}",
                    table_diff.table_name, col_diff.column_name
                ));
            }
            for renamed in &table_diff.renamed_columns {
                lines.push(format!(
                    "  ~ RENAME COLUMN {}.{} -> {}",
                    table_diff.table_name, renamed.old_name, renamed.new_column.name
                ));
            }
            for idx in &table_diff.added_indexes {
                lines.push(format!(
                    "  + ADD INDEX {} ON {}",
                    idx.name, table_diff.table_name
                ));
            }
            for idx_name in &table_diff.removed_indexes {
                lines.push(format!(
                    "  - DROP INDEX {} ON {}",
                    idx_name, table_diff.table_name
                ));
            }
            for constraint in &table_diff.added_constraints {
                lines.push(format!(
                    "  + ADD {} ON {}",
                    constraint.kind(),
                    table_diff.table_name
                ));
            }
            for constraint in &table_diff.removed_constraints {
                lines.push(format!(
                    "  - DROP {} ON {}",
                    constraint.kind(),
                    table_diff.table_name
                ));
            }
        }

        for enum_def in &diff.added_enums {
            lines.push(format!("  + ADD ENUM {}", enum_def.name));
        }

        for enum_name in &diff.removed_enums {
            lines.push(format!("  - DROP ENUM {}", enum_name));
        }

        lines.join("\n")
    }

    /// 差分から自動的にdescriptionを生成
    pub(super) fn generate_auto_description(
        &self,
        diff: &crate::core::schema_diff::SchemaDiff,
    ) -> String {
        let mut parts = Vec::new();

        if !diff.added_tables.is_empty() {
            let table_names: Vec<&str> =
                diff.added_tables.iter().map(|t| t.name.as_str()).collect();
            parts.push(format!("add tables {}", table_names.join(", ")));
        }

        if !diff.removed_tables.is_empty() {
            let removed_names: Vec<&str> = diff.removed_tables.iter().map(|s| s.as_str()).collect();
            parts.push(format!("remove tables {}", removed_names.join(", ")));
        }

        if !diff.modified_tables.is_empty() {
            let table_names: Vec<&str> = diff
                .modified_tables
                .iter()
                .map(|t| t.table_name.as_str())
                .collect();
            parts.push(format!("modify tables {}", table_names.join(", ")));
        }

        if parts.is_empty() {
            "schema changes".to_string()
        } else {
            parts.join(" and ")
        }
    }
}
