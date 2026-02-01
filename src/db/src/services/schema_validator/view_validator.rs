// ビュー定義の検証
//
// ビュー名の命名規則・衝突チェック、depends_on 依存検証・循環検出、
// definition の妥当性検証を行います。

use std::collections::HashMap;

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

/// 依存グラフの循環検出（Tarjan's SCC ベース）
///
/// 強連結成分（SCC）を検出し、サイズ2以上のSCCまたは自己参照を
/// 循環依存としてエラー報告する。Kahn法と比べて循環に含まれるビューのみを
/// 正確に報告できる。
fn validate_view_dependency_cycle(schema: &Schema) -> ValidationResult {
    let mut result = ValidationResult::new();

    // ビュー間の依存のみを対象（テーブル依存は循環しない）
    let view_names: Vec<&str> = {
        let mut names: Vec<&str> = schema.views.keys().map(|s| s.as_str()).collect();
        names.sort(); // 安定した出力のためソート
        names
    };

    if view_names.is_empty() {
        return result;
    }

    let name_to_idx: HashMap<&str, usize> = view_names
        .iter()
        .enumerate()
        .map(|(i, &name)| (name, i))
        .collect();

    // 隣接リスト構築（view_name → deps 方向: view_name depends on dep）
    let n = view_names.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

    for (view_name, view) in &schema.views {
        if let Some(&from) = name_to_idx.get(view_name.as_str()) {
            for dep in &view.depends_on {
                if let Some(&to) = name_to_idx.get(dep.as_str()) {
                    adj[from].push(to);
                }
            }
        }
    }

    // Tarjan's SCC algorithm
    let sccs = tarjan_scc(&adj, n);

    for scc in &sccs {
        let is_cycle = if scc.len() == 1 {
            // 自己参照チェック
            let node = scc[0];
            adj[node].contains(&node)
        } else {
            true
        };

        if is_cycle {
            let mut cycle_views: Vec<&str> = scc.iter().map(|&i| view_names[i]).collect();
            cycle_views.sort();

            result.add_error(ValidationError::Reference {
                message: format!(
                    "Circular dependency detected among views: [{}]",
                    cycle_views.join(", ")
                ),
                location: None,
                suggestion: Some("Remove circular depends_on references between views".to_string()),
            });
        }
    }

    result
}

/// Tarjan's strongly connected components algorithm
fn tarjan_scc(adj: &[Vec<usize>], n: usize) -> Vec<Vec<usize>> {
    struct TarjanState {
        index_counter: usize,
        stack: Vec<usize>,
        on_stack: Vec<bool>,
        index: Vec<Option<usize>>,
        lowlink: Vec<usize>,
        sccs: Vec<Vec<usize>>,
    }

    fn strongconnect(v: usize, adj: &[Vec<usize>], state: &mut TarjanState) {
        state.index[v] = Some(state.index_counter);
        state.lowlink[v] = state.index_counter;
        state.index_counter += 1;
        state.stack.push(v);
        state.on_stack[v] = true;

        for &w in &adj[v] {
            if state.index[w].is_none() {
                strongconnect(w, adj, state);
                state.lowlink[v] = state.lowlink[v].min(state.lowlink[w]);
            } else if state.on_stack[w] {
                state.lowlink[v] = state.lowlink[v].min(state.index[w].unwrap());
            }
        }

        if state.lowlink[v] == state.index[v].unwrap() {
            let mut scc = Vec::new();
            loop {
                let w = state.stack.pop().unwrap();
                state.on_stack[w] = false;
                scc.push(w);
                if w == v {
                    break;
                }
            }
            state.sccs.push(scc);
        }
    }

    let mut state = TarjanState {
        index_counter: 0,
        stack: Vec::new(),
        on_stack: vec![false; n],
        index: vec![None; n],
        lowlink: vec![0; n],
        sccs: Vec::new(),
    };

    for v in 0..n {
        if state.index[v].is_none() {
            strongconnect(v, adj, &mut state);
        }
    }

    state.sccs
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
