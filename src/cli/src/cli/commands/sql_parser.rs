// SQLステートメント分割パーサー
//
// SQL文字列をセミコロン区切りで個別のステートメントに分割します。
// シングルクォート、ダブルクォート、PostgreSQLドル引用符内の
// セミコロンはステートメント区切りとして扱いません。
// SQLコメント（行コメント `--` / ブロックコメント `/* */`）内の
// セミコロンも同様にスキップします。

/// SQL文字列を個別のステートメントに分割
///
/// クォート内やコメント内のセミコロンを正しくスキップしながら、
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
    let mut state = ParseState::Normal;
    let chars: Vec<(usize, char)> = sql.char_indices().collect();
    let mut i = 0;

    while i < chars.len() {
        let (byte_pos, c) = chars[i];

        match &state {
            ParseState::DollarQuoted(tag) => {
                if c == '$' && sql[byte_pos..].starts_with(tag.as_str()) {
                    current.push_str(tag);
                    // タグの文字数分スキップ
                    let tag_char_count = tag.chars().count();
                    i += tag_char_count;
                    state = ParseState::Normal;
                } else {
                    current.push(c);
                    i += 1;
                }
            }
            ParseState::SingleQuoted => {
                if c == '\'' {
                    if i + 1 < chars.len() && chars[i + 1].1 == '\'' {
                        // エスケープされたシングルクォート('')
                        current.push('\'');
                        current.push('\'');
                        i += 2;
                        continue;
                    }
                    state = ParseState::Normal;
                }
                current.push(c);
                i += 1;
            }
            ParseState::DoubleQuoted => {
                if c == '"' {
                    if i + 1 < chars.len() && chars[i + 1].1 == '"' {
                        // エスケープされたダブルクォート("")
                        current.push('"');
                        current.push('"');
                        i += 2;
                        continue;
                    }
                    state = ParseState::Normal;
                }
                current.push(c);
                i += 1;
            }
            ParseState::LineComment => {
                current.push(c);
                if c == '\n' {
                    state = ParseState::Normal;
                }
                i += 1;
            }
            ParseState::BlockComment(depth) => {
                let depth = *depth;
                if c == '/' && i + 1 < chars.len() && chars[i + 1].1 == '*' {
                    // ネストされたブロックコメント開始
                    current.push('/');
                    current.push('*');
                    i += 2;
                    state = ParseState::BlockComment(depth + 1);
                } else if c == '*' && i + 1 < chars.len() && chars[i + 1].1 == '/' {
                    current.push('*');
                    current.push('/');
                    i += 2;
                    if depth == 1 {
                        state = ParseState::Normal;
                    } else {
                        state = ParseState::BlockComment(depth - 1);
                    }
                } else {
                    current.push(c);
                    i += 1;
                }
            }
            ParseState::Normal => {
                match c {
                    '\'' => {
                        state = ParseState::SingleQuoted;
                        current.push(c);
                        i += 1;
                    }
                    '"' => {
                        state = ParseState::DoubleQuoted;
                        current.push(c);
                        i += 1;
                    }
                    '-' if i + 1 < chars.len() && chars[i + 1].1 == '-' => {
                        // 行コメント開始
                        state = ParseState::LineComment;
                        current.push('-');
                        current.push('-');
                        i += 2;
                    }
                    '/' if i + 1 < chars.len() && chars[i + 1].1 == '*' => {
                        // ブロックコメント開始（深さ1）
                        state = ParseState::BlockComment(1);
                        current.push('/');
                        current.push('*');
                        i += 2;
                    }
                    '$' => {
                        i = try_start_dollar_quote(&mut current, sql, &chars, i, &mut state);
                    }
                    ';' => {
                        let trimmed = current.trim();
                        if !trimmed.is_empty() {
                            statements.push(trimmed.to_string());
                        }
                        current.clear();
                        i += 1;
                    }
                    _ => {
                        current.push(c);
                        i += 1;
                    }
                }
            }
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }

    // コメントのみのステートメントを除外
    statements
        .into_iter()
        .filter(|s| !is_comment_only(s))
        .collect()
}

/// ステートメントがコメントのみで構成されているかを判定
///
/// 行コメント (`--`) とブロックコメント (`/* */`) を除去した後に
/// 有効なSQL文が残らない場合は true を返します。
fn is_comment_only(s: &str) -> bool {
    let mut remaining = s.trim();

    loop {
        if remaining.is_empty() {
            return true;
        }

        if remaining.starts_with("--") {
            // 行コメント: 改行まで、または末尾までスキップ
            match remaining.find('\n') {
                Some(pos) => remaining = remaining[pos + 1..].trim(),
                None => return true,
            }
        } else if remaining.starts_with("/*") {
            // ブロックコメント: ネスト対応でスキップ
            let mut depth: u32 = 1;
            let chars: Vec<char> = remaining.chars().collect();
            let mut i = 2; // "/*" の次から
            while i < chars.len() && depth > 0 {
                if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if depth > 0 {
                // 閉じられていないコメント: 全体がコメント扱い
                return true;
            }
            // コメント後の残りをチェック
            let byte_offset: usize = chars[..i].iter().map(|c| c.len_utf8()).sum();
            remaining = remaining[byte_offset..].trim();
        } else {
            // コメントでない文字が見つかった
            return false;
        }
    }
}

/// パーサーの状態
enum ParseState {
    Normal,
    SingleQuoted,
    DoubleQuoted,
    DollarQuoted(String),
    LineComment,
    /// ブロックコメント（ネスト深さを保持。PostgreSQLのネストされたコメントに対応）
    BlockComment(u32),
}

/// ドル引用符の開始を試行
///
/// `$tag$` パターンに一致する場合はドル引用符状態に遷移し、
/// 一致しない場合はリテラル `$` として扱います。
fn try_start_dollar_quote(
    current: &mut String,
    sql: &str,
    chars: &[(usize, char)],
    i: usize,
    state: &mut ParseState,
) -> usize {
    let (byte_pos, _) = chars[i];
    if let Some(end) = sql[byte_pos + 1..].find('$') {
        let tag = &sql[byte_pos..=byte_pos + end + 1];
        let inner = &tag[1..tag.len() - 1];
        if inner
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            *state = ParseState::DollarQuoted(tag.to_string());
            current.push_str(tag);
            let tag_char_count = tag.chars().count();
            return i + tag_char_count;
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

    // ==========================================
    // UTF-8対応テスト
    // ==========================================

    #[test]
    fn test_utf8_in_string_literal() {
        // 日本語文字列リテラル内のセミコロンは区切りとして扱わない
        let sql = "INSERT INTO t VALUES ('日本語;テスト'); SELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "INSERT INTO t VALUES ('日本語;テスト')");
        assert_eq!(stmts[1], "SELECT 1");
    }

    #[test]
    fn test_utf8_in_identifier() {
        // マルチバイト文字を含む識別子
        let sql = r#"SELECT "名前" FROM t; SELECT 1;"#;
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
    }

    // ==========================================
    // コメント処理テスト
    // ==========================================

    #[test]
    fn test_line_comment_with_semicolon() {
        // 行コメント内のセミコロンは区切りとして扱わない
        let sql = "SELECT 1 -- comment; not a separator\nFROM t;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "SELECT 1 -- comment; not a separator\nFROM t");
    }

    #[test]
    fn test_block_comment_with_semicolon() {
        // ブロックコメント内のセミコロンは区切りとして扱わない
        let sql = "SELECT 1 /* comment; with semicolon */ FROM t;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "SELECT 1 /* comment; with semicolon */ FROM t");
    }

    #[test]
    fn test_block_comment_multiline() {
        let sql = "SELECT 1\n/* multi\nline; comment\n*/\nFROM t; SELECT 2;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_line_comment_at_end() {
        // コメントのみのステートメントは除外される
        let sql = "SELECT 1; -- trailing comment";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "SELECT 1");
    }

    #[test]
    fn test_double_dash_in_string_not_comment() {
        // 文字列リテラル内の -- はコメントとして扱わない
        let sql = "INSERT INTO t VALUES ('a--b;c'); SELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "INSERT INTO t VALUES ('a--b;c')");
    }

    #[test]
    fn test_comment_only_statement_filtered() {
        // コメントのみのステートメントが除外されること
        let sql = "/* just a comment */; SELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "SELECT 1");
    }

    #[test]
    fn test_comment_with_sql_preserved() {
        // コメント + SQL のステートメントは保持される
        let sql = "/* comment */ SELECT 1; SELECT 2;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "/* comment */ SELECT 1");
    }

    #[test]
    fn test_nested_block_comment() {
        // PostgreSQLスタイルのネストされたブロックコメント
        let sql = "SELECT 1 /* outer /* inner; */ still comment; */ FROM t; SELECT 2;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(
            stmts[0],
            "SELECT 1 /* outer /* inner; */ still comment; */ FROM t"
        );
        assert_eq!(stmts[1], "SELECT 2");
    }

    #[test]
    fn test_nested_block_comment_deep() {
        // 3段ネスト
        let sql = "SELECT /* a /* b /* c; */ d */ e */ 1; SELECT 2;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT /* a /* b /* c; */ d */ e */ 1");
        assert_eq!(stmts[1], "SELECT 2");
    }
}
