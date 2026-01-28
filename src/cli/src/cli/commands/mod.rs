// コマンドハンドラー層
// 各CLIコマンドの実装

pub mod apply;
pub mod destructive_change_formatter;
pub(crate) mod dry_run_formatter;
pub mod export;
pub mod generate;
pub mod init;
pub mod migration_loader;
pub mod rollback;
pub(crate) mod sql_parser;
pub mod status;
pub mod validate;

pub(crate) use sql_parser::split_sql_statements;

use regex::Regex;
use std::sync::LazyLock;

/// 破壊的 SQL 操作を検出するための共通正規表現
///
/// 検出対象:
/// - DROP TABLE/COLUMN/TYPE/INDEX/CONSTRAINT/SCHEMA/DATABASE
/// - ALTER ... DROP/RENAME
/// - RENAME TABLE/COLUMN
/// - TRUNCATE TABLE
/// - DELETE FROM (WHERE句の有無に関わらず潜在的に破壊的)
pub(crate) static DESTRUCTIVE_SQL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(DROP\s+(TABLE|COLUMN|TYPE|INDEX|CONSTRAINT|SCHEMA|DATABASE)|ALTER\s+.*\s+(DROP|RENAME)|RENAME\s+(TABLE|COLUMN)|TRUNCATE\s+TABLE|DELETE\s+FROM)\b")
        .expect("Invalid destructive SQL regex pattern")
});
