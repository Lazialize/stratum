// スキーマ変換サービス
//
// DatabaseIntrospector から取得した生データを内部モデルに変換するサービス。
// TypeMappingService を使用して SQL 型文字列を ColumnType に変換します。

mod builder;
mod converters;

#[cfg(test)]
mod tests;

use crate::adapters::database_introspector::{RawColumnInfo, RawConstraintInfo, RawIndexInfo};
use crate::adapters::type_mapping::TypeMappingService;
use crate::core::config::Dialect;
use std::collections::HashSet;

/// 生のテーブル情報
///
/// DatabaseIntrospector から取得した全テーブル情報を保持します。
#[derive(Debug, Clone)]
pub struct RawTableInfo {
    /// テーブル名
    pub name: String,
    /// カラム情報
    pub columns: Vec<RawColumnInfo>,
    /// インデックス情報
    pub indexes: Vec<RawIndexInfo>,
    /// 制約情報
    pub constraints: Vec<RawConstraintInfo>,
}

/// スキーマ変換サービス
///
/// 生のデータベース情報を内部スキーマモデルに変換します。
pub struct SchemaConversionService {
    type_mapping: TypeMappingService,
    /// 既知のENUM名（PostgreSQL用）
    enum_names: HashSet<String>,
}

impl SchemaConversionService {
    /// 新しい SchemaConversionService を作成
    pub fn new(dialect: Dialect) -> Self {
        Self {
            type_mapping: TypeMappingService::new(dialect),
            enum_names: HashSet::new(),
        }
    }

    /// ENUM名を設定
    ///
    /// PostgreSQL の ENUM 型を正しく認識するために、
    /// ENUM 名のセットを事前に設定します。
    pub fn with_enum_names(mut self, enum_names: HashSet<String>) -> Self {
        self.enum_names = enum_names;
        self
    }
}
