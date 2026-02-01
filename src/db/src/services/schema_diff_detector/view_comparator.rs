// ビュー差分検出
//
// 2つのスキーマ間のビュー差分を検出します。
// ビューの追加、削除、変更、リネームを検出し、
// definition の正規化比較を行います。

use crate::core::schema::Schema;
use crate::core::schema_diff::{RenamedView, SchemaDiff, ViewDiff};
use std::collections::HashSet;

/// ビュー定義の正規化
///
/// 空白・改行・連続スペースの差異のみを除去する最小ルール。
/// SQL 意味の同一性判定ではなく、表面的な空白差異を無視する。
pub fn normalize_definition(definition: &str) -> String {
    definition.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// ビュー差分の検出
pub fn detect_view_diff(old_schema: &Schema, new_schema: &Schema, diff: &mut SchemaDiff) {
    let old_view_names: HashSet<&String> = old_schema.views.keys().collect();
    let new_view_names: HashSet<&String> = new_schema.views.keys().collect();

    // リネームされたビューの旧名を追跡
    let mut renamed_old_names: HashSet<String> = HashSet::new();

    // 追加されたビュー（リネームを含む可能性）
    for view_name in new_view_names.difference(&old_view_names) {
        if let Some(view) = new_schema.views.get(*view_name) {
            if let Some(ref old_name) = view.renamed_from {
                if old_schema.views.contains_key(old_name) {
                    diff.renamed_views.push(RenamedView {
                        old_name: old_name.clone(),
                        new_view: view.clone(),
                    });
                    renamed_old_names.insert(old_name.clone());
                    continue;
                }
            }
            diff.added_views.push(view.clone());
        }
    }

    // 削除されたビュー（リネームされたものを除外）
    for view_name in old_view_names.difference(&new_view_names) {
        if !renamed_old_names.contains(*view_name) {
            diff.removed_views.push((*view_name).clone());
        }
    }

    // 変更されたビュー（definition の正規化比較）
    for view_name in old_view_names.intersection(&new_view_names) {
        if let (Some(old_view), Some(new_view)) = (
            old_schema.views.get(*view_name),
            new_schema.views.get(*view_name),
        ) {
            let old_normalized = normalize_definition(&old_view.definition);
            let new_normalized = normalize_definition(&new_view.definition);

            if old_normalized != new_normalized {
                diff.modified_views.push(ViewDiff {
                    view_name: (*view_name).clone(),
                    old_definition: old_view.definition.clone(),
                    new_definition: new_view.definition.clone(),
                    old_view: old_view.clone(),
                    new_view: new_view.clone(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::View;

    // ===== Task 3.2: definition 正規化比較 =====

    #[test]
    fn test_normalize_definition_trims_whitespace() {
        assert_eq!(
            normalize_definition("  SELECT  *   FROM  users  "),
            "SELECT * FROM users"
        );
    }

    #[test]
    fn test_normalize_definition_collapses_newlines() {
        assert_eq!(
            normalize_definition("SELECT *\nFROM users\nWHERE active = true"),
            "SELECT * FROM users WHERE active = true"
        );
    }

    #[test]
    fn test_normalize_definition_tabs_and_mixed() {
        assert_eq!(
            normalize_definition("SELECT\t*\n  FROM\tusers"),
            "SELECT * FROM users"
        );
    }

    #[test]
    fn test_normalize_definition_identical() {
        let def = "SELECT * FROM users";
        assert_eq!(normalize_definition(def), def);
    }

    // ===== Task 3.1: 追加/更新/削除/rename の差分抽出 =====

    #[test]
    fn test_detect_view_added() {
        let old = Schema::new("1.0".to_string());
        let mut new = Schema::new("1.0".to_string());
        new.add_view(View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        ));

        let mut diff = SchemaDiff::new();
        detect_view_diff(&old, &new, &mut diff);

        assert_eq!(diff.added_views.len(), 1);
        assert_eq!(diff.added_views[0].name, "active_users");
        assert!(diff.removed_views.is_empty());
        assert!(diff.modified_views.is_empty());
    }

    #[test]
    fn test_detect_view_removed() {
        let mut old = Schema::new("1.0".to_string());
        old.add_view(View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        ));
        let new = Schema::new("1.0".to_string());

        let mut diff = SchemaDiff::new();
        detect_view_diff(&old, &new, &mut diff);

        assert!(diff.added_views.is_empty());
        assert_eq!(diff.removed_views.len(), 1);
        assert_eq!(diff.removed_views[0], "active_users");
    }

    #[test]
    fn test_detect_view_modified() {
        let mut old = Schema::new("1.0".to_string());
        old.add_view(View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        ));

        let mut new = Schema::new("1.0".to_string());
        new.add_view(View::new(
            "active_users".to_string(),
            "SELECT id, email FROM users WHERE active = true".to_string(),
        ));

        let mut diff = SchemaDiff::new();
        detect_view_diff(&old, &new, &mut diff);

        assert!(diff.added_views.is_empty());
        assert!(diff.removed_views.is_empty());
        assert_eq!(diff.modified_views.len(), 1);
        assert_eq!(diff.modified_views[0].view_name, "active_users");
        assert_eq!(
            diff.modified_views[0].old_definition,
            "SELECT * FROM users WHERE active = true"
        );
    }

    #[test]
    fn test_detect_view_not_modified_whitespace_only() {
        let mut old = Schema::new("1.0".to_string());
        old.add_view(View::new(
            "active_users".to_string(),
            "SELECT *  FROM  users  WHERE active = true".to_string(),
        ));

        let mut new = Schema::new("1.0".to_string());
        new.add_view(View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        ));

        let mut diff = SchemaDiff::new();
        detect_view_diff(&old, &new, &mut diff);

        assert!(diff.added_views.is_empty());
        assert!(diff.removed_views.is_empty());
        assert!(diff.modified_views.is_empty());
    }

    #[test]
    fn test_detect_view_renamed() {
        let mut old = Schema::new("1.0".to_string());
        old.add_view(View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        ));

        let mut new = Schema::new("1.0".to_string());
        let mut renamed_view = View::new(
            "enabled_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        );
        renamed_view.renamed_from = Some("active_users".to_string());
        new.add_view(renamed_view);

        let mut diff = SchemaDiff::new();
        detect_view_diff(&old, &new, &mut diff);

        assert!(diff.added_views.is_empty());
        assert!(diff.removed_views.is_empty());
        assert!(diff.modified_views.is_empty());
        assert_eq!(diff.renamed_views.len(), 1);
        assert_eq!(diff.renamed_views[0].old_name, "active_users");
        assert_eq!(diff.renamed_views[0].new_view.name, "enabled_users");
    }

    #[test]
    fn test_detect_view_renamed_nonexistent_old() {
        let old = Schema::new("1.0".to_string());

        let mut new = Schema::new("1.0".to_string());
        let mut view = View::new(
            "enabled_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        );
        view.renamed_from = Some("nonexistent".to_string());
        new.add_view(view);

        let mut diff = SchemaDiff::new();
        detect_view_diff(&old, &new, &mut diff);

        // Treated as added since old name doesn't exist
        assert_eq!(diff.added_views.len(), 1);
        assert!(diff.renamed_views.is_empty());
    }

    #[test]
    fn test_detect_no_view_changes() {
        let mut old = Schema::new("1.0".to_string());
        old.add_view(View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        ));

        let mut new = Schema::new("1.0".to_string());
        new.add_view(View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        ));

        let mut diff = SchemaDiff::new();
        detect_view_diff(&old, &new, &mut diff);

        assert!(diff.is_empty());
    }

    #[test]
    fn test_detect_multiple_view_changes() {
        let mut old = Schema::new("1.0".to_string());
        old.add_view(View::new("view_a".to_string(), "SELECT 1".to_string()));
        old.add_view(View::new("view_b".to_string(), "SELECT 2".to_string()));

        let mut new = Schema::new("1.0".to_string());
        // view_a modified
        new.add_view(View::new("view_a".to_string(), "SELECT 1, 2".to_string()));
        // view_b removed (not in new)
        // view_c added
        new.add_view(View::new("view_c".to_string(), "SELECT 3".to_string()));

        let mut diff = SchemaDiff::new();
        detect_view_diff(&old, &new, &mut diff);

        assert_eq!(diff.added_views.len(), 1);
        assert_eq!(diff.removed_views.len(), 1);
        assert_eq!(diff.modified_views.len(), 1);
    }
}
