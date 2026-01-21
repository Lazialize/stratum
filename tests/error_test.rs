/// エラー型のテスト
///
/// カスタムエラー型が正しく動作し、適切なエラーメッセージを生成することを確認します。

#[cfg(test)]
mod error_tests {
    use stratum::core::error::{
        DatabaseError, ErrorLocation, IoError, MigrationError, ValidationError, ValidationResult,
    };
    use std::io;

    /// ValidationError::Syntax のテスト
    #[test]
    fn test_validation_error_syntax() {
        let error = ValidationError::Syntax {
            message: "無効なYAML構文".to_string(),
            location: Some(ErrorLocation {
                table: Some("users".to_string()),
                column: None,
                line: Some(42),
            }),
            suggestion: Some("正しいYAML形式で記述してください".to_string()),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("無効なYAML構文"));
        assert!(error.is_syntax());
        assert!(!error.is_reference());
    }

    /// ValidationError::Reference のテスト
    #[test]
    fn test_validation_error_reference() {
        let error = ValidationError::Reference {
            message: "参照先テーブルが存在しません".to_string(),
            location: Some(ErrorLocation {
                table: Some("posts".to_string()),
                column: Some("user_id".to_string()),
                line: None,
            }),
            suggestion: Some("テーブル 'users' を定義してください".to_string()),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("参照先テーブルが存在しません"));
        assert!(error.is_reference());
        assert!(!error.is_constraint());
    }

    /// ValidationError::Constraint のテスト
    #[test]
    fn test_validation_error_constraint() {
        let error = ValidationError::Constraint {
            message: "プライマリキーが定義されていません".to_string(),
            location: Some(ErrorLocation {
                table: Some("users".to_string()),
                column: None,
                line: Some(10),
            }),
            suggestion: None,
        };

        let error_str = error.to_string();
        assert!(error_str.contains("プライマリキーが定義されていません"));
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

    /// ValidationResult のテスト
    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::new();
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);

        result.add_error(ValidationError::Syntax {
            message: "エラー1".to_string(),
            location: None,
            suggestion: None,
        });

        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);

        result.add_error(ValidationError::Reference {
            message: "エラー2".to_string(),
            location: None,
            suggestion: None,
        });

        assert_eq!(result.error_count(), 2);
    }

    /// MigrationError のテスト
    #[test]
    fn test_migration_error() {
        let error = MigrationError {
            version: "20260121120000".to_string(),
            error: "テーブルが既に存在します".to_string(),
            sql_statement: Some("CREATE TABLE users (id INTEGER);".to_string()),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("20260121120000"));
        assert!(error_str.contains("テーブルが既に存在します"));

        assert_eq!(error.version(), "20260121120000");
        assert!(error.has_sql_statement());
    }

    /// DatabaseError のテスト
    #[test]
    fn test_database_error_connection() {
        let error = DatabaseError::Connection {
            message: "データベースに接続できません".to_string(),
            cause: "タイムアウト".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("データベースに接続できません"));
        assert!(error.is_connection());
        assert!(!error.is_query());
    }

    /// DatabaseError::Query のテスト
    #[test]
    fn test_database_error_query() {
        let error = DatabaseError::Query {
            message: "クエリの実行に失敗しました".to_string(),
            sql: Some("SELECT * FROM users WHERE id = $1".to_string()),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("クエリの実行に失敗しました"));
        assert!(error.is_query());
    }

    /// DatabaseError::Transaction のテスト
    #[test]
    fn test_database_error_transaction() {
        let error = DatabaseError::Transaction {
            message: "トランザクションのコミットに失敗しました".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("トランザクションのコミットに失敗しました"));
        assert!(error.is_transaction());
    }

    /// DatabaseError::Migration のテスト
    #[test]
    fn test_database_error_migration() {
        let migration_error = MigrationError {
            version: "20260121120000".to_string(),
            error: "マイグレーション失敗".to_string(),
            sql_statement: None,
        };

        let error = DatabaseError::Migration {
            error: migration_error,
        };

        assert!(error.is_migration());
    }

    /// IoError のテスト
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

    /// IoError::FileWrite のテスト
    #[test]
    fn test_io_error_file_write() {
        let error = IoError::FileWrite {
            path: "/path/to/output.yaml".to_string(),
            cause: "ディスク容量不足".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("/path/to/output.yaml"));
        assert!(error.is_file_write());
    }

    /// IoError::DirectoryCreate のテスト
    #[test]
    fn test_io_error_directory_create() {
        let error = IoError::DirectoryCreate {
            path: "/path/to/migrations".to_string(),
            cause: "権限がありません".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("/path/to/migrations"));
        assert!(error.is_directory_create());
    }

    /// エラーメッセージの日本語対応確認
    #[test]
    fn test_error_messages_in_japanese() {
        let validation_error = ValidationError::Syntax {
            message: "YAMLファイルの解析に失敗しました".to_string(),
            location: None,
            suggestion: Some("インデントを確認してください".to_string()),
        };

        let error_str = validation_error.to_string();
        assert!(error_str.contains("YAMLファイルの解析に失敗しました"));

        let db_error = DatabaseError::Connection {
            message: "PostgreSQLサーバーに接続できません".to_string(),
            cause: "接続タイムアウト".to_string(),
        };

        let error_str = db_error.to_string();
        assert!(error_str.contains("PostgreSQLサーバーに接続できません"));
    }

    /// ValidationResultのマージ機能のテスト
    #[test]
    fn test_validation_result_merge() {
        let mut result1 = ValidationResult::new();
        result1.add_error(ValidationError::Syntax {
            message: "エラー1".to_string(),
            location: None,
            suggestion: None,
        });

        let mut result2 = ValidationResult::new();
        result2.add_error(ValidationError::Reference {
            message: "エラー2".to_string(),
            location: None,
            suggestion: None,
        });

        result1.merge(result2);
        assert_eq!(result1.error_count(), 2);
        assert!(!result1.is_valid());
    }
}
