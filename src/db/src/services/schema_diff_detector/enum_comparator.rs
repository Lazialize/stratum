// ENUM差分検出

use crate::core::schema::{EnumDefinition, Schema};
use crate::core::schema_diff::{EnumChangeKind, EnumColumnRef, EnumDiff, SchemaDiff};
use std::collections::HashSet;

use super::SchemaDiffDetector;

impl SchemaDiffDetector {
    pub(crate) fn detect_enum_diff(
        &self,
        old_schema: &Schema,
        new_schema: &Schema,
        diff: &mut SchemaDiff,
    ) {
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
}

#[cfg(test)]
mod tests {
    use crate::core::schema::{EnumDefinition, Schema};
    use crate::services::schema_diff_detector::SchemaDiffDetector;

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
}
