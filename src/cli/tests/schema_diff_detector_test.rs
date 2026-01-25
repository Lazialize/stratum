/// スキーマ差分検出サービスのテスト
///
/// スキーマ間の差分を正しく検出することを確認します。
#[cfg(test)]
mod schema_diff_detector_tests {
    use strata::core::schema::{Column, ColumnType, Constraint, Index, Schema, Table};
    use strata::services::schema_diff_detector::SchemaDiffDetector;

    /// サービスの作成テスト
    #[test]
    fn test_new_service() {
        let service = SchemaDiffDetector::new();
        assert!(format!("{:?}", service).contains("SchemaDiffDetector"));
    }

    /// 同じスキーマの比較（差分なし）
    #[test]
    fn test_detect_diff_no_changes() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(table.clone());

        let mut schema2 = Schema::new("1.0".to_string());
        schema2.add_table(table);

        let diff = service.detect_diff(&schema1, &schema2);

        assert!(diff.is_empty());
    }

    /// テーブル追加の検出
    #[test]
    fn test_detect_table_added() {
        let service = SchemaDiffDetector::new();

        let schema1 = Schema::new("1.0".to_string());

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(table);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.added_tables.len(), 1);
        assert_eq!(diff.added_tables[0].name, "users");
        assert_eq!(diff.removed_tables.len(), 0);
        assert_eq!(diff.modified_tables.len(), 0);
    }

    /// テーブル削除の検出
    #[test]
    fn test_detect_table_removed() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(table);

        let schema2 = Schema::new("1.0".to_string());

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.added_tables.len(), 0);
        assert_eq!(diff.removed_tables.len(), 1);
        assert_eq!(diff.removed_tables[0], "users");
        assert_eq!(diff.modified_tables.len(), 0);
    }

    /// 複数テーブルの追加と削除
    #[test]
    fn test_detect_multiple_table_changes() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut old_table = Table::new("old_table".to_string());
        old_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(old_table);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut new_table = Table::new("new_table".to_string());
        new_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(new_table);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.added_tables.len(), 1);
        assert_eq!(diff.added_tables[0].name, "new_table");
        assert_eq!(diff.removed_tables.len(), 1);
        assert_eq!(diff.removed_tables[0], "old_table");
    }

    /// カラム追加の検出
    #[test]
    fn test_detect_column_added() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        table2.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table2.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.table_name, "users");
        assert_eq!(table_diff.added_columns.len(), 1);
        assert_eq!(table_diff.added_columns[0].name, "email");
    }

    /// カラム削除の検出
    #[test]
    fn test_detect_column_removed() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table1.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        table2.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.removed_columns.len(), 1);
        assert_eq!(table_diff.removed_columns[0], "email");
    }

    /// カラム変更の検出
    #[test]
    fn test_detect_column_modified() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        table2.add_column(Column::new(
            "age".to_string(),
            ColumnType::INTEGER { precision: None },
            true, // nullable changed
        ));
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.modified_columns.len(), 1);
        assert_eq!(table_diff.modified_columns[0].column_name, "age");
    }

    /// インデックス追加の検出
    #[test]
    fn test_detect_index_added() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        table2.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table2.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            false,
        ));
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.added_indexes.len(), 1);
        assert_eq!(table_diff.added_indexes[0].name, "idx_email");
    }

    /// インデックス削除の検出
    #[test]
    fn test_detect_index_removed() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table1.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        table2.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.removed_indexes.len(), 1);
        assert_eq!(table_diff.removed_indexes[0], "idx_email");
    }

    /// 制約追加の検出
    #[test]
    fn test_detect_constraint_added() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        table2.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table2.add_constraint(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.added_constraints.len(), 1);
    }

    /// 制約削除の検出
    #[test]
    fn test_detect_constraint_removed() {
        let service = SchemaDiffDetector::new();

        let mut schema1 = Schema::new("1.0".to_string());
        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table1.add_constraint(Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        });
        schema1.add_table(table1);

        let mut schema2 = Schema::new("1.0".to_string());
        let mut table2 = Table::new("users".to_string());
        table2.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        schema2.add_table(table2);

        let diff = service.detect_diff(&schema1, &schema2);

        assert_eq!(diff.modified_tables.len(), 1);
        let table_diff = &diff.modified_tables[0];
        assert_eq!(table_diff.removed_constraints.len(), 1);
    }

    /// 複雑なスキーマ変更の検出
    #[test]
    fn test_detect_complex_changes() {
        let service = SchemaDiffDetector::new();

        // Schema 1: users, posts tables
        let mut schema1 = Schema::new("1.0".to_string());

        let mut users1 = Table::new("users".to_string());
        users1.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        users1.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        schema1.add_table(users1);

        let mut posts1 = Table::new("posts".to_string());
        posts1.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema1.add_table(posts1);

        // Schema 2: users (modified), comments (new), posts removed
        let mut schema2 = Schema::new("1.0".to_string());

        let mut users2 = Table::new("users".to_string());
        users2.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        users2.add_column(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));
        users2.add_column(Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        )); // new column
        schema2.add_table(users2);

        let mut comments = Table::new("comments".to_string());
        comments.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema2.add_table(comments);

        let diff = service.detect_diff(&schema1, &schema2);

        // 1 table added (comments), 1 removed (posts), 1 modified (users)
        assert_eq!(diff.added_tables.len(), 1);
        assert_eq!(diff.added_tables[0].name, "comments");
        assert_eq!(diff.removed_tables.len(), 1);
        assert_eq!(diff.removed_tables[0], "posts");
        assert_eq!(diff.modified_tables.len(), 1);
        assert_eq!(diff.modified_tables[0].table_name, "users");
        assert_eq!(diff.modified_tables[0].added_columns.len(), 1);
    }
}
