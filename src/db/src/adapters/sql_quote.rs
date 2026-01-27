// SQL識別子クォートユーティリティ
//
// 各データベース方言用の識別子クォート関数を提供します。
// type_mappingとsql_generatorの両方から使用される共有モジュールです。

/// PostgreSQL用識別子クォート（ダブルクォート）
///
/// 識別子内のダブルクォートは二重にエスケープします。
///
/// # Examples
/// ```
/// use strata_db::adapters::sql_quote::quote_identifier_postgres;
/// assert_eq!(quote_identifier_postgres("users"), r#""users""#);
/// assert_eq!(quote_identifier_postgres(r#"table"name"#), r#""table""name""#);
/// ```
pub fn quote_identifier_postgres(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// MySQL用識別子クォート（バッククォート）
///
/// 識別子内のバッククォートは二重にエスケープします。
///
/// # Examples
/// ```
/// use strata_db::adapters::sql_quote::quote_identifier_mysql;
/// assert_eq!(quote_identifier_mysql("users"), "`users`");
/// assert_eq!(quote_identifier_mysql("table`name"), "`table``name`");
/// ```
pub fn quote_identifier_mysql(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

/// SQLite用識別子クォート（ダブルクォート）
///
/// 識別子内のダブルクォートは二重にエスケープします。
///
/// # Examples
/// ```
/// use strata_db::adapters::sql_quote::quote_identifier_sqlite;
/// assert_eq!(quote_identifier_sqlite("users"), r#""users""#);
/// assert_eq!(quote_identifier_sqlite(r#"table"name"#), r#""table""name""#);
/// ```
pub fn quote_identifier_sqlite(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// カラム名リストをクォートしてカンマ区切りで結合（PostgreSQL用）
pub fn quote_columns_postgres(columns: &[String]) -> String {
    columns
        .iter()
        .map(|c| quote_identifier_postgres(c))
        .collect::<Vec<_>>()
        .join(", ")
}

/// カラム名リストをクォートしてカンマ区切りで結合（MySQL用）
pub fn quote_columns_mysql(columns: &[String]) -> String {
    columns
        .iter()
        .map(|c| quote_identifier_mysql(c))
        .collect::<Vec<_>>()
        .join(", ")
}

/// カラム名リストをクォートしてカンマ区切りで結合（SQLite用）
pub fn quote_columns_sqlite(columns: &[String]) -> String {
    columns
        .iter()
        .map(|c| quote_identifier_sqlite(c))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PostgreSQL quote_identifier tests
    // =========================================================================

    #[test]
    fn test_quote_identifier_postgres_simple() {
        assert_eq!(quote_identifier_postgres("users"), r#""users""#);
        assert_eq!(quote_identifier_postgres("table_name"), r#""table_name""#);
    }

    #[test]
    fn test_quote_identifier_postgres_reserved_word() {
        assert_eq!(quote_identifier_postgres("select"), r#""select""#);
        assert_eq!(quote_identifier_postgres("order"), r#""order""#);
        assert_eq!(quote_identifier_postgres("group"), r#""group""#);
    }

    #[test]
    fn test_quote_identifier_postgres_with_embedded_quote() {
        // ダブルクォートを含む識別子は二重にエスケープ
        assert_eq!(
            quote_identifier_postgres(r#"table"name"#),
            r#""table""name""#
        );
        // 単一の " は "" にエスケープされ、外側のクォートと合わせて """" になる
        assert_eq!(quote_identifier_postgres("\""), "\"\"\"\"");
        assert_eq!(quote_identifier_postgres(r#"a"b"c"#), r#""a""b""c""#);
    }

    #[test]
    fn test_quote_identifier_postgres_mixed_case() {
        assert_eq!(quote_identifier_postgres("CamelCase"), r#""CamelCase""#);
        assert_eq!(quote_identifier_postgres("UPPERCASE"), r#""UPPERCASE""#);
    }

    #[test]
    fn test_quote_identifier_postgres_special_chars() {
        assert_eq!(quote_identifier_postgres("table-name"), r#""table-name""#);
        assert_eq!(quote_identifier_postgres("table name"), r#""table name""#);
        assert_eq!(quote_identifier_postgres("table.name"), r#""table.name""#);
    }

    #[test]
    fn test_quote_identifier_postgres_empty() {
        assert_eq!(quote_identifier_postgres(""), r#""""#);
    }

    // =========================================================================
    // MySQL quote_identifier tests
    // =========================================================================

    #[test]
    fn test_quote_identifier_mysql_simple() {
        assert_eq!(quote_identifier_mysql("users"), "`users`");
        assert_eq!(quote_identifier_mysql("table_name"), "`table_name`");
    }

    #[test]
    fn test_quote_identifier_mysql_reserved_word() {
        assert_eq!(quote_identifier_mysql("select"), "`select`");
        assert_eq!(quote_identifier_mysql("order"), "`order`");
        assert_eq!(quote_identifier_mysql("group"), "`group`");
    }

    #[test]
    fn test_quote_identifier_mysql_with_embedded_backtick() {
        // バッククォートを含む識別子は二重にエスケープ
        assert_eq!(quote_identifier_mysql("table`name"), "`table``name`");
        // 単一の ` は `` にエスケープされ、外側のクォートと合わせて ```` になる
        assert_eq!(quote_identifier_mysql("`"), "````");
        assert_eq!(quote_identifier_mysql("a`b`c"), "`a``b``c`");
    }

    #[test]
    fn test_quote_identifier_mysql_mixed_case() {
        assert_eq!(quote_identifier_mysql("CamelCase"), "`CamelCase`");
        assert_eq!(quote_identifier_mysql("UPPERCASE"), "`UPPERCASE`");
    }

    #[test]
    fn test_quote_identifier_mysql_special_chars() {
        assert_eq!(quote_identifier_mysql("table-name"), "`table-name`");
        assert_eq!(quote_identifier_mysql("table name"), "`table name`");
        // MySQLではダブルクォートはエスケープ不要
        assert_eq!(quote_identifier_mysql(r#"table"name"#), r#"`table"name`"#);
    }

    #[test]
    fn test_quote_identifier_mysql_empty() {
        assert_eq!(quote_identifier_mysql(""), "``");
    }

    // =========================================================================
    // SQLite quote_identifier tests
    // =========================================================================

    #[test]
    fn test_quote_identifier_sqlite_simple() {
        assert_eq!(quote_identifier_sqlite("users"), r#""users""#);
        assert_eq!(quote_identifier_sqlite("table_name"), r#""table_name""#);
    }

    #[test]
    fn test_quote_identifier_sqlite_reserved_word() {
        assert_eq!(quote_identifier_sqlite("select"), r#""select""#);
        assert_eq!(quote_identifier_sqlite("order"), r#""order""#);
        assert_eq!(quote_identifier_sqlite("group"), r#""group""#);
    }

    #[test]
    fn test_quote_identifier_sqlite_with_embedded_quote() {
        // ダブルクォートを含む識別子は二重にエスケープ
        assert_eq!(quote_identifier_sqlite(r#"table"name"#), r#""table""name""#);
        // 単一の " は "" にエスケープされ、外側のクォートと合わせて """" になる
        assert_eq!(quote_identifier_sqlite("\""), "\"\"\"\"");
        assert_eq!(quote_identifier_sqlite(r#"a"b"c"#), r#""a""b""c""#);
    }

    #[test]
    fn test_quote_identifier_sqlite_mixed_case() {
        assert_eq!(quote_identifier_sqlite("CamelCase"), r#""CamelCase""#);
        assert_eq!(quote_identifier_sqlite("UPPERCASE"), r#""UPPERCASE""#);
    }

    #[test]
    fn test_quote_identifier_sqlite_empty() {
        assert_eq!(quote_identifier_sqlite(""), r#""""#);
    }

    // =========================================================================
    // quote_columns tests
    // =========================================================================

    #[test]
    fn test_quote_columns_postgres_single() {
        let columns = vec!["id".to_string()];
        assert_eq!(quote_columns_postgres(&columns), r#""id""#);
    }

    #[test]
    fn test_quote_columns_postgres_multiple() {
        let columns = vec!["id".to_string(), "name".to_string(), "email".to_string()];
        assert_eq!(quote_columns_postgres(&columns), r#""id", "name", "email""#);
    }

    #[test]
    fn test_quote_columns_postgres_with_special_chars() {
        let columns = vec!["user_id".to_string(), r#"col"name"#.to_string()];
        assert_eq!(
            quote_columns_postgres(&columns),
            r#""user_id", "col""name""#
        );
    }

    #[test]
    fn test_quote_columns_postgres_empty() {
        let columns: Vec<String> = vec![];
        assert_eq!(quote_columns_postgres(&columns), "");
    }

    #[test]
    fn test_quote_columns_mysql_single() {
        let columns = vec!["id".to_string()];
        assert_eq!(quote_columns_mysql(&columns), "`id`");
    }

    #[test]
    fn test_quote_columns_mysql_multiple() {
        let columns = vec!["id".to_string(), "name".to_string(), "email".to_string()];
        assert_eq!(quote_columns_mysql(&columns), "`id`, `name`, `email`");
    }

    #[test]
    fn test_quote_columns_mysql_with_backtick() {
        let columns = vec!["user_id".to_string(), "col`name".to_string()];
        assert_eq!(quote_columns_mysql(&columns), "`user_id`, `col``name`");
    }

    #[test]
    fn test_quote_columns_mysql_empty() {
        let columns: Vec<String> = vec![];
        assert_eq!(quote_columns_mysql(&columns), "");
    }

    #[test]
    fn test_quote_columns_sqlite_single() {
        let columns = vec!["id".to_string()];
        assert_eq!(quote_columns_sqlite(&columns), r#""id""#);
    }

    #[test]
    fn test_quote_columns_sqlite_multiple() {
        let columns = vec!["id".to_string(), "name".to_string(), "email".to_string()];
        assert_eq!(quote_columns_sqlite(&columns), r#""id", "name", "email""#);
    }

    #[test]
    fn test_quote_columns_sqlite_empty() {
        let columns: Vec<String> = vec![];
        assert_eq!(quote_columns_sqlite(&columns), "");
    }
}
