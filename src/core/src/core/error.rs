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
    /// リネーム元カラムが存在しない警告
    OldColumnNotFound,
    /// 外部キー参照カラムのリネーム警告
    ForeignKeyReference,
    /// renamed_from属性削除推奨警告
    RenamedFromRemoveRecommendation,
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

    /// 外部キー参照カラムのリネーム警告を作成
    pub fn foreign_key_reference(message: String, location: Option<ErrorLocation>) -> Self {
        Self::new(message, location, WarningKind::ForeignKeyReference)
    }

    /// 旧カラム不存在警告を作成
    pub fn old_column_not_found(message: String, location: Option<ErrorLocation>) -> Self {
        Self::new(message, location, WarningKind::OldColumnNotFound)
    }

    /// renamed_from属性削除推奨警告を作成
    pub fn renamed_from_remove_recommendation(
        message: String,
        location: Option<ErrorLocation>,
    ) -> Self {
        Self::new(
            message,
            location,
            WarningKind::RenamedFromRemoveRecommendation,
        )
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

    /// テーブル名とカラム名を指定してエラー位置を作成
    pub fn with_table_and_column(table: &str, column: &str) -> Self {
        Self {
            table: Some(table.to_string()),
            column: Some(column.to_string()),
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

    /// 複数のバリデーション結果を一括マージ
    pub fn merge_all(&mut self, results: impl IntoIterator<Item = ValidationResult>) {
        for result in results {
            self.merge(result);
        }
    }

    /// Result型に変換する
    ///
    /// エラーがない場合は `Ok(warnings)` を返し、
    /// エラーがある場合は `Err(errors)` を返します。
    pub fn into_result(self) -> Result<Vec<ValidationWarning>, Vec<ValidationError>> {
        if self.errors.is_empty() {
            Ok(self.warnings)
        } else {
            Err(self.errors)
        }
    }

    /// 全エラーを改行区切りの文字列に変換
    pub fn errors_to_string(&self) -> String {
        self.errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
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

    /// Column rename operation failed
    #[error(
        "Failed to rename column '{old_name}' to '{new_name}' in table '{table_name}': {reason}"
    )]
    RenameColumnFailed {
        /// テーブル名
        table_name: String,
        /// 旧カラム名
        old_name: String,
        /// 新カラム名
        new_name: String,
        /// 失敗理由
        reason: String,
        /// 提案
        suggestion: Option<String>,
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

    /// リネームカラム失敗エラーかどうか
    pub fn is_rename_column_failed(&self) -> bool {
        matches!(self, DatabaseError::RenameColumnFailed { .. })
    }

    /// データベースエラーメッセージからリネーム失敗の原因を解析
    ///
    /// # Arguments
    /// * `error_message` - データベースから返されたエラーメッセージ
    /// * `table_name` - テーブル名
    /// * `old_name` - 旧カラム名
    /// * `new_name` - 新カラム名
    ///
    /// # Returns
    /// リネーム失敗エラー
    pub fn parse_rename_error(
        error_message: &str,
        table_name: &str,
        old_name: &str,
        new_name: &str,
    ) -> DatabaseError {
        let lower_msg = error_message.to_lowercase();

        let (reason, suggestion) = if lower_msg.contains("does not exist")
            || lower_msg.contains("no such column")
            || lower_msg.contains("unknown column")
        {
            (
                format!(
                    "Column '{}' does not exist in table '{}'",
                    old_name, table_name
                ),
                Some(format!(
                    "Check if column '{}' exists or if it was already renamed",
                    old_name
                )),
            )
        } else if lower_msg.contains("permission denied") || lower_msg.contains("access denied") {
            (
                "Insufficient privileges to rename column".to_string(),
                Some("Ensure the database user has ALTER TABLE privileges".to_string()),
            )
        } else if lower_msg.contains("duplicate column") || lower_msg.contains("already exists") {
            (
                format!(
                    "Column '{}' already exists in table '{}'",
                    new_name, table_name
                ),
                Some(format!(
                    "Choose a different name or drop the existing column '{}' first",
                    new_name
                )),
            )
        } else if lower_msg.contains("foreign key") || lower_msg.contains("constraint") {
            (
                "Column is referenced by a foreign key constraint".to_string(),
                Some("Consider dropping or updating the foreign key constraint first".to_string()),
            )
        } else {
            (error_message.to_string(), None)
        };

        DatabaseError::RenameColumnFailed {
            table_name: table_name.to_string(),
            old_name: old_name.to_string(),
            new_name: new_name.to_string(),
            reason,
            suggestion,
        }
    }
}

/// 設定エラー
///
/// 設定ファイルの読み込み・検証時に発生するエラーを表現します。
#[derive(Debug, Error)]
pub enum ConfigError {
    /// バージョン未指定
    #[error("Config file version is not specified")]
    MissingVersion,

    /// 環境設定なし
    #[error("At least one environment configuration is required")]
    NoEnvironments,

    /// 環境が見つからない
    #[error("Environment '{name}' not found. Available environments: {available:?}")]
    EnvironmentNotFound {
        /// 指定された環境名
        name: String,
        /// 利用可能な環境名リスト
        available: Vec<String>,
    },

    /// データベース名未指定
    #[error("Database name is not specified")]
    MissingDatabaseName,

    /// 環境別設定の検証エラー
    #[error("Invalid config for environment '{environment}': {source}")]
    InvalidEnvironment {
        /// 環境名
        environment: String,
        /// 原因
        #[source]
        source: Box<ConfigError>,
    },
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

    #[test]
    fn test_validation_result_into_result_ok() {
        let mut result = ValidationResult::new();
        result.add_warning(ValidationWarning::compatibility(
            "test warning".to_string(),
            None,
        ));
        let converted = result.into_result();
        assert!(converted.is_ok());
        assert_eq!(converted.unwrap().len(), 1);
    }

    #[test]
    fn test_validation_result_into_result_err() {
        let mut result = ValidationResult::new();
        result.add_error(ValidationError::Syntax {
            message: "test error".to_string(),
            location: None,
            suggestion: None,
        });
        let converted = result.into_result();
        assert!(converted.is_err());
        assert_eq!(converted.unwrap_err().len(), 1);
    }

    #[test]
    fn test_validation_result_into_result_empty() {
        let result = ValidationResult::new();
        let converted = result.into_result();
        assert!(converted.is_ok());
        assert!(converted.unwrap().is_empty());
    }

    #[test]
    fn test_validation_result_errors_to_string() {
        let mut result = ValidationResult::new();
        result.add_error(ValidationError::Syntax {
            message: "error1".to_string(),
            location: None,
            suggestion: None,
        });
        result.add_error(ValidationError::Reference {
            message: "error2".to_string(),
            location: None,
            suggestion: None,
        });
        let s = result.errors_to_string();
        assert!(s.contains("error1"));
        assert!(s.contains("error2"));
        assert!(s.contains('\n'));
    }

    #[test]
    fn test_validation_result_errors_to_string_empty() {
        let result = ValidationResult::new();
        assert!(result.errors_to_string().is_empty());
    }
}
