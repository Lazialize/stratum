// 型マッピングサービス
//
// 方言に依存しない共通インターフェースで ColumnType <-> SQL型文字列 の
// 双方向変換を一元管理します。

pub mod common;
mod mysql_mapper;
mod postgres_mapper;
mod sqlite_mapper;

pub use mysql_mapper::MySqlTypeMapper;
pub use postgres_mapper::PostgresTypeMapper;
pub use sqlite_mapper::SqliteTypeMapper;

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
    /// ENUM値のリスト（MySQL用）
    pub enum_values: Option<Vec<String>>,
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

/// 型マッピングサービス
///
/// 方言に依存しない共通インターフェースで型変換を提供します。
/// ColumnType <-> SQL型文字列 の双方向変換を一元管理します。
pub struct TypeMappingService {
    dialect: Dialect,
    mapper: Box<dyn TypeMapper>,
}

impl Clone for TypeMappingService {
    fn clone(&self) -> Self {
        Self::new(self.dialect)
    }
}

impl std::fmt::Debug for TypeMappingService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeMappingService")
            .field("dialect", &self.dialect)
            .finish()
    }
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

    /// ColumnType -> SQL型文字列
    ///
    /// # Arguments
    /// * `column_type` - 変換対象の内部型
    ///
    /// # Returns
    /// SQL型文字列（例: "VARCHAR(255)", "INTEGER"）
    pub fn to_sql_type(&self, column_type: &ColumnType) -> String {
        self.to_sql_type_with_auto_increment(column_type, None)
    }

    /// ColumnType -> SQL型文字列（自動増分オプション付き）
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

    /// SQL型文字列 -> ColumnType
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Dialect;

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
}
