/// エラー型のテスト
///
/// カスタムエラー型が正しく動作し、適切なエラーメッセージを生成することを確認します。

#[cfg(test)]
mod error_tests {
    use std::io;
    use strata::core::error::{
        DatabaseError, ErrorLocation, IoError, MigrationError, ValidationError, ValidationResult,
    };

    /// ValidationError::Syntax test
    #[test]
    fn test_validation_error_syntax() {
        let error = ValidationError::Syntax {
            message: "Invalid YAML syntax".to_string(),
            location: Some(ErrorLocation {
                table: Some("users".to_string()),
                column: None,
                line: Some(42),
            }),
            suggestion: Some("Please write in correct YAML format".to_string()),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Invalid YAML syntax"));
        assert!(error.is_syntax());
        assert!(!error.is_reference());
    }

    /// ValidationError::Reference test
    #[test]
    fn test_validation_error_reference() {
        let error = ValidationError::Reference {
            message: "Referenced table does not exist".to_string(),
            location: Some(ErrorLocation {
                table: Some("posts".to_string()),
                column: Some("user_id".to_string()),
                line: None,
            }),
            suggestion: Some("Define table 'users'".to_string()),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Referenced table does not exist"));
        assert!(error.is_reference());
        assert!(!error.is_constraint());
    }

    /// ValidationError::Constraint test
    #[test]
    fn test_validation_error_constraint() {
        let error = ValidationError::Constraint {
            message: "Primary key is not defined".to_string(),
            location: Some(ErrorLocation {
                table: Some("users".to_string()),
                column: None,
                line: Some(10),
            }),
            suggestion: None,
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Primary key is not defined"));
        assert!(error.is_constraint());
    }

    /// ErrorLocation の作成とフォーマットのテスト
    #[test]
    fn test_error_location() {
        let location = ErrorLocation {
            table: Some("users".to_string()),
            column: Some("email".to_string()),
            line: Some(25),
        };

        assert_eq!(location.table.as_deref(), Some("users"));
        assert_eq!(location.column.as_deref(), Some("email"));
        assert_eq!(location.line, Some(25));

        let formatted = location.format();
        assert!(formatted.contains("users"));
        assert!(formatted.contains("email"));
        assert!(formatted.contains("25"));
    }

    /// ValidationResult test
    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::new();
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);

        result.add_error(ValidationError::Syntax {
            message: "Error 1".to_string(),
            location: None,
            suggestion: None,
        });

        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);

        result.add_error(ValidationError::Reference {
            message: "Error 2".to_string(),
            location: None,
            suggestion: None,
        });

        assert_eq!(result.error_count(), 2);
    }

    /// MigrationError test
    #[test]
    fn test_migration_error() {
        let error = MigrationError {
            version: "20260121120000".to_string(),
            error: "Table already exists".to_string(),
            sql_statement: Some("CREATE TABLE users (id INTEGER);".to_string()),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("20260121120000"));
        assert!(error_str.contains("Table already exists"));

        assert_eq!(error.version(), "20260121120000");
        assert!(error.has_sql_statement());
    }

    /// DatabaseError test
    #[test]
    fn test_database_error_connection() {
        let error = DatabaseError::Connection {
            message: "Cannot connect to database".to_string(),
            cause: "Timeout".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Cannot connect to database"));
        assert!(error.is_connection());
        assert!(!error.is_query());
    }

    /// DatabaseError::Query test
    #[test]
    fn test_database_error_query() {
        let error = DatabaseError::Query {
            message: "Query execution failed".to_string(),
            sql: Some("SELECT * FROM users WHERE id = $1".to_string()),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Query execution failed"));
        assert!(error.is_query());
    }

    /// DatabaseError::Transaction test
    #[test]
    fn test_database_error_transaction() {
        let error = DatabaseError::Transaction {
            message: "Transaction commit failed".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Transaction commit failed"));
        assert!(error.is_transaction());
    }

    /// DatabaseError::Migration test
    #[test]
    fn test_database_error_migration() {
        let migration_error = MigrationError {
            version: "20260121120000".to_string(),
            error: "Migration failed".to_string(),
            sql_statement: None,
        };

        let error = DatabaseError::Migration {
            error: migration_error,
        };

        assert!(error.is_migration());
    }

    /// IoError test
    #[test]
    fn test_io_error_file_not_found() {
        let error = IoError::FileNotFound {
            path: "/path/to/schema.yaml".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("/path/to/schema.yaml"));
        assert!(error.is_file_not_found());
    }

    /// IoError::FileRead のテスト
    #[test]
    fn test_io_error_file_read() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Permission denied");
        let error = IoError::FileRead {
            path: "/path/to/file.yaml".to_string(),
            cause: io_error.to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("/path/to/file.yaml"));
        assert!(error.is_file_read());
    }

    /// IoError::FileWrite test
    #[test]
    fn test_io_error_file_write() {
        let error = IoError::FileWrite {
            path: "/path/to/output.yaml".to_string(),
            cause: "Insufficient disk space".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("/path/to/output.yaml"));
        assert!(error.is_file_write());
    }

    /// IoError::DirectoryCreate test
    #[test]
    fn test_io_error_directory_create() {
        let error = IoError::DirectoryCreate {
            path: "/path/to/migrations".to_string(),
            cause: "Permission denied".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("/path/to/migrations"));
        assert!(error.is_directory_create());
    }

    /// Error message internationalization test
    #[test]
    fn test_error_messages() {
        let validation_error = ValidationError::Syntax {
            message: "Failed to parse YAML file".to_string(),
            location: None,
            suggestion: Some("Check the indentation".to_string()),
        };

        let error_str = validation_error.to_string();
        assert!(error_str.contains("Failed to parse YAML file"));

        let db_error = DatabaseError::Connection {
            message: "Cannot connect to PostgreSQL server".to_string(),
            cause: "Connection timeout".to_string(),
        };

        let error_str = db_error.to_string();
        assert!(error_str.contains("Cannot connect to PostgreSQL server"));
    }

    /// ValidationResult merge functionality test
    #[test]
    fn test_validation_result_merge() {
        let mut result1 = ValidationResult::new();
        result1.add_error(ValidationError::Syntax {
            message: "Error 1".to_string(),
            location: None,
            suggestion: None,
        });

        let mut result2 = ValidationResult::new();
        result2.add_error(ValidationError::Reference {
            message: "Error 2".to_string(),
            location: None,
            suggestion: None,
        });

        result1.merge(result2);
        assert_eq!(result1.error_count(), 2);
        assert!(!result1.is_valid());
    }
}
