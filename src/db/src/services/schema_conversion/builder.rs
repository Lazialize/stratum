use super::{RawTableInfo, SchemaConversionService};
use crate::adapters::database_introspector::{RawEnumInfo, RawViewInfo};
use crate::core::schema::{Schema, Table, View};
use anyhow::{Context, Result};
use std::collections::HashSet;

impl SchemaConversionService {
    /// 生のテーブル情報を内部モデルに変換
    pub fn convert_table(&self, raw: &RawTableInfo) -> Result<Table> {
        let mut table = Table::new(raw.name.clone());

        // カラムを変換
        for raw_column in &raw.columns {
            let column = self
                .convert_column(raw_column)
                .with_context(|| format!("Failed to convert column in table '{}'", raw.name))?;
            table.add_column(column);
        }

        // インデックスを変換
        for raw_index in &raw.indexes {
            let index = self
                .convert_index(raw_index)
                .with_context(|| format!("Failed to convert index in table '{}'", raw.name))?;
            table.add_index(index);
        }

        // unique: true のインデックスのカラムセットを収集
        // UNIQUE制約との重複を除外するために使用
        let unique_index_column_sets: Vec<HashSet<&str>> = raw
            .indexes
            .iter()
            .filter(|idx| idx.unique)
            .map(|idx| {
                idx.columns
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<HashSet<_>>()
            })
            .collect();

        // 制約を変換（UNIQUEインデックスと重複するUNIQUE制約をスキップ）
        for raw_constraint in &raw.constraints {
            // UNIQUE制約がユニークインデックスと同じカラムセットの場合はスキップ
            if is_duplicate_unique_constraint(raw_constraint, &unique_index_column_sets) {
                continue; // ユニークインデックスで既にカバー済み
            }

            let constraint = self
                .convert_constraint(raw_constraint)
                .with_context(|| format!("Failed to convert constraint in table '{}'", raw.name))?;
            table.add_constraint(constraint);
        }

        Ok(table)
    }

    /// 複数のテーブル情報から Schema を構築
    pub fn build_schema(
        &self,
        raw_tables: Vec<RawTableInfo>,
        raw_enums: Vec<RawEnumInfo>,
    ) -> Result<Schema> {
        self.build_schema_with_views(raw_tables, raw_enums, Vec::new())
    }

    /// 複数のテーブル情報とビュー情報から Schema を構築
    pub fn build_schema_with_views(
        &self,
        raw_tables: Vec<RawTableInfo>,
        raw_enums: Vec<RawEnumInfo>,
        raw_views: Vec<RawViewInfo>,
    ) -> Result<Schema> {
        let mut schema = Schema::new("1.0".to_string());

        // ENUMを変換
        for raw_enum in raw_enums {
            let enum_def = self
                .convert_enum(&raw_enum)
                .with_context(|| format!("Failed to convert enum '{}'", raw_enum.name))?;
            schema.add_enum(enum_def);
        }

        // テーブルを変換
        for raw_table in raw_tables {
            let table = self
                .convert_table(&raw_table)
                .with_context(|| format!("Failed to convert table '{}'", raw_table.name))?;
            schema.add_table(table);
        }

        // Viewを変換（マテリアライズドビューは除外）
        for raw_view in raw_views {
            if !raw_view.is_materialized {
                let view = View::new(raw_view.name, raw_view.definition);
                schema.add_view(view);
            }
        }

        Ok(schema)
    }
}

fn is_duplicate_unique_constraint(
    raw_constraint: &crate::adapters::database_introspector::RawConstraintInfo,
    unique_index_column_sets: &[HashSet<&str>],
) -> bool {
    if let crate::adapters::database_introspector::RawConstraintInfo::Unique { columns } =
        raw_constraint
    {
        let constraint_cols: HashSet<&str> = columns.iter().map(|c| c.as_str()).collect();
        return unique_index_column_sets.contains(&constraint_cols);
    }

    false
}
