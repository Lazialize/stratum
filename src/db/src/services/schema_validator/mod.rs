// スキーマバリデーターサービス
//
// スキーマ定義の整合性、参照整合性、制約の検証を行うサービス。
// テーブル定義、インデックス、外部キー制約などを検証します。

mod column_type_validator;
mod constraint_validator;
mod dialect_validator;
mod enum_validator;
mod index_validator;
mod rename_validator;
mod table_validator;
mod validation_helpers;

use crate::core::config::Dialect;
use crate::core::error::{ValidationError, ValidationResult, ValidationWarning};
use crate::core::schema::Schema;

/// スキーマバリデーターサービス
///
/// スキーマ定義の検証を行います。
#[derive(Debug, Clone)]
pub struct SchemaValidatorService {
    // 将来的な拡張のためのフィールドを予約
}

impl SchemaValidatorService {
    /// 新しいSchemaValidatorServiceを作成
    pub fn new() -> Self {
        Self {}
    }

    /// スキーマ定義の全体的な検証を実行
    ///
    /// # Arguments
    ///
    /// * `schema` - 検証対象のスキーマ
    ///
    /// # Returns
    ///
    /// 検証結果（エラーのリストを含む）
    pub fn validate(&self, schema: &Schema) -> ValidationResult {
        self.validate_internal(schema, None)
    }

    /// スキーマ定義の全体的な検証を実行（方言指定あり）
    ///
    /// # Arguments
    ///
    /// * `schema` - 検証対象のスキーマ
    /// * `dialect` - データベース方言
    pub fn validate_with_dialect(&self, schema: &Schema, dialect: Dialect) -> ValidationResult {
        self.validate_internal(schema, Some(dialect))
    }

    fn validate_internal(&self, schema: &Schema, dialect: Option<Dialect>) -> ValidationResult {
        let mut result = ValidationResult::new();

        // カテゴリ別に検証を実行（Task 5.1）
        result.merge(self.validate_enums(schema, dialect));

        // 空のスキーマは有効
        if schema.table_count() == 0 && schema.enums.is_empty() {
            return result;
        }

        // テーブル構造の検証
        result.merge_all([
            self.validate_table_structure(schema),
            self.validate_duplicate_column_names(schema),
            self.validate_column_types(schema),
            self.validate_primary_keys(schema),
            self.validate_index_references(schema),
            self.validate_constraint_references(schema),
            self.validate_check_expressions(schema),
            self.validate_duplicate_unique_constraints(schema),
        ]);

        result
    }

    /// ENUM定義の検証
    ///
    /// - PostgreSQL以外の方言でENUMが定義されていないか確認
    /// - ENUM値が空でないか確認
    /// - ENUM値に重複がないか確認
    pub fn validate_enums(&self, schema: &Schema, dialect: Option<Dialect>) -> ValidationResult {
        enum_validator::validate_enums(schema, dialect)
    }

    /// テーブル構造の検証（カラムの存在確認）
    fn validate_table_structure(&self, schema: &Schema) -> ValidationResult {
        table_validator::validate_table_structure(schema)
    }

    /// 重複カラム名の検証
    fn validate_duplicate_column_names(&self, schema: &Schema) -> ValidationResult {
        let mut result = ValidationResult::new();

        for (table_name, table) in &schema.tables {
            let mut seen = std::collections::HashSet::new();
            for column in &table.columns {
                if !seen.insert(&column.name) {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "Table '{}' has duplicate column name '{}'",
                            table_name, column.name
                        ),
                        location: Some(crate::core::error::ErrorLocation::with_table_and_column(
                            table_name,
                            &column.name,
                        )),
                        suggestion: Some("Remove the duplicate column definition".to_string()),
                    });
                }
            }
        }

        result
    }

    /// カラム型の検証
    ///
    /// - DECIMAL型の精度とスケールの検証
    /// - CHAR型の長さの検証
    /// - ENUM参照の存在確認
    pub fn validate_column_types(&self, schema: &Schema) -> ValidationResult {
        column_type_validator::validate_column_types(schema)
    }

    /// プライマリキーの存在確認
    pub fn validate_primary_keys(&self, schema: &Schema) -> ValidationResult {
        constraint_validator::validate_primary_keys(schema)
    }

    /// インデックスのカラム参照整合性検証
    pub fn validate_index_references(&self, schema: &Schema) -> ValidationResult {
        index_validator::validate_index_references(schema)
    }

    /// 制約のカラム/テーブル参照整合性検証
    pub fn validate_constraint_references(&self, schema: &Schema) -> ValidationResult {
        constraint_validator::validate_constraint_references(schema)
    }

    /// CHECK制約のexpression空チェック
    pub fn validate_check_expressions(&self, schema: &Schema) -> ValidationResult {
        constraint_validator::validate_check_expressions(schema)
    }

    /// 重複UNIQUE制約チェック
    pub fn validate_duplicate_unique_constraints(&self, schema: &Schema) -> ValidationResult {
        constraint_validator::validate_duplicate_unique_constraints(schema)
    }

    /// 方言固有の警告を生成
    ///
    /// # Arguments
    ///
    /// * `schema` - 検証対象のスキーマ
    /// * `dialect` - 対象データベース方言
    ///
    /// # Returns
    ///
    /// 方言固有の警告のリスト
    pub fn generate_dialect_warnings(
        &self,
        schema: &Schema,
        dialect: &Dialect,
    ) -> Vec<ValidationWarning> {
        dialect_validator::generate_dialect_warnings(schema, dialect)
    }

    /// カラムリネームの検証
    ///
    /// # Arguments
    ///
    /// * `new_schema` - 検証対象の新スキーマ
    ///
    /// # Returns
    ///
    /// 検証結果（エラーと警告を含む）
    ///
    /// Note: 旧カラム存在確認を行う場合は `validate_renames_with_old_schema` を使用してください。
    pub fn validate_renames(&self, new_schema: &Schema) -> ValidationResult {
        rename_validator::validate_renames_internal(new_schema, None)
    }

    /// カラムリネームの検証（旧スキーマ照合あり）
    ///
    /// # Arguments
    ///
    /// * `old_schema` - 旧スキーマ（リネーム元カラムの存在確認用）
    /// * `new_schema` - 検証対象の新スキーマ
    ///
    /// # Returns
    ///
    /// 検証結果（エラーと警告を含む）
    pub fn validate_renames_with_old_schema(
        &self,
        old_schema: &Schema,
        new_schema: &Schema,
    ) -> ValidationResult {
        rename_validator::validate_renames_internal(new_schema, Some(old_schema))
    }

    /// 外部キー制約の参照整合性を検証
    ///
    /// # Arguments
    ///
    /// * `schema` - Schema to validate
    ///
    /// # Returns
    ///
    /// List of referential integrity errors
    pub fn validate_referential_integrity(&self, schema: &Schema) -> Vec<ValidationError> {
        constraint_validator::validate_referential_integrity(schema)
    }
}

impl Default for SchemaValidatorService {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::services::traits::SchemaValidator for SchemaValidatorService {
    fn validate_renames_with_old_schema(
        &self,
        old_schema: &Schema,
        new_schema: &Schema,
    ) -> ValidationResult {
        self.validate_renames_with_old_schema(old_schema, new_schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Dialect;
    use crate::core::schema::{Column, ColumnType, Constraint, EnumDefinition, Index, Table};

    #[test]
    fn test_new_service() {
        let service = SchemaValidatorService::new();
        // サービスが正常に作成されることを確認
        assert!(format!("{:?}", service).contains("SchemaValidatorService"));
    }

    // ===============================================
    // Task 5.2: バリデーション結果の統合テスト
    // ===============================================

    #[test]
    fn test_validate_collects_all_errors() {
        let mut schema = Schema::new("1.0".to_string());

        // Error 1: ENUM with no values
        schema.add_enum(EnumDefinition {
            name: "empty_enum".to_string(),
            values: vec![],
        });

        // Error 2: Table without primary key
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        // Error 3: Index referencing non-existent column
        table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["nonexistent".to_string()],
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // All errors should be collected
        assert!(!result.is_valid());
        assert!(result.error_count() >= 3);
    }

    #[test]
    fn test_validate_with_dialect_returns_warnings_separately() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        // UUID type will generate warning for SQLite
        table.add_column(Column::new("uuid".to_string(), ColumnType::UUID, false));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();

        // Validate without errors
        let result = validator.validate(&schema);
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);

        // Generate dialect warnings separately
        let warnings = validator.generate_dialect_warnings(&schema, &Dialect::SQLite);
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.message.contains("UUID")));
    }

    #[test]
    fn test_each_validation_category_is_independently_testable() {
        // This test demonstrates that each validation category can be tested independently
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
        table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();

        // Each category can be tested independently
        let enum_result = validator.validate_enums(&schema, None);
        let column_type_result = validator.validate_column_types(&schema);
        let pk_result = validator.validate_primary_keys(&schema);
        let index_result = validator.validate_index_references(&schema);
        let constraint_result = validator.validate_constraint_references(&schema);

        // All should be valid for this well-formed schema
        assert!(enum_result.is_valid());
        assert!(column_type_result.is_valid());
        assert!(pk_result.is_valid());
        assert!(index_result.is_valid());
        assert!(constraint_result.is_valid());
    }

    #[test]
    fn test_validate_empty_schema() {
        let schema = Schema::new("1.0".to_string());
        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_validate_table_without_columns() {
        let mut schema = Schema::new("1.0".to_string());
        let table = Table::new("empty_table".to_string());
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
    }

    #[test]
    fn test_validate_table_without_primary_key() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
    }

    #[test]
    fn test_validate_valid_schema() {
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

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_validate_enum_empty_values() {
        let mut schema = Schema::new("1.0".to_string());

        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec![],
        });

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::Enum {
                name: "status".to_string(),
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("ENUM") && e.to_string().contains("no values")));
    }

    #[test]
    fn test_validate_enum_duplicate_values() {
        let mut schema = Schema::new("1.0".to_string());

        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "active".to_string()],
        });

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::Enum {
                name: "status".to_string(),
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("ENUM") && e.to_string().contains("duplicate")));
    }

    #[test]
    fn test_validate_enum_reference_missing() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::Enum {
                name: "status".to_string(),
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("undefined ENUM")));
    }

    #[test]
    fn test_validate_enum_non_postgres_dialect() {
        let mut schema = Schema::new("1.0".to_string());

        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string()],
        });

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::Enum {
                name: "status".to_string(),
            },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_with_dialect(&schema, Dialect::MySQL);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("PostgreSQL") && e.to_string().contains("ENUM")));
    }

    #[test]
    fn test_validate_dialect_specific_type_skip_validation() {
        // DialectSpecific バリアントは検証をスキップする（データベースに委譲）
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());

        // PostgreSQL SERIAL型（方言固有型）
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::DialectSpecific {
                kind: "SERIAL".to_string(),
                params: serde_json::Value::Null,
            },
            false,
        ));

        // プライマリキーを追加（必須）
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // DialectSpecific型は検証エラーを生成しない
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_validate_dialect_specific_type_with_params() {
        // パラメータ付きDialectSpecific型（MySQL ENUM）も検証スキップ
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());

        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        // MySQL ENUM型（パラメータ付き方言固有型）
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::DialectSpecific {
                kind: "ENUM".to_string(),
                params: serde_json::json!({
                    "values": ["active", "inactive", "pending"]
                }),
            },
            false,
        ));

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // DialectSpecific型は検証エラーを生成しない
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_validate_dialect_specific_type_invalid_kind() {
        // 無効な型名（INVALID_TYPE）でも検証をスキップ
        // データベース実行時にエラーが検出される
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());

        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::DialectSpecific {
                kind: "INVALID_TYPE".to_string(), // 存在しない型
                params: serde_json::Value::Null,
            },
            false,
        ));

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // Strata内部では検証しない（データベースに委譲）
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_validate_mixed_common_and_dialect_specific_types() {
        // 共通型と方言固有型の混在スキーマ
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());

        // 方言固有型（PostgreSQL SERIAL）
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::DialectSpecific {
                kind: "SERIAL".to_string(),
                params: serde_json::Value::Null,
            },
            false,
        ));

        // 共通型（VARCHAR）
        table.add_column(Column::new(
            "username".to_string(),
            ColumnType::VARCHAR { length: 50 },
            false,
        ));

        // 方言固有型（PostgreSQL INET）
        table.add_column(Column::new(
            "ip_address".to_string(),
            ColumnType::DialectSpecific {
                kind: "INET".to_string(),
                params: serde_json::Value::Null,
            },
            true,
        ));

        // 共通型（TIMESTAMP）
        table.add_column(Column::new(
            "created_at".to_string(),
            ColumnType::TIMESTAMP {
                with_time_zone: Some(true),
            },
            false,
        ));

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        // 混在スキーマも有効
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }
}
