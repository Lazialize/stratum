// SQLステートメント分割パーサー
//
// SQL文字列をセミコロン区切りで個別のステートメントに分割します。
// シングルクォート、ダブルクォート、PostgreSQLドル引用符内の
// セミコロンはステートメント区切りとして扱いません。

/// SQL文字列を個別のステートメントに分割
///
/// クォート内のセミコロンを正しくスキップしながら、
/// SQL文を分割します。
///
/// # Arguments
///
/// * `sql` - 分割するSQL文字列
///
/// # Returns
///
/// 個別のSQL文のベクター（前後の空白はトリム済み）
pub(crate) fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut parser = QuoteState::None;
    let bytes = sql.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i] as char;

        match &parser {
            QuoteState::DollarQuoted(tag) => {
                i = consume_dollar_quoted(&mut current, sql, i, c, tag.clone(), &mut parser);
            }
            QuoteState::SingleQuoted => {
                i = consume_single_quoted(&mut current, bytes, i, c, &mut parser);
            }
            QuoteState::DoubleQuoted => {
                i = consume_double_quoted(&mut current, bytes, i, c, &mut parser);
            }
            QuoteState::None => {
                i = consume_unquoted(&mut current, &mut statements, sql, bytes, i, c, &mut parser);
            }
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }

    statements
}

/// クォート状態
enum QuoteState {
    None,
    SingleQuoted,
    DoubleQuoted,
    DollarQuoted(String),
}

/// ドル引用符内の文字を処理
///
/// 閉じタグに一致したらクォート状態を解除します。
fn consume_dollar_quoted(
    current: &mut String,
    sql: &str,
    i: usize,
    c: char,
    tag: String,
    state: &mut QuoteState,
) -> usize {
    if c == '$' && sql[i..].starts_with(&tag) {
        current.push_str(&tag);
        *state = QuoteState::None;
        i + tag.len()
    } else {
        current.push(c);
        i + 1
    }
}

/// シングルクォート内の文字を処理
///
/// エスケープされたシングルクォート('')をスキップし、
/// 閉じクォートでクォート状態を解除します。
fn consume_single_quoted(
    current: &mut String,
    bytes: &[u8],
    i: usize,
    c: char,
    state: &mut QuoteState,
) -> usize {
    if c == '\'' {
        if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
            current.push('\'');
            current.push('\'');
            return i + 2;
        }
        *state = QuoteState::None;
    }
    current.push(c);
    i + 1
}

/// ダブルクォート内の文字を処理
///
/// エスケープされたダブルクォート("")をスキップし、
/// 閉じクォートでクォート状態を解除します。
fn consume_double_quoted(
    current: &mut String,
    bytes: &[u8],
    i: usize,
    c: char,
    state: &mut QuoteState,
) -> usize {
    if c == '"' {
        if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
            current.push('"');
            current.push('"');
            return i + 2;
        }
        *state = QuoteState::None;
    }
    current.push(c);
    i + 1
}

/// クォート外の文字を処理
///
/// セミコロンでステートメントを区切り、
/// クォート開始文字でクォート状態に遷移します。
fn consume_unquoted(
    current: &mut String,
    statements: &mut Vec<String>,
    sql: &str,
    _bytes: &[u8],
    i: usize,
    c: char,
    state: &mut QuoteState,
) -> usize {
    match c {
        '\'' => {
            *state = QuoteState::SingleQuoted;
            current.push(c);
            i + 1
        }
        '"' => {
            *state = QuoteState::DoubleQuoted;
            current.push(c);
            i + 1
        }
        '$' => try_start_dollar_quote(current, sql, i, state),
        ';' => {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                statements.push(trimmed.to_string());
            }
            current.clear();
            i + 1
        }
        _ => {
            current.push(c);
            i + 1
        }
    }
}

/// ドル引用符の開始を試行
///
/// `$tag$` パターンに一致する場合はドル引用符状態に遷移し、
/// 一致しない場合はリテラル `$` として扱います。
fn try_start_dollar_quote(
    current: &mut String,
    sql: &str,
    i: usize,
    state: &mut QuoteState,
) -> usize {
    if let Some(end) = sql[i + 1..].find('$') {
        let tag = &sql[i..=i + end + 1];
        let inner = &tag[1..tag.len() - 1];
        if inner
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            *state = QuoteState::DollarQuoted(tag.to_string());
            current.push_str(tag);
            return i + tag.len();
        }
    }
    current.push('$');
    i + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_statements() {
        let sql = "CREATE TABLE users (id INT); INSERT INTO users VALUES (1);";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "CREATE TABLE users (id INT)");
        assert_eq!(stmts[1], "INSERT INTO users VALUES (1)");
    }

    #[test]
    fn test_single_quoted_semicolon() {
        let sql = "INSERT INTO t VALUES ('a;b'); SELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "INSERT INTO t VALUES ('a;b')");
    }

    #[test]
    fn test_double_quoted_semicolon() {
        let sql = r#"SELECT "col;name" FROM t; SELECT 1;"#;
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_dollar_quoted_semicolon() {
        let sql = "CREATE FUNCTION f() RETURNS void AS $$ BEGIN NULL; END; $$ LANGUAGE plpgsql; SELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_escaped_single_quote() {
        let sql = "INSERT INTO t VALUES ('it''s'); SELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "INSERT INTO t VALUES ('it''s')");
    }

    #[test]
    fn test_trailing_statement_without_semicolon() {
        let sql = "SELECT 1";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "SELECT 1");
    }

    #[test]
    fn test_empty_input() {
        let stmts = split_sql_statements("");
        assert!(stmts.is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let stmts = split_sql_statements("  \n  ");
        assert!(stmts.is_empty());
    }
}
