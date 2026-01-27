// 型マッピングサービス
//
// 方言に依存しない共通インターフェースで ColumnType ↔ SQL型文字列 の
// 双方向変換を一元管理します。

use crate::adapters::sql_quote::{quote_identifier_mysql, quote_identifier_postgres};
use crate::core::config::Dialect;
use crate::core::schema::ColumnType;
use anyhow::Result;
use std::collections::HashSet;

/// 型メタデータ
///
/// データベースから取得した型の追加情報を保持します。
#[derive(Debug, Clone, Default)]
pub struct TypeMetadata {
    /// 文字列型の最大長
    pub char_max_length: Option<u32>,
    /// 数値型の精度
    pub numeric_precision: Option<u32>,
    /// 数値型の小数点以下桁数
    pub numeric_scale: Option<u32>,
    /// ユーザー定義型名（PostgreSQLのENUM等）
    pub udt_name: Option<String>,
    /// 既知のENUM型名のセット（PostgreSQL用）
    pub enum_names: Option<HashSet<String>>,
}

/// 方言固有の型マッピング拡張
///
/// 各データベース方言固有の型変換ロジックを提供するトレイト。
pub trait TypeMapper: Send + Sync {
    /// SQL型文字列からColumnTypeへパース
    ///
    /// # Arguments
    /// * `sql_type` - データベースから取得した型文字列
    /// * `metadata` - 追加メタデータ
    ///
    /// # Returns
    /// 変換されたColumnType、変換できない場合はNone
    fn parse_sql_type(&self, sql_type: &str, metadata: &TypeMetadata) -> Option<ColumnType>;

    /// ColumnTypeからSQL型文字列へ変換
    ///
    /// # Arguments
    /// * `column_type` - 変換対象の内部型
    /// * `auto_increment` - 自動増分フラグ（PostgreSQLのSERIAL等）
    ///
    /// # Returns
    /// SQL型文字列
    fn format_sql_type(&self, column_type: &ColumnType, auto_increment: Option<bool>) -> String;

    /// 方言固有型のフォーマット
    ///
    /// DialectSpecific型のパラメータを解釈してSQL型文字列を生成します。
    fn format_dialect_specific(&self, kind: &str, params: &serde_json::Value) -> String;

    /// デフォルト型（パース失敗時のフォールバック）
    fn default_type(&self) -> ColumnType {
        ColumnType::TEXT
    }
}

/// PostgreSQL用型マッパー
pub struct PostgresTypeMapper;

/// MySQL用型マッパー
pub struct MySqlTypeMapper;

/// SQLite用型マッパー
pub struct SqliteTypeMapper;

/// 型マッピングサービス
///
/// 方言に依存しない共通インターフェースで型変換を提供します。
/// ColumnType ↔ SQL型文字列 の双方向変換を一元管理します。
pub struct TypeMappingService {
    dialect: Dialect,
    mapper: Box<dyn TypeMapper>,
}

impl TypeMappingService {
    /// 新しいTypeMappingServiceを作成
    pub fn new(dialect: Dialect) -> Self {
        let mapper: Box<dyn TypeMapper> = match dialect {
            Dialect::PostgreSQL => Box::new(PostgresTypeMapper),
            Dialect::MySQL => Box::new(MySqlTypeMapper),
            Dialect::SQLite => Box::new(SqliteTypeMapper),
        };
        Self { dialect, mapper }
    }

    /// 方言を取得
    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    /// ColumnType → SQL型文字列
    ///
    /// # Arguments
    /// * `column_type` - 変換対象の内部型
    ///
    /// # Returns
    /// SQL型文字列（例: "VARCHAR(255)", "INTEGER"）
    pub fn to_sql_type(&self, column_type: &ColumnType) -> String {
        self.to_sql_type_with_auto_increment(column_type, None)
    }

    /// ColumnType → SQL型文字列（自動増分オプション付き）
    ///
    /// # Arguments
    /// * `column_type` - 変換対象の内部型
    /// * `auto_increment` - 自動増分フラグ
    ///
    /// # Returns
    /// SQL型文字列
    pub fn to_sql_type_with_auto_increment(
        &self,
        column_type: &ColumnType,
        auto_increment: Option<bool>,
    ) -> String {
        self.mapper.format_sql_type(column_type, auto_increment)
    }

    /// SQL型文字列 → ColumnType
    ///
    /// # Arguments
    /// * `sql_type` - データベースから取得した型文字列
    /// * `metadata` - 追加メタデータ（precision, scaleなど）
    ///
    /// # Returns
    /// 内部型表現、パース失敗時はデフォルト型（TEXT）
    pub fn from_sql_type(&self, sql_type: &str, metadata: &TypeMetadata) -> Result<ColumnType> {
        Ok(self
            .mapper
            .parse_sql_type(sql_type, metadata)
            .unwrap_or_else(|| self.mapper.default_type()))
    }
}

// =============================================================================
// PostgreSQL TypeMapper 実装
// =============================================================================

impl TypeMapper for PostgresTypeMapper {
    fn parse_sql_type(&self, sql_type: &str, metadata: &TypeMetadata) -> Option<ColumnType> {
        match sql_type {
            "integer" | "int4" => Some(ColumnType::INTEGER { precision: Some(4) }),
            "smallint" | "int2" => Some(ColumnType::INTEGER { precision: Some(2) }),
            "bigint" | "int8" => Some(ColumnType::INTEGER { precision: Some(8) }),
            "character varying" | "varchar" => Some(ColumnType::VARCHAR {
                length: metadata.char_max_length.unwrap_or(255),
            }),
            "text" => Some(ColumnType::TEXT),
            "boolean" | "bool" => Some(ColumnType::BOOLEAN),
            "timestamp with time zone" | "timestamptz" => Some(ColumnType::TIMESTAMP {
                with_time_zone: Some(true),
            }),
            "timestamp without time zone" | "timestamp" => Some(ColumnType::TIMESTAMP {
                with_time_zone: Some(false),
            }),
            "json" => Some(ColumnType::JSON),
            "jsonb" => Some(ColumnType::JSONB),
            "numeric" | "decimal" => Some(ColumnType::DECIMAL {
                precision: metadata.numeric_precision.unwrap_or(10),
                scale: metadata.numeric_scale.unwrap_or(0),
            }),
            "real" | "float4" => Some(ColumnType::FLOAT),
            "double precision" | "float8" => Some(ColumnType::DOUBLE),
            "character" | "char" => Some(ColumnType::CHAR {
                length: metadata.char_max_length.unwrap_or(1),
            }),
            "date" => Some(ColumnType::DATE),
            "time with time zone" | "timetz" => Some(ColumnType::TIME {
                with_time_zone: Some(true),
            }),
            "time without time zone" | "time" => Some(ColumnType::TIME {
                with_time_zone: Some(false),
            }),
            "bytea" => Some(ColumnType::BLOB),
            "uuid" => Some(ColumnType::UUID),
            "USER-DEFINED" => {
                // ENUM型のチェック
                if let (Some(enum_names), Some(udt_name)) =
                    (&metadata.enum_names, &metadata.udt_name)
                {
                    if enum_names.contains(udt_name) {
                        return Some(ColumnType::Enum {
                            name: udt_name.clone(),
                        });
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn format_sql_type(&self, column_type: &ColumnType, auto_increment: Option<bool>) -> String {
        match column_type {
            ColumnType::INTEGER { precision } => {
                if auto_increment.unwrap_or(false) {
                    match precision {
                        Some(8) => "BIGSERIAL".to_string(),
                        Some(2) => "SMALLSERIAL".to_string(),
                        _ => "SERIAL".to_string(),
                    }
                } else {
                    match precision {
                        Some(2) => "SMALLINT".to_string(),
                        Some(8) => "BIGINT".to_string(),
                        _ => "INTEGER".to_string(),
                    }
                }
            }
            ColumnType::VARCHAR { length } => format!("VARCHAR({})", length),
            ColumnType::TEXT => "TEXT".to_string(),
            ColumnType::BOOLEAN => "BOOLEAN".to_string(),
            ColumnType::TIMESTAMP { with_time_zone } => {
                if with_time_zone.unwrap_or(false) {
                    "TIMESTAMP WITH TIME ZONE".to_string()
                } else {
                    "TIMESTAMP".to_string()
                }
            }
            ColumnType::JSON => "JSON".to_string(),
            ColumnType::JSONB => "JSONB".to_string(),
            ColumnType::DECIMAL { precision, scale } => {
                format!("NUMERIC({}, {})", precision, scale)
            }
            ColumnType::FLOAT => "REAL".to_string(),
            ColumnType::DOUBLE => "DOUBLE PRECISION".to_string(),
            ColumnType::CHAR { length } => format!("CHAR({})", length),
            ColumnType::DATE => "DATE".to_string(),
            ColumnType::TIME { with_time_zone } => {
                if with_time_zone.unwrap_or(false) {
                    "TIME WITH TIME ZONE".to_string()
                } else {
                    "TIME".to_string()
                }
            }
            ColumnType::BLOB => "BYTEA".to_string(),
            ColumnType::UUID => "UUID".to_string(),
            ColumnType::Enum { name } => quote_identifier_postgres(name),
            ColumnType::DialectSpecific { kind, params } => {
                self.format_dialect_specific(kind, params)
            }
        }
    }

    fn format_dialect_specific(&self, kind: &str, params: &serde_json::Value) -> String {
        // lengthパラメータがある場合（例: VARBIT(16)）
        if let Some(length) = params.get("length").and_then(|v| v.as_u64()) {
            return format!("{}({})", kind, length);
        }

        // arrayパラメータがtrueの場合（例: TEXT[]）
        if params
            .get("array")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return format!("{}[]", kind);
        }

        // valuesパラメータがある場合（例: ENUM('a', 'b', 'c')）
        if let Some(values) = params.get("values").and_then(|v| v.as_array()) {
            let values_str = values
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("'{}'", s))
                .collect::<Vec<_>>()
                .join(", ");
            return format!("{}({})", kind, values_str);
        }

        kind.to_string()
    }
}

// =============================================================================
// MySQL TypeMapper 実装
// =============================================================================

impl TypeMapper for MySqlTypeMapper {
    fn parse_sql_type(&self, sql_type: &str, metadata: &TypeMetadata) -> Option<ColumnType> {
        match sql_type {
            "int" | "integer" => Some(ColumnType::INTEGER {
                precision: metadata.numeric_precision,
            }),
            "smallint" => Some(ColumnType::INTEGER { precision: Some(2) }),
            "bigint" => Some(ColumnType::INTEGER { precision: Some(8) }),
            "tinyint" => Some(ColumnType::INTEGER {
                precision: metadata.numeric_precision,
            }),
            "varchar" => Some(ColumnType::VARCHAR {
                length: metadata.char_max_length.unwrap_or(255),
            }),
            "text" | "longtext" | "mediumtext" => Some(ColumnType::TEXT),
            "tinyint(1)" => Some(ColumnType::BOOLEAN),
            "datetime" | "timestamp" => Some(ColumnType::TIMESTAMP {
                with_time_zone: None,
            }),
            "json" => Some(ColumnType::JSON),
            "decimal" | "numeric" => Some(ColumnType::DECIMAL {
                precision: metadata.numeric_precision.unwrap_or(10),
                scale: metadata.numeric_scale.unwrap_or(0),
            }),
            "float" => Some(ColumnType::FLOAT),
            "double" => Some(ColumnType::DOUBLE),
            "char" => Some(ColumnType::CHAR {
                length: metadata.char_max_length.unwrap_or(1),
            }),
            "date" => Some(ColumnType::DATE),
            "time" => Some(ColumnType::TIME {
                with_time_zone: None,
            }),
            "blob" | "longblob" | "mediumblob" => Some(ColumnType::BLOB),
            _ => None,
        }
    }

    fn format_sql_type(&self, column_type: &ColumnType, _auto_increment: Option<bool>) -> String {
        match column_type {
            ColumnType::INTEGER { precision } => match precision {
                Some(2) => "SMALLINT".to_string(),
                Some(8) => "BIGINT".to_string(),
                _ => "INT".to_string(),
            },
            ColumnType::VARCHAR { length } => format!("VARCHAR({})", length),
            ColumnType::TEXT => "TEXT".to_string(),
            ColumnType::BOOLEAN => "BOOLEAN".to_string(),
            ColumnType::TIMESTAMP { .. } => "TIMESTAMP".to_string(),
            ColumnType::JSON => "JSON".to_string(),
            ColumnType::JSONB => "JSON".to_string(),
            ColumnType::DECIMAL { precision, scale } => {
                format!("DECIMAL({}, {})", precision, scale)
            }
            ColumnType::FLOAT => "FLOAT".to_string(),
            ColumnType::DOUBLE => "DOUBLE".to_string(),
            ColumnType::CHAR { length } => format!("CHAR({})", length),
            ColumnType::DATE => "DATE".to_string(),
            ColumnType::TIME { .. } => "TIME".to_string(),
            ColumnType::BLOB => "BLOB".to_string(),
            ColumnType::UUID => "CHAR(36)".to_string(),
            ColumnType::Enum { name } => quote_identifier_mysql(name),
            ColumnType::DialectSpecific { kind, params } => {
                self.format_dialect_specific(kind, params)
            }
        }
    }

    fn format_dialect_specific(&self, kind: &str, params: &serde_json::Value) -> String {
        // valuesパラメータがある場合（例: ENUM('a', 'b', 'c') または SET('a', 'b', 'c')）
        if let Some(values) = params.get("values").and_then(|v| v.as_array()) {
            let values_str = values
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("'{}'", s))
                .collect::<Vec<_>>()
                .join(", ");
            return format!("{}({})", kind, values_str);
        }

        // lengthパラメータがある場合
        if let Some(length) = params.get("length").and_then(|v| v.as_u64()) {
            return format!("{}({})", kind, length);
        }

        // unsignedパラメータがtrueの場合
        if params
            .get("unsigned")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return format!("{} UNSIGNED", kind);
        }

        kind.to_string()
    }
}

// =============================================================================
// SQLite TypeMapper 実装
// =============================================================================

impl TypeMapper for SqliteTypeMapper {
    fn parse_sql_type(&self, sql_type: &str, _metadata: &TypeMetadata) -> Option<ColumnType> {
        let upper = sql_type.to_uppercase();

        if upper.contains("INT") {
            Some(ColumnType::INTEGER { precision: None })
        } else if upper.contains("CHAR") || upper.contains("VARCHAR") {
            // VARCHAR(255) のような形式から長さを抽出
            if let Some(start) = sql_type.find('(') {
                if let Some(end) = sql_type.find(')') {
                    if let Ok(length) = sql_type[start + 1..end].parse::<u32>() {
                        return Some(ColumnType::VARCHAR { length });
                    }
                }
            }
            Some(ColumnType::VARCHAR { length: 255 })
        } else if upper == "TEXT" {
            Some(ColumnType::TEXT)
        } else if upper == "REAL" {
            Some(ColumnType::FLOAT)
        } else if upper == "BLOB" {
            Some(ColumnType::BLOB)
        } else {
            Some(ColumnType::TEXT)
        }
    }

    fn format_sql_type(&self, column_type: &ColumnType, _auto_increment: Option<bool>) -> String {
        match column_type {
            ColumnType::INTEGER { .. } => "INTEGER".to_string(),
            ColumnType::VARCHAR { .. } => "TEXT".to_string(),
            ColumnType::TEXT => "TEXT".to_string(),
            ColumnType::BOOLEAN => "INTEGER".to_string(),
            ColumnType::TIMESTAMP { .. } => "TEXT".to_string(),
            ColumnType::JSON => "TEXT".to_string(),
            ColumnType::JSONB => "TEXT".to_string(),
            ColumnType::DECIMAL { .. } => "TEXT".to_string(),
            ColumnType::FLOAT => "REAL".to_string(),
            ColumnType::DOUBLE => "REAL".to_string(),
            ColumnType::CHAR { .. } => "TEXT".to_string(),
            ColumnType::DATE => "TEXT".to_string(),
            ColumnType::TIME { .. } => "TEXT".to_string(),
            ColumnType::BLOB => "BLOB".to_string(),
            ColumnType::UUID => "TEXT".to_string(),
            // SQLiteはENUM型をサポートしないため、TEXTにフォールバック
            ColumnType::Enum { .. } => "TEXT".to_string(),
            ColumnType::DialectSpecific { kind, params } => {
                self.format_dialect_specific(kind, params)
            }
        }
    }

    fn format_dialect_specific(&self, kind: &str, _params: &serde_json::Value) -> String {
        // SQLiteは型アフィニティによる柔軟な型システムを持つため、
        // 方言固有型はそのまま出力
        kind.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // TypeMappingService 基本テスト
    // =========================================================================

    #[test]
    fn test_new_service_postgres() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(service.dialect(), Dialect::PostgreSQL);
    }

    #[test]
    fn test_new_service_mysql() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(service.dialect(), Dialect::MySQL);
    }

    #[test]
    fn test_new_service_sqlite() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(service.dialect(), Dialect::SQLite);
    }

    // =========================================================================
    // PostgreSQL to_sql_type テスト
    // =========================================================================

    #[test]
    fn test_postgres_integer() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: None }),
            "INTEGER"
        );
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: Some(2) }),
            "SMALLINT"
        );
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: Some(8) }),
            "BIGINT"
        );
    }

    #[test]
    fn test_postgres_serial() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type_with_auto_increment(
                &ColumnType::INTEGER { precision: None },
                Some(true)
            ),
            "SERIAL"
        );
        assert_eq!(
            service.to_sql_type_with_auto_increment(
                &ColumnType::INTEGER { precision: Some(8) },
                Some(true)
            ),
            "BIGSERIAL"
        );
        assert_eq!(
            service.to_sql_type_with_auto_increment(
                &ColumnType::INTEGER { precision: Some(2) },
                Some(true)
            ),
            "SMALLSERIAL"
        );
    }

    #[test]
    fn test_postgres_varchar() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::VARCHAR { length: 255 }),
            "VARCHAR(255)"
        );
    }

    #[test]
    fn test_postgres_timestamp() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::TIMESTAMP {
                with_time_zone: Some(true)
            }),
            "TIMESTAMP WITH TIME ZONE"
        );
        assert_eq!(
            service.to_sql_type(&ColumnType::TIMESTAMP {
                with_time_zone: Some(false)
            }),
            "TIMESTAMP"
        );
    }

    #[test]
    fn test_postgres_decimal() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::DECIMAL {
                precision: 10,
                scale: 2
            }),
            "NUMERIC(10, 2)"
        );
    }

    #[test]
    fn test_postgres_float_double() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(service.to_sql_type(&ColumnType::FLOAT), "REAL");
        assert_eq!(service.to_sql_type(&ColumnType::DOUBLE), "DOUBLE PRECISION");
    }

    #[test]
    fn test_postgres_blob() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(service.to_sql_type(&ColumnType::BLOB), "BYTEA");
    }

    #[test]
    fn test_postgres_uuid() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(service.to_sql_type(&ColumnType::UUID), "UUID");
    }

    #[test]
    fn test_postgres_json_jsonb() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        assert_eq!(service.to_sql_type(&ColumnType::JSON), "JSON");
        assert_eq!(service.to_sql_type(&ColumnType::JSONB), "JSONB");
    }

    // =========================================================================
    // MySQL to_sql_type テスト
    // =========================================================================

    #[test]
    fn test_mysql_integer() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: None }),
            "INT"
        );
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: Some(2) }),
            "SMALLINT"
        );
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: Some(8) }),
            "BIGINT"
        );
    }

    #[test]
    fn test_mysql_varchar() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(
            service.to_sql_type(&ColumnType::VARCHAR { length: 255 }),
            "VARCHAR(255)"
        );
    }

    #[test]
    fn test_mysql_boolean() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(service.to_sql_type(&ColumnType::BOOLEAN), "BOOLEAN");
    }

    #[test]
    fn test_mysql_uuid() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(service.to_sql_type(&ColumnType::UUID), "CHAR(36)");
    }

    #[test]
    fn test_mysql_jsonb_fallback() {
        let service = TypeMappingService::new(Dialect::MySQL);
        assert_eq!(service.to_sql_type(&ColumnType::JSONB), "JSON");
    }

    // =========================================================================
    // SQLite to_sql_type テスト
    // =========================================================================

    #[test]
    fn test_sqlite_integer() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(
            service.to_sql_type(&ColumnType::INTEGER { precision: None }),
            "INTEGER"
        );
    }

    #[test]
    fn test_sqlite_varchar() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(
            service.to_sql_type(&ColumnType::VARCHAR { length: 255 }),
            "TEXT"
        );
    }

    #[test]
    fn test_sqlite_boolean() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(service.to_sql_type(&ColumnType::BOOLEAN), "INTEGER");
    }

    #[test]
    fn test_sqlite_timestamp() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(
            service.to_sql_type(&ColumnType::TIMESTAMP {
                with_time_zone: Some(true)
            }),
            "TEXT"
        );
    }

    #[test]
    fn test_sqlite_decimal() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(
            service.to_sql_type(&ColumnType::DECIMAL {
                precision: 10,
                scale: 2
            }),
            "TEXT"
        );
    }

    #[test]
    fn test_sqlite_float_double() {
        let service = TypeMappingService::new(Dialect::SQLite);
        assert_eq!(service.to_sql_type(&ColumnType::FLOAT), "REAL");
        assert_eq!(service.to_sql_type(&ColumnType::DOUBLE), "REAL");
    }

    // =========================================================================
    // from_sql_type テスト（PostgreSQL）
    // =========================================================================

    #[test]
    fn test_postgres_parse_integer() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("integer", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { precision: Some(4) }));

        let result = service.from_sql_type("bigint", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { precision: Some(8) }));
    }

    #[test]
    fn test_postgres_parse_varchar() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let metadata = TypeMetadata {
            char_max_length: Some(100),
            ..Default::default()
        };

        let result = service
            .from_sql_type("character varying", &metadata)
            .unwrap();
        assert!(matches!(result, ColumnType::VARCHAR { length: 100 }));
    }

    #[test]
    fn test_postgres_parse_timestamp() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let metadata = TypeMetadata::default();

        let result = service
            .from_sql_type("timestamp with time zone", &metadata)
            .unwrap();
        assert!(matches!(
            result,
            ColumnType::TIMESTAMP {
                with_time_zone: Some(true)
            }
        ));

        let result = service
            .from_sql_type("timestamp without time zone", &metadata)
            .unwrap();
        assert!(matches!(
            result,
            ColumnType::TIMESTAMP {
                with_time_zone: Some(false)
            }
        ));
    }

    #[test]
    fn test_postgres_parse_enum() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let mut enum_names = HashSet::new();
        enum_names.insert("status".to_string());

        let metadata = TypeMetadata {
            udt_name: Some("status".to_string()),
            enum_names: Some(enum_names),
            ..Default::default()
        };

        let result = service.from_sql_type("USER-DEFINED", &metadata).unwrap();
        assert!(matches!(result, ColumnType::Enum { name } if name == "status"));
    }

    #[test]
    fn test_postgres_parse_unknown_returns_text() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("unknown_type", &metadata).unwrap();
        assert!(matches!(result, ColumnType::TEXT));
    }

    // =========================================================================
    // from_sql_type テスト（MySQL）
    // =========================================================================

    #[test]
    fn test_mysql_parse_int() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("int", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { .. }));
    }

    #[test]
    fn test_mysql_parse_varchar() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let metadata = TypeMetadata {
            char_max_length: Some(200),
            ..Default::default()
        };

        let result = service.from_sql_type("varchar", &metadata).unwrap();
        assert!(matches!(result, ColumnType::VARCHAR { length: 200 }));
    }

    // =========================================================================
    // from_sql_type テスト（SQLite）
    // =========================================================================

    #[test]
    fn test_sqlite_parse_integer() {
        let service = TypeMappingService::new(Dialect::SQLite);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("INTEGER", &metadata).unwrap();
        assert!(matches!(result, ColumnType::INTEGER { .. }));
    }

    #[test]
    fn test_sqlite_parse_varchar_with_length() {
        let service = TypeMappingService::new(Dialect::SQLite);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("VARCHAR(100)", &metadata).unwrap();
        assert!(matches!(result, ColumnType::VARCHAR { length: 100 }));
    }

    #[test]
    fn test_sqlite_parse_text() {
        let service = TypeMappingService::new(Dialect::SQLite);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("TEXT", &metadata).unwrap();
        assert!(matches!(result, ColumnType::TEXT));
    }

    #[test]
    fn test_sqlite_parse_real() {
        let service = TypeMappingService::new(Dialect::SQLite);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("REAL", &metadata).unwrap();
        assert!(matches!(result, ColumnType::FLOAT));
    }

    #[test]
    fn test_sqlite_parse_blob() {
        let service = TypeMappingService::new(Dialect::SQLite);
        let metadata = TypeMetadata::default();

        let result = service.from_sql_type("BLOB", &metadata).unwrap();
        assert!(matches!(result, ColumnType::BLOB));
    }

    // =========================================================================
    // DialectSpecific 型テスト
    // =========================================================================

    #[test]
    fn test_postgres_dialect_specific_with_length() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let params = serde_json::json!({ "length": 16 });
        let col_type = ColumnType::DialectSpecific {
            kind: "VARBIT".to_string(),
            params,
        };

        assert_eq!(service.to_sql_type(&col_type), "VARBIT(16)");
    }

    #[test]
    fn test_postgres_dialect_specific_array() {
        let service = TypeMappingService::new(Dialect::PostgreSQL);
        let params = serde_json::json!({ "array": true });
        let col_type = ColumnType::DialectSpecific {
            kind: "TEXT".to_string(),
            params,
        };

        assert_eq!(service.to_sql_type(&col_type), "TEXT[]");
    }

    #[test]
    fn test_mysql_dialect_specific_with_values() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let params = serde_json::json!({ "values": ["a", "b", "c"] });
        let col_type = ColumnType::DialectSpecific {
            kind: "ENUM".to_string(),
            params,
        };

        assert_eq!(service.to_sql_type(&col_type), "ENUM('a', 'b', 'c')");
    }

    #[test]
    fn test_mysql_dialect_specific_unsigned() {
        let service = TypeMappingService::new(Dialect::MySQL);
        let params = serde_json::json!({ "unsigned": true });
        let col_type = ColumnType::DialectSpecific {
            kind: "TINYINT".to_string(),
            params,
        };

        assert_eq!(service.to_sql_type(&col_type), "TINYINT UNSIGNED");
    }
}
