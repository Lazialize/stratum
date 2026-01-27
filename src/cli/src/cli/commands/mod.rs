// コマンドハンドラー層
// 各CLIコマンドの実装

pub mod apply;
pub mod destructive_change_formatter;
pub mod export;
pub mod generate;
pub mod init;
pub mod rollback;
pub mod status;
pub mod validate;

pub(crate) fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let bytes = sql.as_bytes();
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut dollar_tag: Option<String> = None;

    while i < bytes.len() {
        let c = bytes[i] as char;

        if let Some(tag) = dollar_tag.as_ref() {
            if c == '$' && sql[i..].starts_with(tag) {
                current.push_str(tag);
                i += tag.len();
                dollar_tag = None;
                continue;
            }
            current.push(c);
            i += 1;
            continue;
        }

        if in_single {
            if c == '\'' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    current.push('\'');
                    current.push('\'');
                    i += 2;
                    continue;
                }
                in_single = false;
            }
            current.push(c);
            i += 1;
            continue;
        }

        if in_double {
            if c == '"' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                    current.push('"');
                    current.push('"');
                    i += 2;
                    continue;
                }
                in_double = false;
            }
            current.push(c);
            i += 1;
            continue;
        }

        match c {
            '\'' => {
                in_single = true;
                current.push(c);
                i += 1;
            }
            '"' => {
                in_double = true;
                current.push(c);
                i += 1;
            }
            '$' => {
                if let Some(end) = sql[i + 1..].find('$') {
                    let tag = &sql[i..=i + end + 1];
                    let inner = &tag[1..tag.len() - 1];
                    if inner
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                    {
                        dollar_tag = Some(tag.to_string());
                        current.push_str(tag);
                        i += tag.len();
                    } else {
                        current.push(c);
                        i += 1;
                    }
                } else {
                    current.push(c);
                    i += 1;
                }
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

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }

    statements
}
