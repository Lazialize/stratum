/// スキーマバリデーターのテスト
///
/// スキーマ定義の検証機能が正しく動作することを確認します。

#[cfg(test)]
mod schema_validator_tests {
    use stratum::core::error::ValidationError;
    use stratum::core::schema::{
        Column, ColumnType, Constraint, Index, Schema, Table,
    };
    use stratum::services::schema_validator::SchemaValidatorService;

    /// 有効なスキーマの検証テスト
    #[test]
    fn test_validate_valid_schema() {
        let mut schema = Schema::new("1.0".to_string());

        let mut users_table = Table::new("users".to_string());
        users_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        users_table.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        users_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(users_table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    /// プライマリキーが存在しないテーブルの検証テスト
    #[test]
    fn test_validate_table_without_primary_key() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        // プライマリキー制約を追加しない

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);

        // Check for primary key error in result
        let has_pk_error = result.errors.iter().any(|e| match e {
            ValidationError::Constraint { message, .. } => {
                message.contains("primary key") || message.contains("PRIMARY KEY")
            }
            _ => false,
        });
        assert!(has_pk_error);
    }

    /// Test validation of table without columns
    #[test]
    fn test_validate_table_without_columns() {
        let mut schema = Schema::new("1.0".to_string());

        let table = Table::new("empty_table".to_string());
        // カラムを追加しない

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);

        // Check for column error in result
        let has_column_error = result.errors.iter().any(|e| match e {
            ValidationError::Constraint { message, .. } => {
                message.contains("column") || message.contains("Column")
            }
            _ => false,
        });
        assert!(has_column_error);
    }

    /// Test validation of foreign key reference to non-existent table
    #[test]
    fn test_validate_foreign_key_reference_not_found() {
        let mut schema = Schema::new("1.0".to_string());

        // usersテーブルを作成
        let mut users_table = Table::new("users".to_string());
        users_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        users_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(users_table);

        // postsテーブルを作成（存在しないテーブルを参照）
        let mut posts_table = Table::new("posts".to_string());
        posts_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts_table.add_column(Column::new(
            "author_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        // 存在しない "authors" テーブルを参照
        posts_table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["author_id".to_string()],
            referenced_table: "authors".to_string(),
            referenced_columns: vec!["id".to_string()],
        });
        schema.add_table(posts_table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);

        // Check for reference error in result
        let has_reference_error = result.errors.iter().any(|e| match e {
            ValidationError::Reference { message, .. } => {
                message.contains("authors") || message.contains("does not exist")
            }
            _ => false,
        });
        assert!(has_reference_error);
    }

    /// Test validation of foreign key reference to non-existent column
    #[test]
    fn test_validate_foreign_key_column_not_found() {
        let mut schema = Schema::new("1.0".to_string());

        // usersテーブルを作成
        let mut users_table = Table::new("users".to_string());
        users_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        users_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(users_table);

        // postsテーブルを作成（存在しないカラムを参照）
        let mut posts_table = Table::new("posts".to_string());
        posts_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts_table.add_column(Column::new(
            "user_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        // 存在しない "uuid" カラムを参照
        posts_table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["uuid".to_string()],
        });
        schema.add_table(posts_table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);

        // Check for reference error in result
        let has_reference_error = result.errors.iter().any(|e| match e {
            ValidationError::Reference { message, .. } => {
                message.contains("uuid") || message.contains("column")
            }
            _ => false,
        });
        assert!(has_reference_error);
    }

    /// Test validation of index with non-existent column
    #[test]
    fn test_validate_index_column_not_found() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        // 存在しない "email" カラムにインデックスを追加
        table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            false,
        ));

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);

        // Check for reference error in result
        let has_reference_error = result.errors.iter().any(|e| match e {
            ValidationError::Reference { message, .. } => {
                message.contains("email") || message.contains("Index")
            }
            _ => false,
        });
        assert!(has_reference_error);
    }

    /// Test validation of constraint with non-existent column
    #[test]
    fn test_validate_constraint_column_not_found() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        // 存在しない "nonexistent" カラムにプライマリキー制約を追加
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["nonexistent".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);

        // Check for reference error in result
        let has_reference_error = result.errors.iter().any(|e| match e {
            ValidationError::Reference { message, .. } => {
                message.contains("nonexistent") || message.contains("column")
            }
            _ => false,
        });
        assert!(has_reference_error);
    }

    /// Test detection of multiple errors
    #[test]
    fn test_validate_multiple_errors() {
        let mut schema = Schema::new("1.0".to_string());

        // エラー1: カラムなし
        let table1 = Table::new("empty_table".to_string());
        schema.add_table(table1);

        // エラー2: プライマリキーなし
        let mut table2 = Table::new("no_pk_table".to_string());
        table2.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema.add_table(table2);

        // エラー3: 存在しないテーブルを参照
        let mut table3 = Table::new("bad_fk_table".to_string());
        table3.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table3.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        table3.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["id".to_string()],
            referenced_table: "nonexistent".to_string(),
            referenced_columns: vec!["id".to_string()],
        });
        schema.add_table(table3);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        // 少なくとも3つのエラーがあるはず
        assert!(result.error_count() >= 3);
    }

    /// 空のスキーマの検証テスト
    #[test]
    fn test_validate_empty_schema() {
        let schema = Schema::new("1.0".to_string());

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // 空のスキーマは有効
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    /// 有効な外部キー制約のテスト
    #[test]
    fn test_validate_valid_foreign_key() {
        let mut schema = Schema::new("1.0".to_string());

        // usersテーブル
        let mut users_table = Table::new("users".to_string());
        users_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        users_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(users_table);

        // postsテーブル（正しい外部キー）
        let mut posts_table = Table::new("posts".to_string());
        posts_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts_table.add_column(Column::new(
            "user_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        posts_table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
        });
        schema.add_table(posts_table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }
}
