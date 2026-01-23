// 型変更検証サービス
//
// カラム型変更の互換性を検証し、警告/エラーを生成します。

use crate::core::config::Dialect;
use crate::core::error::{ErrorLocation, ValidationError, ValidationResult, ValidationWarning};
use crate::core::schema::ColumnType;
use crate::core::schema_diff::{ColumnChange, ColumnDiff};
use crate::core::type_category::{TypeCategory, TypeConversionResult};

/// 型変更検証サービス
///
/// カラム型変更の互換性を検証し、警告やエラーを生成します。
pub struct TypeChangeValidator;

impl TypeChangeValidator {
    /// 新しいTypeChangeValidatorを作成
    pub fn new() -> Self {
        Self
    }

    /// 型変更の検証を実行
    ///
    /// # Arguments
    /// * `table_name` - テーブル名
    /// * `column_diffs` - 検証対象のカラム差分リスト
    /// * `dialect` - 対象データベース方言
    ///
    /// # Returns
    /// 警告とエラーを含むValidationResult
    pub fn validate_type_changes(
        &self,
        table_name: &str,
        column_diffs: &[ColumnDiff],
        dialect: &Dialect,
    ) -> ValidationResult {
        let mut result = ValidationResult::new();

        for column_diff in column_diffs {
            // TypeChangedを含むカラムのみ検証
            if !self.has_type_change(column_diff) {
                continue;
            }

            let old_type = &column_diff.old_column.column_type;
            let new_type = &column_diff.new_column.column_type;
            let column_name = &column_diff.column_name;

            // 方言固有の制約検証
            if let Some(error) = self.validate_dialect_constraints(
                old_type,
                new_type,
                table_name,
                column_name,
                dialect,
            ) {
                result.add_error(error);
                continue; // 方言制約エラーがあれば他の検証をスキップ
            }

            // 型互換性の検証
            match self.validate_type_compatibility(old_type, new_type, table_name, column_name) {
                Ok(Some(warning)) => result.add_warning(warning),
                Ok(None) => {}
                Err(error) => result.add_error(error),
            }

            // 精度損失の検証（同一カテゴリ内でのサイズ縮小）
            if let Some(warning) =
                self.validate_precision_loss(old_type, new_type, table_name, column_name)
            {
                result.add_warning(warning);
            }
        }

        result
    }

    /// カラム差分がTypeChangedを含むかどうか
    fn has_type_change(&self, column_diff: &ColumnDiff) -> bool {
        column_diff
            .changes
            .iter()
            .any(|change| matches!(change, ColumnChange::TypeChanged { .. }))
    }

    /// 型互換性の検証
    ///
    /// カテゴリ間の変換ルールに基づいて警告またはエラーを返します。
    fn validate_type_compatibility(
        &self,
        old_type: &ColumnType,
        new_type: &ColumnType,
        table_name: &str,
        column_name: &str,
    ) -> Result<Option<ValidationWarning>, ValidationError> {
        let old_category = TypeCategory::from_column_type(old_type);
        let new_category = TypeCategory::from_column_type(new_type);

        let location = Some(ErrorLocation {
            table: Some(table_name.to_string()),
            column: Some(column_name.to_string()),
            line: None,
        });

        match old_category.conversion_result(&new_category) {
            TypeConversionResult::Safe | TypeConversionResult::SafeWithPrecisionCheck => Ok(None),
            TypeConversionResult::Warning => {
                let message = format!(
                    "{:?} → {:?} may cause data loss for incompatible values",
                    old_type, new_type
                );
                Ok(Some(ValidationWarning::data_loss(message, location)))
            }
            TypeConversionResult::Error => {
                let message = format!(
                    "{:?} → {:?} is not supported (incompatible types)",
                    old_type, new_type
                );
                let suggestion = Some(self.suggest_intermediate_type(&old_category, &new_category));
                Err(ValidationError::TypeConversion {
                    message,
                    location,
                    suggestion,
                })
            }
        }
    }

    /// 精度損失の検証
    ///
    /// 同一カテゴリ内でのサイズ縮小を検出します。
    fn validate_precision_loss(
        &self,
        old_type: &ColumnType,
        new_type: &ColumnType,
        table_name: &str,
        column_name: &str,
    ) -> Option<ValidationWarning> {
        let location = Some(ErrorLocation {
            table: Some(table_name.to_string()),
            column: Some(column_name.to_string()),
            line: None,
        });

        match (old_type, new_type) {
            // VARCHAR サイズ縮小
            (ColumnType::VARCHAR { length: old_len }, ColumnType::VARCHAR { length: new_len })
                if new_len < old_len =>
            {
                let message = format!(
                    "VARCHAR({}) → VARCHAR({}) may cause data truncation",
                    old_len, new_len
                );
                Some(ValidationWarning::precision_loss(message, location))
            }

            // CHAR サイズ縮小
            (ColumnType::CHAR { length: old_len }, ColumnType::CHAR { length: new_len })
                if new_len < old_len =>
            {
                let message = format!(
                    "CHAR({}) → CHAR({}) may cause data truncation",
                    old_len, new_len
                );
                Some(ValidationWarning::precision_loss(message, location))
            }

            // DECIMAL 精度縮小
            (
                ColumnType::DECIMAL {
                    precision: old_prec,
                    scale: old_scale,
                },
                ColumnType::DECIMAL {
                    precision: new_prec,
                    scale: new_scale,
                },
            ) if new_prec < old_prec || new_scale < old_scale => {
                let message = format!(
                    "DECIMAL({}, {}) → DECIMAL({}, {}) may cause precision loss",
                    old_prec, old_scale, new_prec, new_scale
                );
                Some(ValidationWarning::precision_loss(message, location))
            }

            // INTEGER 精度縮小 (precision指定がある場合)
            (
                ColumnType::INTEGER {
                    precision: Some(old_prec),
                },
                ColumnType::INTEGER {
                    precision: Some(new_prec),
                },
            ) if new_prec < old_prec => {
                let message = format!(
                    "INTEGER({}) → INTEGER({}) may cause overflow",
                    old_prec, new_prec
                );
                Some(ValidationWarning::precision_loss(message, location))
            }

            // BIGINT → INTEGER (8 → 4バイト)
            (
                ColumnType::INTEGER { precision: Some(8) },
                ColumnType::INTEGER { precision: Some(4) },
            )
            | (
                ColumnType::INTEGER { precision: Some(8) },
                ColumnType::INTEGER { precision: None },
            ) => {
                let message = "BIGINT → INTEGER may cause overflow for large values".to_string();
                Some(ValidationWarning::precision_loss(message, location))
            }

            // INTEGER → SMALLINT (4 → 2バイト)
            (
                ColumnType::INTEGER { precision: Some(4) },
                ColumnType::INTEGER { precision: Some(2) },
            )
            | (
                ColumnType::INTEGER { precision: None },
                ColumnType::INTEGER { precision: Some(2) },
            ) => {
                let message = "INTEGER → SMALLINT may cause overflow for large values".to_string();
                Some(ValidationWarning::precision_loss(message, location))
            }

            _ => None,
        }
    }

    /// 中間型の提案を生成
    fn suggest_intermediate_type(
        &self,
        _from_category: &TypeCategory,
        _to_category: &TypeCategory,
    ) -> String {
        "Use TEXT as an intermediate type or reconsider the type change".to_string()
    }

    /// 方言固有の制約を検証
    ///
    /// 各データベース方言でサポートされない型変更を検出します。
    fn validate_dialect_constraints(
        &self,
        old_type: &ColumnType,
        new_type: &ColumnType,
        table_name: &str,
        column_name: &str,
        dialect: &Dialect,
    ) -> Option<ValidationError> {
        let location = Some(ErrorLocation {
            table: Some(table_name.to_string()),
            column: Some(column_name.to_string()),
            line: None,
        });

        match dialect {
            Dialect::MySQL => self.validate_mysql_constraints(old_type, new_type, location),
            Dialect::SQLite => self.validate_sqlite_constraints(old_type, new_type, location),
            Dialect::PostgreSQL => self.validate_postgres_constraints(old_type, new_type, location),
        }
    }

    /// MySQL固有の制約を検証
    fn validate_mysql_constraints(
        &self,
        old_type: &ColumnType,
        new_type: &ColumnType,
        location: Option<ErrorLocation>,
    ) -> Option<ValidationError> {
        // MySQLはJSONBをサポートしない（JSONのみ）
        if matches!(new_type, ColumnType::JSONB) {
            return Some(ValidationError::DialectConstraint {
                message: format!(
                    "{:?} → {:?} is not supported in MySQL (use JSON instead of JSONB)",
                    old_type, new_type
                ),
                location,
                dialect: "MySQL".to_string(),
            });
        }

        // MySQLはUUIDをネイティブサポートしない（CHAR(36)またはBINARY(16)を使用）
        if matches!(new_type, ColumnType::UUID) && !matches!(old_type, ColumnType::UUID) {
            return Some(ValidationError::DialectConstraint {
                message: format!(
                    "{:?} → {:?} is not natively supported in MySQL (use CHAR(36) or BINARY(16))",
                    old_type, new_type
                ),
                location,
                dialect: "MySQL".to_string(),
            });
        }

        None
    }

    /// SQLite固有の制約を検証
    fn validate_sqlite_constraints(
        &self,
        old_type: &ColumnType,
        new_type: &ColumnType,
        location: Option<ErrorLocation>,
    ) -> Option<ValidationError> {
        // SQLiteはBOOLEAN型をネイティブサポートしない（INTEGER 0/1で代替）
        // ただし、他の型からBOOLEANへの変換は許容（内部的にはINTEGERとして扱われる）

        // SQLiteはDECIMAL/NUMERICの精度を保証しない
        if matches!(
            new_type,
            ColumnType::DECIMAL {
                precision: _,
                scale: _
            }
        ) && !matches!(
            old_type,
            ColumnType::DECIMAL {
                precision: _,
                scale: _
            }
        ) {
            // 警告レベルなのでここではエラーを返さない
            // ただし、JSONBへの変換は厳密にはサポートされない
        }

        // SQLiteはJSONBをサポートしない（JSONはSQLite 3.38+で部分サポート）
        if matches!(new_type, ColumnType::JSONB) {
            return Some(ValidationError::DialectConstraint {
                message: format!(
                    "{:?} → {:?} is not supported in SQLite (JSONB is PostgreSQL-specific)",
                    old_type, new_type
                ),
                location,
                dialect: "SQLite".to_string(),
            });
        }

        // SQLiteはUUIDをネイティブサポートしない
        if matches!(new_type, ColumnType::UUID) && !matches!(old_type, ColumnType::UUID) {
            return Some(ValidationError::DialectConstraint {
                message: format!(
                    "{:?} → {:?} is not natively supported in SQLite (use TEXT for UUID storage)",
                    old_type, new_type
                ),
                location,
                dialect: "SQLite".to_string(),
            });
        }

        None
    }

    /// PostgreSQL固有の制約を検証
    fn validate_postgres_constraints(
        &self,
        _old_type: &ColumnType,
        _new_type: &ColumnType,
        _location: Option<ErrorLocation>,
    ) -> Option<ValidationError> {
        // PostgreSQLは最も柔軟な型サポートを持つため、
        // 特別な方言固有の制約はほとんどない
        None
    }
}

impl Default for TypeChangeValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::Column;

    fn create_column_diff(
        column_name: &str,
        old_type: ColumnType,
        new_type: ColumnType,
    ) -> ColumnDiff {
        let old_column = Column::new(column_name.to_string(), old_type, false);
        let new_column = Column::new(column_name.to_string(), new_type, false);
        ColumnDiff::new(column_name.to_string(), old_column, new_column)
    }

    // ==========================================
    // 型互換性検証のテスト
    // ==========================================

    #[test]
    fn test_safe_conversion_same_category() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff(
            "age",
            ColumnType::INTEGER { precision: Some(4) },
            ColumnType::INTEGER { precision: Some(8) },
        );

        let result = validator.validate_type_changes("users", &[diff], &Dialect::PostgreSQL);

        // 同一カテゴリで拡大は警告なし
        assert!(result.is_valid());
    }

    #[test]
    fn test_warning_conversion_string_to_numeric() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff(
            "price",
            ColumnType::VARCHAR { length: 255 },
            ColumnType::INTEGER { precision: None },
        );

        let result = validator.validate_type_changes("products", &[diff], &Dialect::PostgreSQL);

        // String → Numeric は警告
        assert!(result.is_valid()); // エラーではない
        assert_eq!(result.warning_count(), 1);
        assert!(result.warnings[0].message.contains("data loss"));
    }

    #[test]
    fn test_error_conversion_numeric_to_datetime() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff(
            "created_at",
            ColumnType::INTEGER { precision: None },
            ColumnType::TIMESTAMP {
                with_time_zone: None,
            },
        );

        let result = validator.validate_type_changes("users", &[diff], &Dialect::PostgreSQL);

        // Numeric → DateTime はエラー
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
        assert!(result.errors[0].is_type_conversion());
    }

    #[test]
    fn test_error_conversion_json_to_numeric() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff(
            "data",
            ColumnType::JSONB,
            ColumnType::INTEGER { precision: None },
        );

        let result = validator.validate_type_changes("documents", &[diff], &Dialect::PostgreSQL);

        // Json → Numeric はエラー
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
    }

    #[test]
    fn test_safe_conversion_boolean_to_numeric() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff(
            "is_active",
            ColumnType::BOOLEAN,
            ColumnType::INTEGER { precision: None },
        );

        let result = validator.validate_type_changes("users", &[diff], &Dialect::PostgreSQL);

        // Boolean → Numeric は安全
        assert!(result.is_valid());
        assert_eq!(result.warning_count(), 0);
    }

    #[test]
    fn test_safe_conversion_any_to_string() {
        let validator = TypeChangeValidator::new();
        let diffs = vec![
            create_column_diff(
                "col1",
                ColumnType::INTEGER { precision: None },
                ColumnType::TEXT,
            ),
            create_column_diff("col2", ColumnType::BOOLEAN, ColumnType::TEXT),
            create_column_diff("col3", ColumnType::UUID, ColumnType::TEXT),
        ];

        let result = validator.validate_type_changes("test_table", &diffs, &Dialect::PostgreSQL);

        // すべてStringへの変換は安全
        assert!(result.is_valid());
        assert_eq!(result.warning_count(), 0);
    }

    // ==========================================
    // 精度損失検証のテスト
    // ==========================================

    #[test]
    fn test_precision_loss_varchar_shrink() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff(
            "email",
            ColumnType::VARCHAR { length: 255 },
            ColumnType::VARCHAR { length: 100 },
        );

        let result = validator.validate_type_changes("users", &[diff], &Dialect::PostgreSQL);

        // VARCHARサイズ縮小は警告
        assert!(result.is_valid());
        assert_eq!(result.warning_count(), 1);
        assert!(result.warnings[0].message.contains("truncation"));
    }

    #[test]
    fn test_precision_loss_decimal_shrink() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff(
            "price",
            ColumnType::DECIMAL {
                precision: 10,
                scale: 2,
            },
            ColumnType::DECIMAL {
                precision: 5,
                scale: 2,
            },
        );

        let result = validator.validate_type_changes("products", &[diff], &Dialect::PostgreSQL);

        // DECIMAL精度縮小は警告
        assert!(result.is_valid());
        assert_eq!(result.warning_count(), 1);
        assert!(result.warnings[0].message.contains("precision loss"));
    }

    #[test]
    fn test_precision_loss_bigint_to_integer() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff(
            "big_id",
            ColumnType::INTEGER { precision: Some(8) },
            ColumnType::INTEGER { precision: Some(4) },
        );

        let result = validator.validate_type_changes("users", &[diff], &Dialect::PostgreSQL);

        // BIGINT → INTEGER は警告
        assert!(result.is_valid());
        assert_eq!(result.warning_count(), 1);
        assert!(result.warnings[0].message.contains("overflow"));
    }

    #[test]
    fn test_no_precision_loss_varchar_expand() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff(
            "email",
            ColumnType::VARCHAR { length: 100 },
            ColumnType::VARCHAR { length: 255 },
        );

        let result = validator.validate_type_changes("users", &[diff], &Dialect::PostgreSQL);

        // VARCHARサイズ拡大は警告なし
        assert!(result.is_valid());
        assert_eq!(result.warning_count(), 0);
    }

    // ==========================================
    // 複合テスト
    // ==========================================

    #[test]
    fn test_multiple_changes_mixed_results() {
        let validator = TypeChangeValidator::new();
        let diffs = vec![
            // 安全な変換
            create_column_diff(
                "col1",
                ColumnType::INTEGER { precision: None },
                ColumnType::TEXT,
            ),
            // 警告（データ損失リスク）
            create_column_diff(
                "col2",
                ColumnType::TEXT,
                ColumnType::INTEGER { precision: None },
            ),
            // エラー（互換性なし）
            create_column_diff(
                "col3",
                ColumnType::JSONB,
                ColumnType::INTEGER { precision: None },
            ),
        ];

        let result = validator.validate_type_changes("test_table", &diffs, &Dialect::PostgreSQL);

        // 1つのエラーと1つの警告
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
        assert_eq!(result.warning_count(), 1);
    }

    #[test]
    fn test_skip_non_type_change() {
        let validator = TypeChangeValidator::new();

        // 型変更なし（NULLableのみ変更）
        let mut old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        old_column.nullable = false;
        let mut new_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            true,
        );
        new_column.nullable = true;
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let result = validator.validate_type_changes("users", &[diff], &Dialect::PostgreSQL);

        // 型変更がないので検証はスキップ
        assert!(result.is_valid());
        assert_eq!(result.warning_count(), 0);
    }

    // ==========================================
    // 方言固有制約検証のテスト
    // ==========================================

    #[test]
    fn test_mysql_jsonb_not_supported() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff("data", ColumnType::TEXT, ColumnType::JSONB);

        let result = validator.validate_type_changes("documents", &[diff], &Dialect::MySQL);

        // MySQLではJSONBはサポートされない
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
        assert!(result.errors[0].is_dialect_constraint());
    }

    #[test]
    fn test_mysql_uuid_not_supported() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff("user_id", ColumnType::TEXT, ColumnType::UUID);

        let result = validator.validate_type_changes("users", &[diff], &Dialect::MySQL);

        // MySQLではUUIDはネイティブサポートされない
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
        assert!(result.errors[0].is_dialect_constraint());
    }

    #[test]
    fn test_sqlite_jsonb_not_supported() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff("data", ColumnType::TEXT, ColumnType::JSONB);

        let result = validator.validate_type_changes("documents", &[diff], &Dialect::SQLite);

        // SQLiteではJSONBはサポートされない
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
        assert!(result.errors[0].is_dialect_constraint());
    }

    #[test]
    fn test_sqlite_uuid_not_supported() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff("user_id", ColumnType::TEXT, ColumnType::UUID);

        let result = validator.validate_type_changes("users", &[diff], &Dialect::SQLite);

        // SQLiteではUUIDはネイティブサポートされない
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
        assert!(result.errors[0].is_dialect_constraint());
    }

    #[test]
    fn test_postgres_jsonb_supported() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff("data", ColumnType::TEXT, ColumnType::JSONB);

        let result = validator.validate_type_changes("documents", &[diff], &Dialect::PostgreSQL);

        // PostgreSQLではJSONBはサポートされる（型変換としては警告）
        // 方言制約エラーは発生しない
        assert!(result.errors.iter().all(|e| !e.is_dialect_constraint()));
    }

    #[test]
    fn test_postgres_uuid_supported() {
        let validator = TypeChangeValidator::new();
        let diff = create_column_diff("user_id", ColumnType::TEXT, ColumnType::UUID);

        let result = validator.validate_type_changes("users", &[diff], &Dialect::PostgreSQL);

        // PostgreSQLではUUIDはサポートされる（型変換としては警告）
        // 方言制約エラーは発生しない
        assert!(result.errors.iter().all(|e| !e.is_dialect_constraint()));
    }
}
