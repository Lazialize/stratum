// スキーマバリデーターサービス
//
// スキーマ定義の整合性、参照整合性、制約の検証を行うサービス。
// テーブル定義、インデックス、外部キー制約などを検証します。

use crate::core::config::Dialect;
use crate::core::error::{ErrorLocation, ValidationError, ValidationResult, ValidationWarning};
use crate::core::schema::{ColumnType, Constraint, Schema};

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
        result.merge(self.validate_table_structure(schema));
        result.merge(self.validate_column_types(schema));
        result.merge(self.validate_primary_keys(schema));
        result.merge(self.validate_index_references(schema));
        result.merge(self.validate_constraint_references(schema));

        result
    }

    // ===============================================
    // Task 5.1: カテゴリ別バリデーション関数
    // ===============================================

    /// ENUM定義の検証
    ///
    /// - PostgreSQL以外の方言でENUMが定義されていないか確認
    /// - ENUM値が空でないか確認
    /// - ENUM値に重複がないか確認
    pub fn validate_enums(&self, schema: &Schema, dialect: Option<Dialect>) -> ValidationResult {
        let mut result = ValidationResult::new();

        // ENUMはPostgreSQL専用
        if let Some(dialect) = dialect {
            if !matches!(dialect, Dialect::PostgreSQL) && !schema.enums.is_empty() {
                result.add_error(ValidationError::Constraint {
                    message: format!(
                        "ENUM definitions are only supported in PostgreSQL (current: {})",
                        dialect
                    ),
                    location: None,
                    suggestion: Some("Remove ENUM definitions or switch to PostgreSQL".to_string()),
                });
            }
        }

        // ENUM定義の検証
        for enum_def in schema.enums.values() {
            if enum_def.values.is_empty() {
                result.add_error(ValidationError::Constraint {
                    message: format!("ENUM '{}' has no values defined", enum_def.name),
                    location: None,
                    suggestion: Some("Define at least one ENUM value".to_string()),
                });
                continue;
            }

            let mut seen = std::collections::HashSet::new();
            for value in &enum_def.values {
                if !seen.insert(value) {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "ENUM '{}' has duplicate value '{}'",
                            enum_def.name, value
                        ),
                        location: None,
                        suggestion: Some("Remove duplicate values".to_string()),
                    });
                    break;
                }
            }
        }

        result
    }

    /// テーブル構造の検証（カラムの存在確認）
    fn validate_table_structure(&self, schema: &Schema) -> ValidationResult {
        let mut result = ValidationResult::new();

        for (table_name, table) in &schema.tables {
            if table.columns.is_empty() {
                result.add_error(ValidationError::Constraint {
                    message: format!("Table '{}' has no columns defined", table_name),
                    location: Some(ErrorLocation::with_table(table_name.clone())),
                    suggestion: Some("Define at least one column".to_string()),
                });
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
        let mut result = ValidationResult::new();

        for (table_name, table) in &schema.tables {
            for column in &table.columns {
                self.validate_column_type_internal(
                    &column.column_type,
                    table_name,
                    &column.name,
                    &mut result,
                );

                // ENUM参照の存在確認
                if let ColumnType::Enum { name } = &column.column_type {
                    if !schema.enums.contains_key(name) {
                        result.add_error(ValidationError::Reference {
                            message: format!(
                                "Column '{}.{}' references undefined ENUM '{}'",
                                table_name, column.name, name
                            ),
                            location: Some(ErrorLocation {
                                table: Some(table_name.clone()),
                                column: Some(column.name.clone()),
                                line: None,
                            }),
                            suggestion: Some(format!(
                                "Define ENUM '{}' in the schema enums section",
                                name
                            )),
                        });
                    }
                }
            }
        }

        result
    }

    /// プライマリキーの存在確認
    pub fn validate_primary_keys(&self, schema: &Schema) -> ValidationResult {
        let mut result = ValidationResult::new();

        for (table_name, table) in &schema.tables {
            let has_primary_key = table
                .constraints
                .iter()
                .any(|c| matches!(c, Constraint::PRIMARY_KEY { .. }));

            if !has_primary_key && !table.columns.is_empty() {
                result.add_error(ValidationError::Constraint {
                    message: format!("Table '{}' has no primary key defined", table_name),
                    location: Some(ErrorLocation::with_table(table_name.clone())),
                    suggestion: Some("Add a PRIMARY KEY constraint".to_string()),
                });
            }
        }

        result
    }

    /// インデックスのカラム参照整合性検証
    pub fn validate_index_references(&self, schema: &Schema) -> ValidationResult {
        let mut result = ValidationResult::new();

        for (table_name, table) in &schema.tables {
            for index in &table.indexes {
                for column_name in &index.columns {
                    if table.get_column(column_name).is_none() {
                        result.add_error(ValidationError::Reference {
                            message: format!(
                                "Index '{}' references column '{}' which does not exist in table '{}'",
                                index.name, column_name, table_name
                            ),
                            location: Some(ErrorLocation {
                                table: Some(table_name.clone()),
                                column: Some(column_name.clone()),
                                line: None,
                            }),
                            suggestion: Some(format!(
                                "Define column '{}' or remove it from the index",
                                column_name
                            )),
                        });
                    }
                }
            }
        }

        result
    }

    /// 制約のカラム/テーブル参照整合性検証
    pub fn validate_constraint_references(&self, schema: &Schema) -> ValidationResult {
        let mut result = ValidationResult::new();

        for (table_name, table) in &schema.tables {
            for constraint in &table.constraints {
                match constraint {
                    Constraint::PRIMARY_KEY { columns }
                    | Constraint::UNIQUE { columns }
                    | Constraint::CHECK { columns, .. } => {
                        for column_name in columns {
                            if table.get_column(column_name).is_none() {
                                result.add_error(ValidationError::Reference {
                                    message: format!(
                                        "Constraint references column '{}' which does not exist in table '{}'",
                                        column_name, table_name
                                    ),
                                    location: Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column_name.clone()),
                                        line: None,
                                    }),
                                    suggestion: Some(format!("Define column '{}'", column_name)),
                                });
                            }
                        }
                    }
                    Constraint::FOREIGN_KEY {
                        columns,
                        referenced_table,
                        referenced_columns,
                    } => {
                        // 外部キーのソースカラム存在確認
                        for column_name in columns {
                            if table.get_column(column_name).is_none() {
                                result.add_error(ValidationError::Reference {
                                    message: format!(
                                        "Foreign key constraint references column '{}' which does not exist in table '{}'",
                                        column_name, table_name
                                    ),
                                    location: Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column_name.clone()),
                                        line: None,
                                    }),
                                    suggestion: Some(format!("Define column '{}'", column_name)),
                                });
                            }
                        }

                        // 参照先テーブルの存在確認
                        if !schema.has_table(referenced_table) {
                            result.add_error(ValidationError::Reference {
                                message: format!(
                                    "Foreign key constraint references table '{}' which does not exist",
                                    referenced_table
                                ),
                                location: Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: None,
                                    line: None,
                                }),
                                suggestion: Some(format!("Define table '{}'", referenced_table)),
                            });
                        } else if let Some(ref_table) = schema.get_table(referenced_table) {
                            // 参照先カラムの存在確認
                            for ref_column_name in referenced_columns {
                                if ref_table.get_column(ref_column_name).is_none() {
                                    result.add_error(ValidationError::Reference {
                                        message: format!(
                                            "Foreign key constraint references column '{}' which does not exist in table '{}'",
                                            ref_column_name, referenced_table
                                        ),
                                        location: Some(ErrorLocation {
                                            table: Some(referenced_table.clone()),
                                            column: Some(ref_column_name.clone()),
                                            line: None,
                                        }),
                                        suggestion: Some(format!(
                                            "Define column '{}' in table '{}'",
                                            ref_column_name, referenced_table
                                        )),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        result
    }

    /// カラムの型固有バリデーション（内部用）
    ///
    /// # Arguments
    ///
    /// * `column_type` - 検証対象のカラム型
    /// * `table_name` - テーブル名
    /// * `column_name` - カラム名
    /// * `result` - バリデーション結果（エラーを追加）
    fn validate_column_type_internal(
        &self,
        column_type: &ColumnType,
        table_name: &str,
        column_name: &str,
        result: &mut ValidationResult,
    ) {
        match column_type {
            ColumnType::DECIMAL { precision, scale } => {
                // scale <= precision の検証
                if scale > precision {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "DECIMAL type in column '{}.{}' has scale ({}) greater than precision ({})",
                            table_name, column_name, scale, precision
                        ),
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column_name.to_string()),
                            line: None,
                        }),
                        suggestion: Some(format!(
                            "Set scale <= precision (e.g., DECIMAL({}, {}))",
                            precision,
                            precision.min(scale)
                        )),
                    });
                }

                // precision の範囲チェック（MySQL: 65, PostgreSQL: 1000）
                // 最も厳しい制約（MySQL）を基準とする
                if *precision > 65 {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "DECIMAL type in column '{}.{}' has precision ({}) exceeding maximum (65)",
                            table_name, column_name, precision
                        ),
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column_name.to_string()),
                            line: None,
                        }),
                        suggestion: Some(
                            "Set precision to 65 or less for MySQL compatibility".to_string()
                        ),
                    });
                }

                // precision が 0 でないことを検証
                if *precision == 0 {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "DECIMAL type in column '{}.{}' has invalid precision (0)",
                            table_name, column_name
                        ),
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column_name.to_string()),
                            line: None,
                        }),
                        suggestion: Some("Set precision to at least 1".to_string()),
                    });
                }
            }
            ColumnType::CHAR { length } => {
                // length の範囲チェック（1-255）
                if *length == 0 {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "CHAR type in column '{}.{}' has invalid length (0)",
                            table_name, column_name
                        ),
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column_name.to_string()),
                            line: None,
                        }),
                        suggestion: Some("Set length to at least 1".to_string()),
                    });
                }

                if *length > 255 {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "CHAR type in column '{}.{}' has length ({}) exceeding maximum (255)",
                            table_name, column_name, length
                        ),
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column_name.to_string()),
                            line: None,
                        }),
                        suggestion: Some(
                            "Set length to 255 or less, or use VARCHAR/TEXT for longer strings"
                                .to_string(),
                        ),
                    });
                }
            }
            // 他の型は追加のバリデーション不要
            _ => {}
        }
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
        let mut warnings = Vec::new();

        for (table_name, table) in &schema.tables {
            for column in &table.columns {
                match &column.column_type {
                    ColumnType::DECIMAL { precision, scale } => {
                        // SQLiteでは精度損失の警告
                        if matches!(dialect, Dialect::SQLite) {
                            warnings.push(ValidationWarning::precision_loss(
                                format!(
                                    "DECIMAL({}, {}) in column '{}.{}' will be stored as TEXT in SQLite. Numeric operations may not work as expected.",
                                    precision, scale, table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                    }
                    ColumnType::UUID => {
                        // MySQLではCHAR(36)へのフォールバック警告
                        if matches!(dialect, Dialect::MySQL) {
                            warnings.push(ValidationWarning::dialect_specific(
                                format!(
                                    "UUID in column '{}.{}' will be stored as CHAR(36) in MySQL (native UUID type not available in older versions).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                        // SQLiteではTEXTへのフォールバック警告
                        if matches!(dialect, Dialect::SQLite) {
                            warnings.push(ValidationWarning::dialect_specific(
                                format!(
                                    "UUID in column '{}.{}' will be stored as TEXT in SQLite (native UUID type not available).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                    }
                    ColumnType::JSONB => {
                        // MySQLではJSONへのフォールバック警告
                        if matches!(dialect, Dialect::MySQL) {
                            warnings.push(ValidationWarning::dialect_specific(
                                format!(
                                    "JSONB in column '{}.{}' will be stored as JSON in MySQL (JSONB type not available, binary optimization not applied).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                        // SQLiteではTEXTへのフォールバック警告
                        if matches!(dialect, Dialect::SQLite) {
                            warnings.push(ValidationWarning::dialect_specific(
                                format!(
                                    "JSONB in column '{}.{}' will be stored as TEXT in SQLite (native JSON/JSONB types not available).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                    }
                    ColumnType::TIME { with_time_zone } => {
                        // MySQLとSQLiteではタイムゾーン情報が失われる警告
                        if *with_time_zone == Some(true) {
                            if matches!(dialect, Dialect::MySQL) {
                                warnings.push(ValidationWarning::dialect_specific(
                                    format!(
                                        "TIME WITH TIME ZONE in column '{}.{}' will be stored as TIME in MySQL (timezone information will be lost).",
                                        table_name, column.name
                                    ),
                                    Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column.name.clone()),
                                        line: None,
                                    }),
                                ));
                            }
                            if matches!(dialect, Dialect::SQLite) {
                                warnings.push(ValidationWarning::precision_loss(
                                    format!(
                                        "TIME WITH TIME ZONE in column '{}.{}' will be stored as TEXT in SQLite (timezone information will be lost).",
                                        table_name, column.name
                                    ),
                                    Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column.name.clone()),
                                        line: None,
                                    }),
                                ));
                            }
                        }
                    }
                    ColumnType::DATE => {
                        // SQLiteではTEXT保存の警告
                        if matches!(dialect, Dialect::SQLite) {
                            warnings.push(ValidationWarning::dialect_specific(
                                format!(
                                    "DATE in column '{}.{}' will be stored as TEXT in SQLite (native DATE type not available).",
                                    table_name, column.name
                                ),
                                Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }

        warnings
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
        self.validate_renames_internal(new_schema, None)
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
        self.validate_renames_internal(new_schema, Some(old_schema))
    }

    /// カラムリネーム検証の内部実装
    fn validate_renames_internal(
        &self,
        schema: &Schema,
        old_schema: Option<&Schema>,
    ) -> ValidationResult {
        let mut result = ValidationResult::new();

        for (table_name, table) in &schema.tables {
            // 重複リネーム検出用のマップ（renamed_from -> カラム名のリスト）
            let mut rename_sources: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();

            // renamed_fromを持つカラムを収集
            for column in &table.columns {
                if let Some(ref old_name) = column.renamed_from {
                    rename_sources
                        .entry(old_name.clone())
                        .or_default()
                        .push(column.name.clone());
                }
            }

            // 重複リネームの検出
            for (old_name, new_names) in &rename_sources {
                if new_names.len() > 1 {
                    result.add_error(ValidationError::Constraint {
                        message: format!(
                            "duplicate rename: '{}' is renamed to multiple columns ({}) in table '{}'",
                            old_name,
                            new_names.join(", "),
                            table_name
                        ),
                        location: Some(ErrorLocation::with_table(table_name.clone())),
                        suggestion: Some("Each column can only be renamed once. Remove duplicate renamed_from attributes.".to_string()),
                    });
                }
            }

            // 名前衝突の検出
            // renamed_fromが既存のカラム名（リネーム先でない）と衝突する場合
            for column in &table.columns {
                if let Some(ref old_name) = column.renamed_from {
                    // 同じテーブル内に old_name と同名のカラムが存在するか確認
                    // （ただし、そのカラム自体が新しい名前に変わるのでなければエラー）
                    for other_column in &table.columns {
                        // 既存のカラム名がold_nameと一致し、
                        // そのカラムがリネーム対象でない（renamed_fromを持たない）場合
                        if other_column.name == *old_name && other_column.renamed_from.is_none() {
                            result.add_error(ValidationError::Constraint {
                                message: format!(
                                    "name collision: renamed_from '{}' conflicts with existing column '{}' in table '{}'",
                                    old_name, other_column.name, table_name
                                ),
                                location: Some(ErrorLocation {
                                    table: Some(table_name.clone()),
                                    column: Some(column.name.clone()),
                                    line: None,
                                }),
                                suggestion: Some(format!(
                                    "Remove the existing column '{}' or change the renamed_from value",
                                    old_name
                                )),
                            });
                        }
                    }

                    // 旧カラム存在確認（old_schemaが提供された場合のみ）
                    if let Some(old_schema) = old_schema {
                        if let Some(old_table) = old_schema.get_table(table_name) {
                            if old_table.get_column(old_name).is_none() {
                                result.add_warning(ValidationWarning::old_column_not_found(
                                    format!(
                                        "Column '{}' in table '{}' has renamed_from='{}', but column '{}' does not exist in the old schema. \
                                        Consider removing the renamed_from attribute.",
                                        column.name, table_name, old_name, old_name
                                    ),
                                    Some(ErrorLocation {
                                        table: Some(table_name.clone()),
                                        column: Some(column.name.clone()),
                                        line: None,
                                    }),
                                ));
                            }
                        }
                    }
                }
            }
        }

        // FK参照カラムのリネーム警告
        // 他のテーブルからFKで参照されているカラムがリネームされる場合
        for (table_name, table) in &schema.tables {
            for column in &table.columns {
                if let Some(ref old_name) = column.renamed_from {
                    // このテーブルのこのカラム（old_name）を参照しているFKがあるか確認
                    for (other_table_name, other_table) in &schema.tables {
                        for constraint in &other_table.constraints {
                            if let Constraint::FOREIGN_KEY {
                                referenced_table,
                                referenced_columns,
                                ..
                            } = constraint
                            {
                                if referenced_table == table_name
                                    && referenced_columns.contains(old_name)
                                {
                                    result.add_warning(ValidationWarning::foreign_key_reference(
                                        format!(
                                            "Column '{}' in table '{}' is referenced by a foreign key from table '{}'. \
                                            Renaming this column may break the FK constraint. \
                                            Update the FK reference after migration.",
                                            old_name, table_name, other_table_name
                                        ),
                                        Some(ErrorLocation {
                                            table: Some(table_name.clone()),
                                            column: Some(old_name.clone()),
                                            line: None,
                                        }),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        result
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
        let mut errors = Vec::new();

        for (table_name, table) in &schema.tables {
            for constraint in &table.constraints {
                if let Constraint::FOREIGN_KEY {
                    referenced_table,
                    referenced_columns,
                    ..
                } = constraint
                {
                    // Check if referenced table exists
                    if !schema.has_table(referenced_table) {
                        errors.push(ValidationError::Reference {
                            message: format!(
                                "Foreign key constraint references table '{}' which does not exist",
                                referenced_table
                            ),
                            location: Some(ErrorLocation::with_table(table_name.clone())),
                            suggestion: Some(format!("Define table '{}'", referenced_table)),
                        });
                    } else if let Some(ref_table) = schema.get_table(referenced_table) {
                        // Check if referenced columns exist
                        for ref_column_name in referenced_columns {
                            if ref_table.get_column(ref_column_name).is_none() {
                                errors.push(ValidationError::Reference {
                                    message: format!(
                                        "Foreign key constraint references column '{}' which does not exist in table '{}'",
                                        ref_column_name, referenced_table
                                    ),
                                    location: Some(ErrorLocation {
                                        table: Some(referenced_table.clone()),
                                        column: Some(ref_column_name.clone()),
                                        line: None,
                                    }),
                                    suggestion: Some(format!(
                                        "Define column '{}' in table '{}'",
                                        ref_column_name, referenced_table
                                    )),
                                });
                            }
                        }
                    }
                }
            }
        }

        errors
    }
}

impl Default for SchemaValidatorService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Dialect;
    use crate::core::schema::{Column, ColumnType, EnumDefinition, Index, Table};

    #[test]
    fn test_new_service() {
        let service = SchemaValidatorService::new();
        // サービスが正常に作成されることを確認
        assert!(format!("{:?}", service).contains("SchemaValidatorService"));
    }

    // ===============================================
    // Task 5.1: カテゴリ別バリデーション関数のテスト
    // ===============================================

    #[test]
    fn test_validate_enums_empty_values() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec![],
        });

        let validator = SchemaValidatorService::new();
        let result = validator.validate_enums(&schema, None);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("no values")));
    }

    #[test]
    fn test_validate_enums_duplicate_values() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "active".to_string()],
        });

        let validator = SchemaValidatorService::new();
        let result = validator.validate_enums(&schema, None);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("duplicate")));
    }

    #[test]
    fn test_validate_enums_non_postgres_dialect() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string()],
        });

        let validator = SchemaValidatorService::new();
        let result = validator.validate_enums(&schema, Some(Dialect::MySQL));

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("PostgreSQL")));
    }

    #[test]
    fn test_validate_enums_valid() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        let validator = SchemaValidatorService::new();
        let result = validator.validate_enums(&schema, Some(Dialect::PostgreSQL));

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_column_types_decimal_invalid_scale() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 10,
                scale: 15,
            },
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_column_types(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("scale")));
    }

    #[test]
    fn test_validate_column_types_enum_reference_missing() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "status".to_string(),
            ColumnType::Enum {
                name: "nonexistent_enum".to_string(),
            },
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_column_types(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("undefined ENUM")));
    }

    #[test]
    fn test_validate_primary_keys_missing() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_primary_keys(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("primary key")));
    }

    #[test]
    fn test_validate_primary_keys_present() {
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
        let result = validator.validate_primary_keys(&schema);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_index_references_invalid_column() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["nonexistent_column".to_string()],
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_index_references(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("Index")));
    }

    #[test]
    fn test_validate_index_references_valid() {
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
        table.add_index(Index::new(
            "idx_email".to_string(),
            vec!["email".to_string()],
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_index_references(&schema);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_constraint_references_invalid_fk_table() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("posts".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_column(Column::new(
            "user_id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "nonexistent_table".to_string(),
            referenced_columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_constraint_references(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("does not exist")));
    }

    #[test]
    fn test_validate_constraint_references_invalid_pk_column() {
        let mut schema = Schema::new("1.0".to_string());
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["nonexistent_column".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_constraint_references(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("Constraint references column")));
    }

    #[test]
    fn test_validate_constraint_references_valid() {
        let mut schema = Schema::new("1.0".to_string());

        // users table
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

        // posts table with valid FK
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
        posts_table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
        });
        schema.add_table(posts_table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_constraint_references(&schema);

        assert!(result.is_valid());
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
    fn test_validate_decimal_type_invalid_scale() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 10,
                scale: 15, // scale > precision はエラー
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
        assert!(result.error_count() > 0);
        assert!(result.errors[0]
            .to_string()
            .contains("scale (15) greater than precision (10)"));
    }

    #[test]
    fn test_validate_decimal_type_zero_precision() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 0, // precision = 0 はエラー
                scale: 0,
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
        assert!(result.error_count() > 0);
        assert!(result.errors[0]
            .to_string()
            .contains("invalid precision (0)"));
    }

    #[test]
    fn test_validate_decimal_type_excessive_precision() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 100, // precision > 65 は警告
                scale: 2,
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
        assert!(result.error_count() > 0);
        assert!(result.errors[0]
            .to_string()
            .contains("precision (100) exceeding maximum (65)"));
    }

    #[test]
    fn test_validate_char_type_zero_length() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "code".to_string(),
            ColumnType::CHAR { length: 0 }, // length = 0 はエラー
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
        assert!(result.errors[0].to_string().contains("invalid length (0)"));
    }

    #[test]
    fn test_validate_char_type_excessive_length() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "code".to_string(),
            ColumnType::CHAR { length: 300 }, // length > 255 はエラー
            false,
        ));
        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate(&schema);

        assert!(!result.is_valid());
        assert!(result.error_count() > 0);
        assert!(result.errors[0]
            .to_string()
            .contains("length (300) exceeding maximum (255)"));
    }

    #[test]
    fn test_generate_dialect_warnings_sqlite_decimal() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("products".to_string());
        table.add_column(Column::new(
            "price".to_string(),
            ColumnType::DECIMAL {
                precision: 10,
                scale: 2,
            },
            false,
        ));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let warnings = validator.generate_dialect_warnings(&schema, &Dialect::SQLite);

        assert!(!warnings.is_empty());
        assert!(warnings[0].message.contains("will be stored as TEXT"));
    }

    #[test]
    fn test_generate_dialect_warnings_mysql_uuid() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new("uuid".to_string(), ColumnType::UUID, false));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let warnings = validator.generate_dialect_warnings(&schema, &Dialect::MySQL);

        assert!(!warnings.is_empty());
        assert!(warnings[0].message.contains("CHAR(36)"));
    }

    #[test]
    fn test_generate_dialect_warnings_mysql_jsonb() {
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("documents".to_string());
        table.add_column(Column::new("data".to_string(), ColumnType::JSONB, false));
        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let warnings = validator.generate_dialect_warnings(&schema, &Dialect::MySQL);

        assert!(!warnings.is_empty());
        assert!(warnings[0]
            .message
            .contains("will be stored as JSON in MySQL"));
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

    // ===============================================
    // Task 3.2: カラムリネーム検証のテスト
    // ===============================================

    #[test]
    fn test_validate_renames_duplicate_renamed_from_error() {
        // 同じrenamed_fromが複数カラムで指定された場合のエラー検出
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        // 両方のカラムが同じold_nameからリネームしようとしている
        let mut column1 = Column::new(
            "new_email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        column1.renamed_from = Some("old_email".to_string());
        table.add_column(column1);

        let mut column2 = Column::new(
            "email_address".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        column2.renamed_from = Some("old_email".to_string()); // 重複
        table.add_column(column2);

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_renames(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("old_email") && e.to_string().contains("duplicate")));
    }

    #[test]
    fn test_validate_renames_name_collision_error() {
        // renamed_fromがリネーム先以外の既存カラム名と一致する場合のエラー
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        // 既存のカラム
        table.add_column(Column::new(
            "existing_column".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        ));

        // renamed_fromが既存のカラム名と衝突
        let mut renamed_column = Column::new(
            "new_name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        renamed_column.renamed_from = Some("existing_column".to_string());
        table.add_column(renamed_column);

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_renames(&schema);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("existing_column")
                && e.to_string().contains("collision")));
    }

    #[test]
    fn test_validate_renames_fk_reference_warning() {
        // FK参照カラムのリネーム警告
        let mut schema = Schema::new("1.0".to_string());

        // users table (参照元)
        let mut users_table = Table::new("users".to_string());
        users_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        // リネームされるカラム（FK参照されている）
        let mut renamed_column = Column::new("user_uuid".to_string(), ColumnType::UUID, false);
        renamed_column.renamed_from = Some("uuid".to_string());
        users_table.add_column(renamed_column);
        users_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(users_table);

        // posts table (FK参照を持つ)
        let mut posts_table = Table::new("posts".to_string());
        posts_table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        posts_table.add_column(Column::new(
            "user_uuid".to_string(),
            ColumnType::UUID,
            false,
        ));
        posts_table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        // uuid カラムを参照するFK（リネームされるカラム）
        posts_table.add_constraint(Constraint::FOREIGN_KEY {
            columns: vec!["user_uuid".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["uuid".to_string()], // リネームされるカラムを参照
        });
        schema.add_table(posts_table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_renames(&schema);

        // 警告が生成されるべき
        assert!(result.warning_count() > 0);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("foreign key") || w.message.contains("FK")));
    }

    #[test]
    fn test_validate_renames_valid_single_rename() {
        // 有効な単一リネーム
        let mut schema = Schema::new("1.0".to_string());

        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));

        let mut renamed_column = Column::new(
            "email_address".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        renamed_column.renamed_from = Some("email".to_string());
        table.add_column(renamed_column);

        table.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        schema.add_table(table);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_renames(&schema);

        // エラーなし
        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_renames_no_renames() {
        // リネームなしのスキーマ
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

        let validator = SchemaValidatorService::new();
        let result = validator.validate_renames(&schema);

        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
        assert_eq!(result.warning_count(), 0);
    }

    #[test]
    fn test_validate_renames_multiple_tables_with_same_old_name() {
        // 異なるテーブルで同じrenamed_fromを使用（これはOK）
        let mut schema = Schema::new("1.0".to_string());

        let mut table1 = Table::new("users".to_string());
        table1.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        let mut column1 = Column::new(
            "new_name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        column1.renamed_from = Some("old_name".to_string());
        table1.add_column(column1);
        table1.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table1);

        let mut table2 = Table::new("posts".to_string());
        table2.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        // 別のテーブルで同じold_name（これは許可される）
        let mut column2 = Column::new(
            "new_name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        );
        column2.renamed_from = Some("old_name".to_string());
        table2.add_column(column2);
        table2.add_constraint(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });
        schema.add_table(table2);

        let validator = SchemaValidatorService::new();
        let result = validator.validate_renames(&schema);

        // 異なるテーブルでの同名は許可
        assert!(result.is_valid());
    }
}
