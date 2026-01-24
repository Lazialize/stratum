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
    /// Syntax error
    #[error("Syntax error: {message}{}", format_location_opt(.location))]
    Syntax {
        /// エラーメッセージ
        message: String,
        /// エラー発生位置
        location: Option<ErrorLocation>,
        /// 修正提案
        suggestion: Option<String>,
    },

    /// Reference error
    #[error("Reference error: {message}{}", format_location_opt(.location))]
    Reference {
        /// エラーメッセージ
        message: String,
        /// エラー発生位置
        location: Option<ErrorLocation>,
        /// 修正提案
        suggestion: Option<String>,
    },

    /// Constraint error
    #[error("Constraint error: {message}{}", format_location_opt(.location))]
    Constraint {
        /// エラーメッセージ
        message: String,
        /// エラー発生位置
        location: Option<ErrorLocation>,
        /// 修正提案
        suggestion: Option<String>,
    },

    /// Type conversion error (incompatible type change)
    #[error("Type conversion error: {message}{}", format_location_opt(.location))]
    TypeConversion {
        /// エラーメッセージ
        message: String,
        /// エラー発生位置
        location: Option<ErrorLocation>,
        /// 修正提案
        suggestion: Option<String>,
    },

    /// Dialect constraint error (type not supported in specific database)
    #[error("Dialect constraint error ({dialect}): {message}{}", format_location_opt(.location))]
    DialectConstraint {
        /// エラーメッセージ
        message: String,
        /// エラー発生位置
        location: Option<ErrorLocation>,
        /// 対象のデータベース方言
        dialect: String,
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

    /// 型変換エラーかどうか
    pub fn is_type_conversion(&self) -> bool {
        matches!(self, ValidationError::TypeConversion { .. })
    }

    /// 方言制約エラーかどうか
    pub fn is_dialect_constraint(&self) -> bool {
        matches!(self, ValidationError::DialectConstraint { .. })
    }

    /// エラー発生位置を取得
    pub fn location(&self) -> Option<&ErrorLocation> {
        match self {
            ValidationError::Syntax { location, .. }
            | ValidationError::Reference { location, .. }
            | ValidationError::Constraint { location, .. }
            | ValidationError::TypeConversion { location, .. }
            | ValidationError::DialectConstraint { location, .. } => location.as_ref(),
        }
    }

    /// 修正提案を取得
    pub fn suggestion(&self) -> Option<&str> {
        match self {
            ValidationError::Syntax { suggestion, .. }
            | ValidationError::Reference { suggestion, .. }
            | ValidationError::Constraint { suggestion, .. }
            | ValidationError::TypeConversion { suggestion, .. } => suggestion.as_deref(),
            ValidationError::DialectConstraint { .. } => None,
        }
    }
}

/// バリデーション警告
///
/// スキーマ定義の検証時に発生する警告を表現します。
/// エラーではないが、ユーザーに注意を促すべき事項を表します。
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationWarning {
    /// 警告メッセージ
    pub message: String,
    /// 警告発生位置
    pub location: Option<ErrorLocation>,
    /// 警告の種類
    pub kind: WarningKind,
}

/// 警告の種類
#[derive(Debug, Clone, PartialEq)]
pub enum WarningKind {
    /// 方言固有の機能に関する警告（フォールバックなど）
    DialectSpecific,
    /// 精度損失の可能性に関する警告
    PrecisionLoss,
    /// 互換性に関する警告
    Compatibility,
    /// データ損失の可能性に関する警告（型変更時）
    DataLoss,
}

impl ValidationWarning {
    /// 新しい警告を作成
    pub fn new(message: String, location: Option<ErrorLocation>, kind: WarningKind) -> Self {
        Self {
            message,
            location,
            kind,
        }
    }

    /// 方言固有の警告を作成
    pub fn dialect_specific(message: String, location: Option<ErrorLocation>) -> Self {
        Self::new(message, location, WarningKind::DialectSpecific)
    }

    /// 精度損失の警告を作成
    pub fn precision_loss(message: String, location: Option<ErrorLocation>) -> Self {
        Self::new(message, location, WarningKind::PrecisionLoss)
    }

    /// データ損失の警告を作成
    pub fn data_loss(message: String, location: Option<ErrorLocation>) -> Self {
        Self::new(message, location, WarningKind::DataLoss)
    }

    /// 互換性の警告を作成
    pub fn compatibility(message: String, location: Option<ErrorLocation>) -> Self {
        Self::new(message, location, WarningKind::Compatibility)
    }

    /// 位置情報をフォーマット
    pub fn format(&self) -> String {
        let location_str = self
            .location
            .as_ref()
            .map_or(String::new(), |loc| loc.format());
        format!("Warning: {}{}", self.message, location_str)
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
            parts.push(format!("table: {}", table));
        }
        if let Some(column) = &self.column {
            parts.push(format!("column: {}", column));
        }
        if let Some(line) = self.line {
            parts.push(format!("line: {}", line));
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
    /// 警告のリスト
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    /// 新しいバリデーション結果を作成
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// エラーを追加
    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// 警告を追加
    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    /// 検証が成功したかどうか（エラーがない場合は成功）
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// エラーの数を取得
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// 警告の数を取得
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// 他のバリデーション結果をマージ
    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
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
#[error("Migration {version} failed: {error}")]
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
    /// Connection error
    #[error("Database connection error: {message} (cause: {cause})")]
    Connection {
        /// エラーメッセージ
        message: String,
        /// エラー原因
        cause: String,
    },

    /// Query execution error
    #[error("Query execution error: {message}")]
    Query {
        /// エラーメッセージ
        message: String,
        /// 失敗したSQL
        sql: Option<String>,
    },

    /// Transaction error
    #[error("Transaction error: {message}")]
    Transaction {
        /// エラーメッセージ
        message: String,
    },

    /// Migration error
    #[error("Migration error: {error}")]
    Migration {
        /// マイグレーションエラー
        error: MigrationError,
    },

    /// Invalid table name error
    #[error("Invalid table name '{name}': {reason}")]
    InvalidTableName {
        /// テーブル名
        name: String,
        /// 不正な理由
        reason: String,
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

    /// テーブル名不正エラーかどうか
    pub fn is_invalid_table_name(&self) -> bool {
        matches!(self, DatabaseError::InvalidTableName { .. })
    }
}

/// I/Oエラー
///
/// ファイル操作時に発生するエラーを表現します。
#[derive(Debug, Error)]
pub enum IoError {
    /// File not found
    #[error("File not found: {path}")]
    FileNotFound {
        /// ファイルパス
        path: String,
    },

    /// File read error
    #[error("Failed to read file: {path} (cause: {cause})")]
    FileRead {
        /// ファイルパス
        path: String,
        /// エラー原因
        cause: String,
    },

    /// File write error
    #[error("Failed to write file: {path} (cause: {cause})")]
    FileWrite {
        /// ファイルパス
        path: String,
        /// エラー原因
        cause: String,
    },

    /// Directory creation error
    #[error("Failed to create directory: {path} (cause: {cause})")]
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
            message: "Invalid syntax".to_string(),
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
            message: "Error".to_string(),
            location: None,
            suggestion: None,
        });

        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
    }

    #[test]
    fn test_migration_error_creation() {
        let error = MigrationError::new("20260121120000".to_string(), "Failed".to_string());

        assert_eq!(error.version(), "20260121120000");
        assert!(!error.has_sql_statement());

        let error_with_sql = MigrationError::with_sql(
            "20260121120000".to_string(),
            "Failed".to_string(),
            "CREATE TABLE users".to_string(),
        );

        assert!(error_with_sql.has_sql_statement());
    }

    #[test]
    fn test_database_error_variants() {
        let conn_error = DatabaseError::Connection {
            message: "Connection failed".to_string(),
            cause: "Timeout".to_string(),
        };
        assert!(conn_error.is_connection());

        let query_error = DatabaseError::Query {
            message: "Query failed".to_string(),
            sql: None,
        };
        assert!(query_error.is_query());

        let tx_error = DatabaseError::Transaction {
            message: "Transaction failed".to_string(),
        };
        assert!(tx_error.is_transaction());

        let invalid_table_error = DatabaseError::InvalidTableName {
            name: "123invalid".to_string(),
            reason: "Table name must start with letter or underscore".to_string(),
        };
        assert!(invalid_table_error.is_invalid_table_name());
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
