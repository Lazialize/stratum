/// スキーマチェックサム計算のテスト
///
/// SHA-256ハッシュ計算と正規化されたスキーマ表現が正しく動作することを確認します。
#[cfg(test)]
mod schema_checksum_tests {
    use strata::core::schema::{Column, ColumnType, Constraint, Index, Schema, Table};
    use strata::services::schema_checksum::SchemaChecksumService;

    /// チェックサムサービスの作成テスト
    #[test]
    fn test_new_service() {
        let service = SchemaChecksumService::new();
        assert!(format!("{:?}", service).contains("SchemaChecksumService"));
    }

    /// 空のスキーマのチェックサム計算テスト
    #[test]
    fn test_calculate_checksum_empty_schema() {
        let schema = Schema::new("1.0".to_string());
        let service = SchemaChecksumService::new();
        let checksum = service.calculate_checksum(&schema);

        // チェックサムはSHA-256ハッシュ（64文字の16進数文字列）
        assert_eq!(checksum.len(), 64);
        assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// 単一テーブルのチェックサム計算テスト
    #[test]
    fn test_calculate_checksum_single_table() {
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

        let service = SchemaChecksumService::new();
        let checksum = service.calculate_checksum(&schema);

        // チェックサムは64文字の16進数文字列
        assert_eq!(checksum.len(), 64);
        assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// 同じスキーマのチェックサムは同じであることのテスト
    #[test]
    fn test_checksum_deterministic() {
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

        schema.add_table(table);

        let service = SchemaChecksumService::new();
        let checksum1 = service.calculate_checksum(&schema);
        let checksum2 = service.calculate_checksum(&schema);

        // 同じスキーマは常に同じチェックサムを生成
        assert_eq!(checksum1, checksum2);
    }

    /// 異なるスキーマのチェックサムは異なることのテスト
    #[test]
    fn test_checksum_different_schemas() {
        let service = SchemaChecksumService::new();

        // スキーマ1
        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table1.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema1.add_table(table1);

        // スキーマ2（カラム名が異なる）
        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        table2.add_column(Column::new(
            "user_id".to_string(), // 異なるカラム名
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table2.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["user_id".to_string()],
        });
        schema2.add_table(table2);

        let checksum1 = service.calculate_checksum(&schema1);
        let checksum2 = service.calculate_checksum(&schema2);

        // 異なるスキーマは異なるチェックサムを生成
        assert_ne!(checksum1, checksum2);
    }

    /// チェックサム比較のテスト
    #[test]
    fn test_compare_checksums_equal() {
        let service = SchemaChecksumService::new();

        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema.add_table(table);

        let checksum1 = service.calculate_checksum(&schema);
        let checksum2 = service.calculate_checksum(&schema);

        assert!(service.compare_checksums(&checksum1, &checksum2));
    }

    /// チェックサム比較のテスト（異なる場合）
    #[test]
    fn test_compare_checksums_different() {
        let service = SchemaChecksumService::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("posts".to_string()); // 異なるテーブル名
        table2.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(table2);

        let checksum1 = service.calculate_checksum(&schema1);
        let checksum2 = service.calculate_checksum(&schema2);

        assert!(!service.compare_checksums(&checksum1, &checksum2));
    }

    /// 複雑なスキーマのチェックサム計算テスト
    #[test]
    fn test_calculate_checksum_complex_schema() {
        let mut schema = Schema::new("1.0".to_string());

        // usersテーブル
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
        users_table.add_column(Column::new(
            "created_at".to_string(),
            ColumnType::TIMESTAMP {
                with_time_zone: Some(true),
            },
            false,
        ));
        users_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        users_table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            true,
        ));
        schema.add_table(users_table);

        // postsテーブル
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
        posts_table.add_column(Column::new(
            "title".to_string(),
            ColumnType::VARCHAR { length: 200 },
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

        let service = SchemaChecksumService::new();
        let checksum = service.calculate_checksum(&schema);

        // チェックサムが正しく計算される
        assert_eq!(checksum.len(), 64);
        assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// 正規化されたスキーマ表現の生成テスト
    #[test]
    fn test_normalize_schema() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let service = SchemaChecksumService::new();
        let normalized = service.normalize_schema(&schema);

        // 正規化された表現は空ではない
        assert!(!normalized.is_empty());

        // JSON形式であることを確認（簡易チェック）
        assert!(normalized.contains("users"));
        assert!(normalized.contains("id"));
        assert!(normalized.contains("email"));
    }

    /// テーブル順序の異なるスキーマのチェックサムテスト
    #[test]
    fn test_checksum_table_order_independence() {
        let service = SchemaChecksumService::new();

        // スキーマ1: users, posts の順
        let mut schema1 = Schema::new("1.0".to_string());
        let mut users_table = Table::new("users".to_string());
        users_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(users_table);

        let mut posts_table = Table::new("posts".to_string());
        posts_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(posts_table);

        // スキーマ2: posts, users の順
        let mut schema2 = Schema::new("1.0".to_string());
        let mut posts_table2 = Table::new("posts".to_string());
        posts_table2.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(posts_table2);

        let mut users_table2 = Table::new("users".to_string());
        users_table2.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(users_table2);

        let checksum1 = service.calculate_checksum(&schema1);
        let checksum2 = service.calculate_checksum(&schema2);

        // 正規化により、テーブルの順序に関わらず同じチェックサムになる
        assert_eq!(checksum1, checksum2);
    }
}
