use super::SchemaConversionService;
use crate::adapters::database_introspector::{
    RawColumnInfo, RawConstraintInfo, RawEnumInfo, RawIndexInfo,
};
use crate::adapters::type_mapping::TypeMetadata;
use crate::core::schema::{Column, Constraint, EnumDefinition, Index, ReferentialAction};
use anyhow::{Context, Result};

impl SchemaConversionService {
    /// 生のカラム情報を内部モデルに変換
    ///
    /// TypeMappingService を使用して SQL 型文字列を ColumnType に変換します。
    pub fn convert_column(&self, raw: &RawColumnInfo) -> Result<Column> {
        let metadata = TypeMetadata {
            char_max_length: raw.char_max_length.map(|l| l as u32),
            numeric_precision: raw.numeric_precision.map(|p| p as u32),
            numeric_scale: raw.numeric_scale.map(|s| s as u32),
            udt_name: raw.udt_name.clone(),
            enum_names: if self.enum_names.is_empty() {
                None
            } else {
                Some(self.enum_names.clone())
            },
        };

        let column_type = self
            .type_mapping
            .from_sql_type(&raw.data_type, &metadata)
            .with_context(|| format!("Failed to parse column type for '{}'", raw.name))?;

        let mut column = Column::new(raw.name.clone(), column_type, raw.is_nullable);

        // PostgreSQL の SERIAL カラムは nextval('...') をデフォルト値として持つ
        // これを auto_increment: true として認識し、default_value は省略する
        if let Some(ref default) = raw.default_value {
            if default.contains("nextval(") {
                column.auto_increment = Some(true);
            } else {
                column.default_value = Some(default.clone());
            }
        }

        // SQLite の AUTOINCREMENT 検出結果を反映
        if let Some(true) = raw.auto_increment {
            column.auto_increment = Some(true);
        }

        Ok(column)
    }

    /// 生のインデックス情報を内部モデルに変換
    pub fn convert_index(&self, raw: &RawIndexInfo) -> Result<Index> {
        Ok(Index {
            name: raw.name.clone(),
            columns: raw.columns.clone(),
            unique: raw.unique,
        })
    }

    /// 生の制約情報を内部モデルに変換
    pub fn convert_constraint(&self, raw: &RawConstraintInfo) -> Result<Constraint> {
        let constraint = match raw {
            RawConstraintInfo::PrimaryKey { columns } => Constraint::PRIMARY_KEY {
                columns: columns.clone(),
            },
            RawConstraintInfo::ForeignKey {
                columns,
                referenced_table,
                referenced_columns,
                on_delete,
            } => {
                let on_delete_action = on_delete.as_deref().and_then(parse_on_delete_action);
                Constraint::FOREIGN_KEY {
                    columns: columns.clone(),
                    referenced_table: referenced_table.clone(),
                    referenced_columns: referenced_columns.clone(),
                    on_delete: on_delete_action,
                    on_update: None,
                }
            }
            RawConstraintInfo::Unique { columns } => Constraint::UNIQUE {
                columns: columns.clone(),
            },
            RawConstraintInfo::Check {
                columns,
                expression,
            } => Constraint::CHECK {
                columns: columns.clone(),
                check_expression: expression.clone(),
            },
        };

        Ok(constraint)
    }

    /// 生のENUM情報を内部モデルに変換
    pub fn convert_enum(&self, raw: &RawEnumInfo) -> Result<EnumDefinition> {
        Ok(EnumDefinition {
            name: raw.name.clone(),
            values: raw.values.clone(),
        })
    }
}

fn parse_on_delete_action(action: &str) -> Option<ReferentialAction> {
    match action {
        "CASCADE" => Some(ReferentialAction::Cascade),
        "SET NULL" => Some(ReferentialAction::SetNull),
        "SET DEFAULT" => Some(ReferentialAction::SetDefault),
        "RESTRICT" => Some(ReferentialAction::Restrict),
        // NO ACTION はデフォルトなので省略
        _ => None,
    }
}
