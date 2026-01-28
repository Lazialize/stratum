// スキーマチェックサム計算サービス
//
// スキーマ定義のSHA-256ハッシュ計算と比較を行うサービス。
// 正規化されたスキーマ表現を生成してチェックサムを計算します。

use crate::core::schema::Schema;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// スキーマチェックサムサービス
///
/// スキーマのSHA-256ハッシュ計算を行います。
#[derive(Debug, Clone)]
pub struct SchemaChecksumService {
    // 将来的な拡張のためのフィールドを予約
}

impl SchemaChecksumService {
    /// 新しいSchemaChecksumServiceを作成
    pub fn new() -> Self {
        Self {}
    }

    /// スキーマのチェックサムを計算
    ///
    /// # Arguments
    ///
    /// * `schema` - チェックサムを計算するスキーマ
    ///
    /// # Returns
    ///
    /// SHA-256ハッシュ（64文字の16進数文字列）
    pub fn calculate_checksum(&self, schema: &Schema) -> String {
        // スキーマを正規化
        let normalized = self.normalize_schema(schema);

        // SHA-256ハッシュを計算
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        let result = hasher.finalize();

        // 16進数文字列に変換
        format!("{:x}", result)
    }

    /// スキーマを正規化された文字列表現に変換
    ///
    /// テーブルやカラムの順序に依存しない一貫した表現を生成します。
    ///
    /// # Arguments
    ///
    /// * `schema` - 正規化するスキーマ
    ///
    /// # Returns
    ///
    /// 正規化されたJSON文字列
    pub fn normalize_schema(&self, schema: &Schema) -> String {
        // BTreeMapを使用してテーブルを名前でソート
        let mut normalized = BTreeMap::new();

        normalized.insert("version", schema.version.clone());

        // テーブルを名前順にソート
        let mut sorted_tables = BTreeMap::new();
        for (table_name, table) in &schema.tables {
            let mut table_data = BTreeMap::new();

            table_data.insert("name".to_string(), table.name.clone());

            // カラムを名前順にソート
            let mut sorted_columns = table
                .columns
                .iter()
                .map(|col| {
                    let mut col_data = BTreeMap::new();
                    col_data.insert("name".to_string(), col.name.clone());
                    col_data.insert("type".to_string(), format!("{:?}", col.column_type));
                    col_data.insert("nullable".to_string(), col.nullable.to_string());
                    if let Some(ref default_value) = col.default_value {
                        col_data.insert("default_value".to_string(), default_value.clone());
                    }
                    if let Some(auto_increment) = col.auto_increment {
                        col_data.insert("auto_increment".to_string(), auto_increment.to_string());
                    }
                    col_data
                })
                .collect::<Vec<_>>();
            sorted_columns.sort_by(|a, b| a.get("name").cmp(&b.get("name")));

            // インデックスを名前順にソート
            let mut sorted_indexes = table
                .indexes
                .iter()
                .map(|idx| {
                    let mut idx_data = BTreeMap::new();
                    idx_data.insert("name".to_string(), idx.name.clone());
                    idx_data.insert("columns".to_string(), idx.columns.join(","));
                    idx_data.insert("unique".to_string(), idx.unique.to_string());
                    idx_data
                })
                .collect::<Vec<_>>();
            sorted_indexes.sort_by(|a, b| a.get("name").cmp(&b.get("name")));

            // 制約を種類と内容でソート
            let mut sorted_constraints = table
                .constraints
                .iter()
                .map(|constraint| {
                    let mut constraint_data = BTreeMap::new();
                    constraint_data.insert("type".to_string(), constraint.kind().to_string());

                    match constraint {
                        crate::core::schema::Constraint::PRIMARY_KEY { columns } => {
                            constraint_data.insert("columns".to_string(), columns.join(","));
                        }
                        crate::core::schema::Constraint::FOREIGN_KEY {
                            columns,
                            referenced_table,
                            referenced_columns,
                            on_delete,
                            on_update,
                        } => {
                            constraint_data.insert("columns".to_string(), columns.join(","));
                            constraint_data
                                .insert("referenced_table".to_string(), referenced_table.clone());
                            constraint_data.insert(
                                "referenced_columns".to_string(),
                                referenced_columns.join(","),
                            );
                            if let Some(action) = on_delete {
                                constraint_data
                                    .insert("on_delete".to_string(), action.as_sql().to_string());
                            }
                            if let Some(action) = on_update {
                                constraint_data
                                    .insert("on_update".to_string(), action.as_sql().to_string());
                            }
                        }
                        crate::core::schema::Constraint::UNIQUE { columns } => {
                            constraint_data.insert("columns".to_string(), columns.join(","));
                        }
                        crate::core::schema::Constraint::CHECK {
                            columns,
                            check_expression,
                        } => {
                            constraint_data.insert("columns".to_string(), columns.join(","));
                            constraint_data
                                .insert("check_expression".to_string(), check_expression.clone());
                        }
                    }

                    constraint_data
                })
                .collect::<Vec<_>>();
            sorted_constraints.sort_by(|a, b| {
                a.get("type")
                    .cmp(&b.get("type"))
                    .then(a.get("columns").cmp(&b.get("columns")))
            });

            // JSON風の文字列に変換
            let columns_str = sorted_columns
                .iter()
                .map(|c| format!("{:?}", c))
                .collect::<Vec<_>>()
                .join(",");
            let indexes_str = sorted_indexes
                .iter()
                .map(|i| format!("{:?}", i))
                .collect::<Vec<_>>()
                .join(",");
            let constraints_str = sorted_constraints
                .iter()
                .map(|c| format!("{:?}", c))
                .collect::<Vec<_>>()
                .join(",");

            table_data.insert("columns".to_string(), columns_str);
            table_data.insert("indexes".to_string(), indexes_str);
            table_data.insert("constraints".to_string(), constraints_str);

            sorted_tables.insert(table_name.clone(), format!("{:?}", table_data));
        }

        // 最終的な正規化された文字列を生成
        format!(
            "{{version:{},tables:{{{}}}}}",
            schema.version,
            sorted_tables
                .iter()
                .map(|(k, v)| format!("{}:{}", k, v))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    /// チェックサムを比較
    ///
    /// # Arguments
    ///
    /// * `checksum1` - 比較する最初のチェックサム
    /// * `checksum2` - 比較する2番目のチェックサム
    ///
    /// # Returns
    ///
    /// チェックサムが一致する場合は true、そうでない場合は false
    pub fn compare_checksums(&self, checksum1: &str, checksum2: &str) -> bool {
        checksum1 == checksum2
    }
}

impl Default for SchemaChecksumService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{Column, ColumnType, Constraint, Table};

    #[test]
    fn test_new_service() {
        let service = SchemaChecksumService::new();
        assert!(format!("{:?}", service).contains("SchemaChecksumService"));
    }

    #[test]
    fn test_calculate_checksum_empty_schema() {
        let schema = Schema::new("1.0".to_string());
        let service = SchemaChecksumService::new();
        let checksum = service.calculate_checksum(&schema);

        // SHA-256ハッシュは64文字の16進数文字列
        assert_eq!(checksum.len(), 64);
        assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_checksum_deterministic() {
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

        let service = SchemaChecksumService::new();
        let checksum1 = service.calculate_checksum(&schema);
        let checksum2 = service.calculate_checksum(&schema);

        // 同じスキーマは常に同じチェックサムを生成
        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_compare_checksums() {
        let service = SchemaChecksumService::new();
        let checksum1 = "abc123";
        let checksum2 = "abc123";
        let checksum3 = "def456";

        assert!(service.compare_checksums(checksum1, checksum2));
        assert!(!service.compare_checksums(checksum1, checksum3));
    }

    #[test]
    fn test_normalize_schema() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema.add_table(table);

        let service = SchemaChecksumService::new();
        let normalized = service.normalize_schema(&schema);

        // 正規化された表現は空ではない
        assert!(!normalized.is_empty());
        assert!(normalized.contains("users"));
        assert!(normalized.contains("id"));
    }
}
