// ビュー定義の検証
//
// ビュー名の命名規則・衝突チェック、depends_on 依存検証・循環検出、
// definition の妥当性検証を行います。

use std::collections::{HashMap, HashSet};

use crate::core::error::{ErrorLocation, ValidationError, ValidationResult};
use crate::core::schema::Schema;

/// ビュー定義の検証
///
/// - ビュー名がテーブル名と衝突していないか確認
/// - ビュー名が重複していないか確認（BTreeMap のキーと name フィールドの一致含む）
/// - definition が空でないか確認
/// - depends_on の参照先が tables/views に存在するか検証
/// - 依存グラフの循環を検出
pub fn validate_views(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    if schema.views.is_empty() {
        return result;
    }

    result.merge(validate_view_name_collisions(schema));
    result.merge(validate_view_definitions(schema));
    result.merge(validate_view_depends_on(schema));
    result.merge(validate_view_dependency_cycle(schema));

    result
}

/// ビュー名とテーブル名の衝突チェック
fn validate_view_name_collisions(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    for view_name in schema.views.keys() {
        if schema.tables.contains_key(view_name) {
            result.add_error(ValidationError::Constraint {
                message: format!(
                    "View '{}' conflicts with an existing table of the same name",
                    view_name
                ),
                location: Some(ErrorLocation::with_view(view_name)),
                suggestion: Some("Use a different name for the view or the table".to_string()),
            });
        }
    }

    result
}

/// definition の妥当性検証
fn validate_view_definitions(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (view_name, view) in &schema.views {
        if view.definition.trim().is_empty() {
            result.add_error(ValidationError::Constraint {
                message: format!("View '{}' has an empty definition", view_name),
                location: Some(ErrorLocation::with_view(view_name)),
                suggestion: Some(
                    "Provide a SQL SELECT statement as the view definition".to_string(),
                ),
            });
        }
    }

    result
}

/// depends_on の参照先存在検証
fn validate_view_depends_on(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (view_name, view) in &schema.views {
        for dep in &view.depends_on {
            if !schema.tables.contains_key(dep) && !schema.views.contains_key(dep) {
                result.add_error(ValidationError::Reference {
                    message: format!(
                        "View '{}' depends on '{}' which does not exist as a table or view",
                        view_name, dep
                    ),
                    location: Some(ErrorLocation::with_view(view_name)),
                    suggestion: Some(format!(
                        "Define '{}' as a table or view, or remove it from depends_on",
                        dep
                    )),
                });
            }
        }
    }

    result
}

/// 依存グラフの循環検出（トポロジカルソートベース）
fn validate_view_dependency_cycle(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    // ビュー間の依存のみを対象（テーブル依存は循環しない）
    let view_names: HashSet<&str> = schema.views.keys().map(|s| s.as_str()).collect();

    // 隣接リスト構築
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> = HashMap::new();

    for name in &view_names {
        adj.entry(name).or_default();
        in_degree.entry(name).or_insert(0);
    }

    for (view_name, view) in &schema.views {
        for dep in &view.depends_on {
            if view_names.contains(dep.as_str()) {
                adj.entry(dep.as_str())
                    .or_default()
                    .push(view_name.as_str());
                *in_degree.entry(view_name.as_str()).or_insert(0) += 1;
            }
        }
    }

    // Kahn's algorithm
    let mut queue: Vec<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&name, _)| name)
        .collect();
    queue.sort(); // 安定ソートのためソート
    let mut visited_count = 0;

    while let Some(node) = queue.pop() {
        visited_count += 1;
        if let Some(neighbors) = adj.get(node) {
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(neighbor);
                        queue.sort();
                    }
                }
            }
        }
    }

    if visited_count != view_names.len() {
        // 循環が存在する：循環に含まれるビューを特定
        let cycle_views: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &deg)| deg > 0)
            .map(|(&name, _)| name)
            .collect();

        result.add_error(ValidationError::Reference {
            message: format!(
                "Circular dependency detected among views: [{}]",
                cycle_views.join(", ")
            ),
            location: None,
            suggestion: Some("Remove circular depends_on references between views".to_string()),
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{Column, ColumnType, Constraint, Table, View};

    // ===== Task 2.1: 命名規則と衝突の検証 =====

    #[test]
    fn test_view_name_collides_with_table() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        schema.add_view(View::new(
            "users".to_string(),
            "SELECT * FROM users".to_string(),
        ));

        let result = validate_views(&schema);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("conflicts with an existing table")));
    }

    #[test]
    fn test_view_name_no_collision() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        schema.add_view(View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        ));

        let result = validate_views(&schema);
        assert!(result.is_valid());
    }

    // ===== Task 2.2: depends_on 依存検証と循環検出 =====

    #[test]
    fn test_depends_on_references_existing_table() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let mut view = View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        );
        view.depends_on = vec!["users".to_string()];
        schema.add_view(view);

        let result = validate_views(&schema);
        assert!(result.is_valid());
    }

    #[test]
    fn test_depends_on_references_nonexistent() {
        let mut schema = Schema::new("1.0".to_string());

        let mut view = View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        );
        view.depends_on = vec!["nonexistent_table".to_string()];
        schema.add_view(view);

        let result = validate_views(&schema);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("does not exist as a table or view")));
    }

    #[test]
    fn test_depends_on_references_existing_view() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let mut view1 = View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        );
        view1.depends_on = vec!["users".to_string()];
        schema.add_view(view1);

        let mut view2 = View::new(
            "recent_active_users".to_string(),
            "SELECT * FROM active_users WHERE created_at > NOW() - INTERVAL '7 days'".to_string(),
        );
        view2.depends_on = vec!["active_users".to_string()];
        schema.add_view(view2);

        let result = validate_views(&schema);
        assert!(result.is_valid());
    }

    #[test]
    fn test_circular_dependency_two_views() {
        let mut schema = Schema::new("1.0".to_string());

        let mut view_a = View::new("view_a".to_string(), "SELECT 1".to_string());
        view_a.depends_on = vec!["view_b".to_string()];
        schema.add_view(view_a);

        let mut view_b = View::new("view_b".to_string(), "SELECT 1".to_string());
        view_b.depends_on = vec!["view_a".to_string()];
        schema.add_view(view_b);

        let result = validate_views(&schema);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("Circular dependency")));
    }

    #[test]
    fn test_circular_dependency_three_views() {
        let mut schema = Schema::new("1.0".to_string());

        let mut view_a = View::new("view_a".to_string(), "SELECT 1".to_string());
        view_a.depends_on = vec!["view_c".to_string()];
        schema.add_view(view_a);

        let mut view_b = View::new("view_b".to_string(), "SELECT 1".to_string());
        view_b.depends_on = vec!["view_a".to_string()];
        schema.add_view(view_b);

        let mut view_c = View::new("view_c".to_string(), "SELECT 1".to_string());
        view_c.depends_on = vec!["view_b".to_string()];
        schema.add_view(view_c);

        let result = validate_views(&schema);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("Circular dependency")));
    }

    #[test]
    fn test_no_circular_dependency_chain() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("base".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let mut view_a = View::new("view_a".to_string(), "SELECT * FROM base".to_string());
        view_a.depends_on = vec!["base".to_string()];
        schema.add_view(view_a);

        let mut view_b = View::new("view_b".to_string(), "SELECT * FROM view_a".to_string());
        view_b.depends_on = vec!["view_a".to_string()];
        schema.add_view(view_b);

        let mut view_c = View::new("view_c".to_string(), "SELECT * FROM view_b".to_string());
        view_c.depends_on = vec!["view_b".to_string()];
        schema.add_view(view_c);

        let result = validate_views(&schema);
        assert!(result.is_valid());
    }

    // ===== Task 2.3: definition の妥当性検証 =====

    #[test]
    fn test_empty_definition() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_view(View::new("empty_view".to_string(), "".to_string()));

        let result = validate_views(&schema);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("empty definition")));
    }

    #[test]
    fn test_whitespace_only_definition() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_view(View::new("ws_view".to_string(), "   \n\t  ".to_string()));

        let result = validate_views(&schema);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("empty definition")));
    }

    #[test]
    fn test_valid_definition() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let mut view = View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
        );
        view.depends_on = vec!["users".to_string()];
        schema.add_view(view);

        let result = validate_views(&schema);
        assert!(result.is_valid());
    }

    // ===== Edge cases =====

    #[test]
    fn test_no_views_is_valid() {
        let schema = Schema::new("1.0".to_string());
        let result = validate_views(&schema);
        assert!(result.is_valid());
    }

    #[test]
    fn test_view_without_depends_on_is_valid() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_view(View::new(
            "standalone".to_string(),
            "SELECT 1 AS one".to_string(),
        ));

        let result = validate_views(&schema);
        assert!(result.is_valid());
    }

    #[test]
    fn test_self_referencing_view() {
        let mut schema = Schema::new("1.0".to_string());

        let mut view = View::new("self_ref".to_string(), "SELECT 1".to_string());
        view.depends_on = vec!["self_ref".to_string()];
        schema.add_view(view);

        let result = validate_views(&schema);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("Circular dependency")));
    }
}
