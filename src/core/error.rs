// エラー型定義
//
// アプリケーション全体で使用されるカスタムエラー型を提供します。
// thiserrorを使用して、ValidationError, DatabaseError, IoError, MigrationError を定義します。

use thiserror::Error;

/// バリデーションエラー
///
/// スキーマ定義ファイルの検証時に発生するエラーを表現します。
#[derive(Debug, Clone, Error)]
pub enum ValidationError {
    /// 構文エラー
    #[error("構文エラー: {message}{}", format_location_opt(.location))]
    Syntax {
        /// エラーメッセージ
        message: String,
        /// エラー発生位置
        location: Option<ErrorLocation>,
        /// 修正提案
        suggestion: Option<String>,
    },

    /// 参照エラー
    #[error("参照エラー: {message}{}", format_location_opt(.location))]
    Reference {
        /// エラーメッセージ
        message: String,
        /// エラー発生位置
        location: Option<ErrorLocation>,
        /// 修正提案
        suggestion: Option<String>,
    },

    /// 制約エラー
    #[error("制約エラー: {message}{}", format_location_opt(.location))]
    Constraint {
        /// エラーメッセージ
        message: String,
        /// エラー発生位置
        location: Option<ErrorLocation>,
        /// 修正提案
        suggestion: Option<String>,
    },
}

impl ValidationError {
    /// 構文エラーかどうか
    pub fn is_syntax(&self) -> bool {
        matches!(self, ValidationError::Syntax { .. })
    }

    /// 参照エラーかどうか
    pub fn is_reference(&self) -> bool {
        matches!(self, ValidationError::Reference { .. })
    }

    /// 制約エラーかどうか
    pub fn is_constraint(&self) -> bool {
        matches!(self, ValidationError::Constraint { .. })
    }

    /// エラー発生位置を取得
    pub fn location(&self) -> Option<&ErrorLocation> {
        match self {
            ValidationError::Syntax { location, .. }
            | ValidationError::Reference { location, .. }
            | ValidationError::Constraint { location, .. } => location.as_ref(),
        }
    }

    /// 修正提案を取得
    pub fn suggestion(&self) -> Option<&str> {
        match self {
            ValidationError::Syntax { suggestion, .. }
            | ValidationError::Reference { suggestion, .. }
            | ValidationError::Constraint { suggestion, .. } => suggestion.as_deref(),
        }
    }
}

/// エラー発生位置
///
/// スキーマファイル内のエラー発生位置を表現します。
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorLocation {
    /// テーブル名
    pub table: Option<String>,
    /// カラム名
    pub column: Option<String>,
    /// 行番号
    pub line: Option<usize>,
}

impl ErrorLocation {
    /// 新しいエラー位置を作成
    pub fn new() -> Self {
        Self {
            table: None,
            column: None,
            line: None,
        }
    }

    /// テーブル名を指定してエラー位置を作成
    pub fn with_table(table: String) -> Self {
        Self {
            table: Some(table),
            column: None,
            line: None,
        }
    }

    /// 位置情報をフォーマット
    pub fn format(&self) -> String {
        let mut parts = Vec::new();

        if let Some(table) = &self.table {
            parts.push(format!("テーブル: {}", table));
        }
        if let Some(column) = &self.column {
            parts.push(format!("カラム: {}", column));
        }
        if let Some(line) = self.line {
            parts.push(format!("行: {}", line));
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!(" ({})", parts.join(", "))
        }
    }
}

impl Default for ErrorLocation {
    fn default() -> Self {
        Self::new()
    }
}

/// 位置情報をフォーマットするヘルパー関数
fn format_location_opt(location: &Option<ErrorLocation>) -> String {
    location.as_ref().map_or(String::new(), |loc| loc.format())
}

/// バリデーション結果
///
/// スキーマ検証の結果を表現します。
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// エラーのリスト
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    /// 新しいバリデーション結果を作成
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// エラーを追加
    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// 検証が成功したかどうか
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// エラーの数を取得
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// 他のバリデーション結果をマージ
    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// マイグレーションエラー
///
/// マイグレーション適用時に発生するエラーを表現します。
#[derive(Debug, Clone, Error)]
#[error("マイグレーション {version} 失敗: {error}")]
pub struct MigrationError {
    /// マイグレーションバージョン
    pub version: String,
    /// エラーメッセージ
    pub error: String,
    /// 失敗したSQL文
    pub sql_statement: Option<String>,
}

impl MigrationError {
    /// 新しいマイグレーションエラーを作成
    pub fn new(version: String, error: String) -> Self {
        Self {
            version,
            error,
            sql_statement: None,
        }
    }

    /// SQL文を指定してマイグレーションエラーを作成
    pub fn with_sql(version: String, error: String, sql_statement: String) -> Self {
        Self {
            version,
            error,
            sql_statement: Some(sql_statement),
        }
    }

    /// バージョンを取得
    pub fn version(&self) -> &str {
        &self.version
    }

    /// SQL文が含まれているかどうか
    pub fn has_sql_statement(&self) -> bool {
        self.sql_statement.is_some()
    }
}

/// データベースエラー
///
/// データベース操作時に発生するエラーを表現します。
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// 接続エラー
    #[error("データベース接続エラー: {message} (原因: {cause})")]
    Connection {
        /// エラーメッセージ
        message: String,
        /// エラー原因
        cause: String,
    },

    /// クエリ実行エラー
    #[error("クエリ実行エラー: {message}")]
    Query {
        /// エラーメッセージ
        message: String,
        /// 失敗したSQL
        sql: Option<String>,
    },

    /// トランザクションエラー
    #[error("トランザクションエラー: {message}")]
    Transaction {
        /// エラーメッセージ
        message: String,
    },

    /// マイグレーションエラー
    #[error("マイグレーションエラー: {error}")]
    Migration {
        /// マイグレーションエラー
        error: MigrationError,
    },
}

impl DatabaseError {
    /// 接続エラーかどうか
    pub fn is_connection(&self) -> bool {
        matches!(self, DatabaseError::Connection { .. })
    }

    /// クエリエラーかどうか
    pub fn is_query(&self) -> bool {
        matches!(self, DatabaseError::Query { .. })
    }

    /// トランザクションエラーかどうか
    pub fn is_transaction(&self) -> bool {
        matches!(self, DatabaseError::Transaction { .. })
    }

    /// マイグレーションエラーかどうか
    pub fn is_migration(&self) -> bool {
        matches!(self, DatabaseError::Migration { .. })
    }
}

/// I/Oエラー
///
/// ファイル操作時に発生するエラーを表現します。
#[derive(Debug, Error)]
pub enum IoError {
    /// ファイルが見つからない
    #[error("ファイルが見つかりません: {path}")]
    FileNotFound {
        /// ファイルパス
        path: String,
    },

    /// ファイル読み込みエラー
    #[error("ファイルの読み込みに失敗しました: {path} (原因: {cause})")]
    FileRead {
        /// ファイルパス
        path: String,
        /// エラー原因
        cause: String,
    },

    /// ファイル書き込みエラー
    #[error("ファイルの書き込みに失敗しました: {path} (原因: {cause})")]
    FileWrite {
        /// ファイルパス
        path: String,
        /// エラー原因
        cause: String,
    },

    /// ディレクトリ作成エラー
    #[error("ディレクトリの作成に失敗しました: {path} (原因: {cause})")]
    DirectoryCreate {
        /// ディレクトリパス
        path: String,
        /// エラー原因
        cause: String,
    },
}

impl IoError {
    /// ファイルが見つからないエラーかどうか
    pub fn is_file_not_found(&self) -> bool {
        matches!(self, IoError::FileNotFound { .. })
    }

    /// ファイル読み込みエラーかどうか
    pub fn is_file_read(&self) -> bool {
        matches!(self, IoError::FileRead { .. })
    }

    /// ファイル書き込みエラーかどうか
    pub fn is_file_write(&self) -> bool {
        matches!(self, IoError::FileWrite { .. })
    }

    /// ディレクトリ作成エラーかどうか
    pub fn is_directory_create(&self) -> bool {
        matches!(self, IoError::DirectoryCreate { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_syntax_creation() {
        let error = ValidationError::Syntax {
            message: "無効な構文".to_string(),
            location: None,
            suggestion: None,
        };

        assert!(error.is_syntax());
        assert!(!error.is_reference());
        assert!(!error.is_constraint());
    }

    #[test]
    fn test_error_location_format() {
        let location = ErrorLocation {
            table: Some("users".to_string()),
            column: Some("email".to_string()),
            line: Some(42),
        };

        let formatted = location.format();
        assert!(formatted.contains("users"));
        assert!(formatted.contains("email"));
        assert!(formatted.contains("42"));
    }

    #[test]
    fn test_validation_result_operations() {
        let mut result = ValidationResult::new();
        assert!(result.is_valid());

        result.add_error(ValidationError::Syntax {
            message: "エラー".to_string(),
            location: None,
            suggestion: None,
        });

        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
    }

    #[test]
    fn test_migration_error_creation() {
        let error = MigrationError::new("20260121120000".to_string(), "失敗".to_string());

        assert_eq!(error.version(), "20260121120000");
        assert!(!error.has_sql_statement());

        let error_with_sql = MigrationError::with_sql(
            "20260121120000".to_string(),
            "失敗".to_string(),
            "CREATE TABLE users".to_string(),
        );

        assert!(error_with_sql.has_sql_statement());
    }

    #[test]
    fn test_database_error_variants() {
        let conn_error = DatabaseError::Connection {
            message: "接続失敗".to_string(),
            cause: "タイムアウト".to_string(),
        };
        assert!(conn_error.is_connection());

        let query_error = DatabaseError::Query {
            message: "クエリ失敗".to_string(),
            sql: None,
        };
        assert!(query_error.is_query());

        let tx_error = DatabaseError::Transaction {
            message: "トランザクション失敗".to_string(),
        };
        assert!(tx_error.is_transaction());
    }

    #[test]
    fn test_io_error_variants() {
        let not_found = IoError::FileNotFound {
            path: "/path/to/file".to_string(),
        };
        assert!(not_found.is_file_not_found());

        let read_error = IoError::FileRead {
            path: "/path/to/file".to_string(),
            cause: "Permission denied".to_string(),
        };
        assert!(read_error.is_file_read());

        let write_error = IoError::FileWrite {
            path: "/path/to/file".to_string(),
            cause: "Disk full".to_string(),
        };
        assert!(write_error.is_file_write());

        let dir_error = IoError::DirectoryCreate {
            path: "/path/to/dir".to_string(),
            cause: "Permission denied".to_string(),
        };
        assert!(dir_error.is_directory_create());
    }
}
