// 共通型フォーマットロジック
//
// 複数の方言で共通するColumnType → SQL型文字列の変換を提供します。

use crate::core::schema::ColumnType;

/// 共通SQL型のフォーマット
///
/// 複数の方言で同一の出力となる型変換を行います。
/// 方言固有の変換が必要な場合は `None` を返します。
///
/// # Arguments
/// * `column_type` - 変換対象の内部型
///
/// # Returns
/// 共通型の場合は `Some(SQL型文字列)`、方言固有の場合は `None`
pub fn format_common_sql_type(column_type: &ColumnType) -> Option<String> {
    match column_type {
        ColumnType::VARCHAR { length } => Some(format!("VARCHAR({})", length)),
        ColumnType::TEXT => Some("TEXT".to_string()),
        ColumnType::BOOLEAN => Some("BOOLEAN".to_string()),
        ColumnType::CHAR { length } => Some(format!("CHAR({})", length)),
        ColumnType::DATE => Some("DATE".to_string()),
        ColumnType::JSON => Some("JSON".to_string()),
        _ => None,
    }
}
