use serde::{Deserialize, Serialize};

/// 破壊的変更レポート
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestructiveChangeReport {
    /// 削除されるテーブル名のリスト
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tables_dropped: Vec<String>,

    /// 削除されるカラム（テーブルごと）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns_dropped: Vec<DroppedColumn>,

    /// リネームされるカラム
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns_renamed: Vec<RenamedColumnInfo>,

    /// 削除されるENUM
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enums_dropped: Vec<String>,

    /// 再作成されるENUM
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enums_recreated: Vec<String>,
}

/// 削除されるカラム情報
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroppedColumn {
    pub table: String,
    pub columns: Vec<String>,
}

/// リネームされるカラム情報
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenamedColumnInfo {
    pub table: String,
    pub old_name: String,
    pub new_name: String,
}

impl Default for DestructiveChangeReport {
    fn default() -> Self {
        Self::new()
    }
}

impl DestructiveChangeReport {
    /// 新しい空のレポートを作成
    pub fn new() -> Self {
        Self {
            tables_dropped: Vec::new(),
            columns_dropped: Vec::new(),
            columns_renamed: Vec::new(),
            enums_dropped: Vec::new(),
            enums_recreated: Vec::new(),
        }
    }

    /// 破壊的変更が含まれているかを判定
    pub fn has_destructive_changes(&self) -> bool {
        !self.tables_dropped.is_empty()
            || !self.columns_dropped.is_empty()
            || !self.columns_renamed.is_empty()
            || !self.enums_dropped.is_empty()
            || !self.enums_recreated.is_empty()
    }

    /// 破壊的変更の総数をカウント
    pub fn total_change_count(&self) -> usize {
        let dropped_column_count: usize = self
            .columns_dropped
            .iter()
            .map(|entry| entry.columns.len())
            .sum();

        self.tables_dropped.len()
            + dropped_column_count
            + self.columns_renamed.len()
            + self.enums_dropped.len()
            + self.enums_recreated.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{DestructiveChangeReport, DroppedColumn, RenamedColumnInfo};

    #[test]
    fn new_report_is_empty() {
        let report = DestructiveChangeReport::new();
        assert!(!report.has_destructive_changes());
        assert_eq!(report.total_change_count(), 0);
    }

    #[test]
    fn has_destructive_changes_when_any_field_present() {
        let report = DestructiveChangeReport {
            tables_dropped: vec!["old_users".to_string()],
            columns_dropped: Vec::new(),
            columns_renamed: Vec::new(),
            enums_dropped: Vec::new(),
            enums_recreated: Vec::new(),
        };

        assert!(report.has_destructive_changes());
    }

    #[test]
    fn total_change_count_counts_each_item() {
        let report = DestructiveChangeReport {
            tables_dropped: vec!["old_users".to_string(), "posts".to_string()],
            columns_dropped: vec![
                DroppedColumn {
                    table: "products".to_string(),
                    columns: vec!["legacy_field".to_string(), "unused_column".to_string()],
                },
                DroppedColumn {
                    table: "items".to_string(),
                    columns: vec!["deprecated".to_string()],
                },
            ],
            columns_renamed: vec![
                RenamedColumnInfo {
                    table: "orders".to_string(),
                    old_name: "old_id".to_string(),
                    new_name: "order_id".to_string(),
                },
                RenamedColumnInfo {
                    table: "orders".to_string(),
                    old_name: "old_status".to_string(),
                    new_name: "status".to_string(),
                },
            ],
            enums_dropped: vec!["old_status".to_string()],
            enums_recreated: vec!["priority".to_string()],
        };

        assert_eq!(report.total_change_count(), 2 + 3 + 2 + 1 + 1);
    }

    #[test]
    fn report_round_trips_yaml() {
        let report = DestructiveChangeReport {
            tables_dropped: vec!["old_users".to_string()],
            columns_dropped: vec![DroppedColumn {
                table: "products".to_string(),
                columns: vec!["legacy_field".to_string()],
            }],
            columns_renamed: vec![RenamedColumnInfo {
                table: "orders".to_string(),
                old_name: "old_id".to_string(),
                new_name: "order_id".to_string(),
            }],
            enums_dropped: vec!["old_status".to_string()],
            enums_recreated: vec!["priority".to_string()],
        };

        let yaml = serde_saphyr::to_string(&report).expect("serialize report");
        let parsed: DestructiveChangeReport =
            serde_saphyr::from_str(&yaml).expect("deserialize report");

        assert_eq!(parsed, report);
    }

    #[test]
    fn empty_fields_are_omitted_in_yaml() {
        let report = DestructiveChangeReport::new();
        let yaml = serde_saphyr::to_string(&report).expect("serialize report");

        assert!(!yaml.contains("tables_dropped"));
        assert!(!yaml.contains("columns_dropped"));
        assert!(!yaml.contains("columns_renamed"));
        assert!(!yaml.contains("enums_dropped"));
        assert!(!yaml.contains("enums_recreated"));
    }
}
