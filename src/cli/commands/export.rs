// exportコマンドハンドラー
//
// スキーマのエクスポート機能を実装します。
// - データベースからのスキーマ情報取得（INFORMATION_SCHEMA）
// - スキーマ定義のYAML形式への変換
// - ファイルへの出力または標準出力への表示
// - ダイアレクト固有の型の正規化

use crate::adapters::database::DatabaseConnectionService;
use crate::core::config::Config;
use crate::core::schema::{Column, ColumnType, Constraint, EnumDefinition, Index, Schema, Table};
use anyhow::{anyhow, Context, Result};
use sqlx::{AnyPool, Row};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

/// exportコマンドの入力パラメータ
#[derive(Debug, Clone)]
pub struct ExportCommand {
    /// プロジェクトのルートパス
    pub project_path: PathBuf,
    /// 環境名
    pub env: String,
    /// 出力先ディレクトリ（Noneの場合は標準出力）
    pub output_dir: Option<PathBuf>,
}

/// exportコマンドハンドラー
#[derive(Debug, Clone)]
pub struct ExportCommandHandler {}

#[derive(Debug, Clone)]
struct EnumRow {
    name: String,
    value: String,
    order: f64,
}

impl ExportCommandHandler {
    /// 新しいExportCommandHandlerを作成
    pub fn new() -> Self {
        Self {}
    }

    /// exportコマンドを実行
    ///
    /// # Arguments
    ///
    /// * `command` - exportコマンドのパラメータ
    ///
    /// # Returns
    ///
    /// 成功時はエクスポート結果のサマリー（または標準出力用のYAML）、失敗時はエラーメッセージ
    pub async fn execute(&self, command: &ExportCommand) -> Result<String> {
        // 設定ファイルを読み込む
        let config_path = command.project_path.join(Config::DEFAULT_CONFIG_PATH);
        if !config_path.exists() {
            return Err(anyhow!(
                "Config file not found: {:?}. Please initialize the project first with the `init` command.",
                config_path
            ));
        }

        let config =
            Config::from_file(&config_path).with_context(|| "Failed to read config file")?;

        // データベースに接続
        let db_config = config
            .get_database_config(&command.env)
            .with_context(|| format!("Config for environment '{}' not found", command.env))?;

        let db_service = DatabaseConnectionService::new();
        let pool = db_service
            .create_pool(config.dialect, &db_config)
            .await
            .with_context(|| "Failed to connect to database")?;

        // データベースからスキーマ情報を取得
        let schema = self
            .extract_schema_from_database(&pool, config.dialect)
            .await
            .with_context(|| "Failed to get schema information")?;

        // テーブル名のリストを取得
        let table_names: Vec<String> = schema.tables.keys().cloned().collect();

        // YAML形式にシリアライズ
        let yaml_content = serde_saphyr::to_string(&schema)
            .with_context(|| "Failed to serialize schema to YAML")?;

        // 出力先に応じて処理
        if let Some(output_dir) = &command.output_dir {
            // ディレクトリに出力
            fs::create_dir_all(output_dir)
                .with_context(|| format!("Failed to create output directory: {:?}", output_dir))?;

            let output_file = output_dir.join("schema.yaml");
            fs::write(&output_file, yaml_content)
                .with_context(|| format!("Failed to write schema file: {:?}", output_file))?;

            Ok(self.format_export_summary(&table_names, Some(output_dir)))
        } else {
            // 標準出力に出力（YAMLをそのまま返す）
            Ok(yaml_content)
        }
    }

    /// データベースからスキーマ情報を抽出
    async fn extract_schema_from_database(
        &self,
        pool: &AnyPool,
        dialect: crate::core::config::Dialect,
    ) -> Result<Schema> {
        let mut schema = Schema::new("1.0".to_string());

        let enum_names = if matches!(dialect, crate::core::config::Dialect::PostgreSQL) {
            let enums = self.get_enums_postgres(pool).await?;
            for enum_def in enums {
                schema.add_enum(enum_def);
            }
            Some(schema.enums.keys().cloned().collect::<HashSet<_>>())
        } else {
            None
        };

        // テーブル一覧を取得
        let table_names = self.get_table_names(pool, dialect).await?;

        for table_name in table_names {
            let mut table = Table::new(table_name.clone());

            // カラム情報を取得
            let columns = self
                .get_columns(pool, &table_name, dialect, enum_names.as_ref())
                .await?;
            for column in columns {
                table.add_column(column);
            }

            // インデックス情報を取得
            let indexes = self.get_indexes(pool, &table_name, dialect).await?;
            for index in indexes {
                table.add_index(index);
            }

            // 制約情報を取得
            let constraints = self.get_constraints(pool, &table_name, dialect).await?;
            for constraint in constraints {
                table.add_constraint(constraint);
            }

            schema.add_table(table);
        }

        Ok(schema)
    }

    /// テーブル名の一覧を取得
    async fn get_table_names(
        &self,
        pool: &AnyPool,
        dialect: crate::core::config::Dialect,
    ) -> Result<Vec<String>> {
        let sql = match dialect {
            crate::core::config::Dialect::PostgreSQL => {
                "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' ORDER BY table_name"
            }
            crate::core::config::Dialect::MySQL => {
                "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE() ORDER BY table_name"
            }
            crate::core::config::Dialect::SQLite => {
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != 'schema_migrations' ORDER BY name"
            }
        };

        let rows = sqlx::query(sql).fetch_all(pool).await?;

        let table_names = rows.iter().map(|row| row.get::<String, _>(0)).collect();

        Ok(table_names)
    }

    /// カラム情報を取得
    async fn get_columns(
        &self,
        pool: &AnyPool,
        table_name: &str,
        dialect: crate::core::config::Dialect,
        enum_names: Option<&HashSet<String>>,
    ) -> Result<Vec<Column>> {
        match dialect {
            crate::core::config::Dialect::SQLite => self.get_columns_sqlite(pool, table_name).await,
            crate::core::config::Dialect::PostgreSQL => {
                self.get_columns_postgres(pool, table_name, enum_names)
                    .await
            }
            crate::core::config::Dialect::MySQL => self.get_columns_mysql(pool, table_name).await,
        }
    }

    /// SQLiteのカラム情報を取得
    async fn get_columns_sqlite(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<Column>> {
        let sql = format!("PRAGMA table_info({})", table_name);
        let rows = sqlx::query(&sql).fetch_all(pool).await?;

        let mut columns = Vec::new();

        for row in rows {
            let name: String = row.get(1);
            let type_str: String = row.get(2);
            let not_null: i32 = row.get(3);
            let default_value: Option<String> = row.get(4);

            let column_type = self.parse_sqlite_type(&type_str);
            let nullable = not_null == 0;

            let mut column = Column::new(name, column_type, nullable);
            column.default_value = default_value;

            columns.push(column);
        }

        Ok(columns)
    }

    /// PostgreSQLのカラム情報を取得
    async fn get_columns_postgres(
        &self,
        pool: &AnyPool,
        table_name: &str,
        enum_names: Option<&HashSet<String>>,
    ) -> Result<Vec<Column>> {
        let sql = r#"
            SELECT column_name, data_type, is_nullable, column_default, character_maximum_length, numeric_precision, udt_name
            FROM information_schema.columns
            WHERE table_name = $1 AND table_schema = 'public'
            ORDER BY ordinal_position
        "#;

        let rows = sqlx::query(sql).bind(table_name).fetch_all(pool).await?;

        let mut columns = Vec::new();

        for row in rows {
            let name: String = row.get(0);
            let data_type: String = row.get(1);
            let is_nullable: String = row.get(2);
            let default_value: Option<String> = row.get(3);
            let char_max_length: Option<i32> = row.get(4);
            let numeric_precision: Option<i32> = row.get(5);
            let udt_name: Option<String> = row.get(6);

            let column_type = self.parse_postgres_type(
                &data_type,
                char_max_length,
                numeric_precision,
                udt_name.as_deref(),
                enum_names,
            );
            let nullable = is_nullable == "YES";

            let mut column = Column::new(name, column_type, nullable);
            column.default_value = default_value;

            columns.push(column);
        }

        Ok(columns)
    }

    /// MySQLのカラム情報を取得
    async fn get_columns_mysql(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<Column>> {
        let sql = r#"
            SELECT column_name, data_type, is_nullable, column_default, character_maximum_length, numeric_precision
            FROM information_schema.columns
            WHERE table_name = ? AND table_schema = DATABASE()
            ORDER BY ordinal_position
        "#;

        let rows = sqlx::query(sql).bind(table_name).fetch_all(pool).await?;

        let mut columns = Vec::new();

        for row in rows {
            let name: String = row.get(0);
            let data_type: String = row.get(1);
            let is_nullable: String = row.get(2);
            let default_value: Option<String> = row.get(3);
            let char_max_length: Option<i32> = row.get(4);
            let numeric_precision: Option<i32> = row.get(5);

            let column_type = self.parse_mysql_type(&data_type, char_max_length, numeric_precision);
            let nullable = is_nullable == "YES";

            let mut column = Column::new(name, column_type, nullable);
            column.default_value = default_value;

            columns.push(column);
        }

        Ok(columns)
    }

    /// SQLiteの型をパース
    fn parse_sqlite_type(&self, type_str: &str) -> ColumnType {
        let upper = type_str.to_uppercase();

        if upper.contains("INT") {
            ColumnType::INTEGER { precision: None }
        } else if upper.contains("CHAR") {
            // VARCHAR(255) のような形式から長さを抽出
            if let Some(start) = type_str.find('(') {
                if let Some(end) = type_str.find(')') {
                    if let Ok(length) = type_str[start + 1..end].parse::<u32>() {
                        return ColumnType::VARCHAR { length };
                    }
                }
            }
            ColumnType::VARCHAR { length: 255 }
        } else {
            // TEXT, REAL, BLOB, その他の型はすべてTEXTとして扱う
            ColumnType::TEXT
        }
    }

    /// PostgreSQLの型をパース
    fn parse_postgres_type(
        &self,
        data_type: &str,
        char_max_length: Option<i32>,
        numeric_precision: Option<i32>,
        udt_name: Option<&str>,
        enum_names: Option<&HashSet<String>>,
    ) -> ColumnType {
        match data_type {
            "integer" | "smallint" | "bigint" => ColumnType::INTEGER {
                precision: numeric_precision.map(|p| p as u32),
            },
            "character varying" | "varchar" => ColumnType::VARCHAR {
                length: char_max_length.unwrap_or(255) as u32,
            },
            "text" => ColumnType::TEXT,
            "boolean" => ColumnType::BOOLEAN,
            "timestamp with time zone" => ColumnType::TIMESTAMP {
                with_time_zone: Some(true),
            },
            "timestamp without time zone" => ColumnType::TIMESTAMP {
                with_time_zone: Some(false),
            },
            "json" | "jsonb" => ColumnType::JSON,
            "USER-DEFINED" => {
                if let (Some(enum_names), Some(enum_name)) = (enum_names, udt_name) {
                    if enum_names.contains(enum_name) {
                        return ColumnType::Enum {
                            name: enum_name.to_string(),
                        };
                    }
                }
                ColumnType::TEXT
            }
            _ => ColumnType::TEXT,
        }
    }

    async fn get_enums_postgres(&self, pool: &AnyPool) -> Result<Vec<EnumDefinition>> {
        let sql = r#"
            SELECT t.typname, e.enumlabel, e.enumsortorder
            FROM pg_type t
            JOIN pg_enum e ON t.oid = e.enumtypid
            JOIN pg_namespace n ON n.oid = t.typnamespace
            WHERE n.nspname = 'public'
            ORDER BY t.typname, e.enumsortorder
        "#;

        let rows = sqlx::query(sql).fetch_all(pool).await?;

        let mut entries = Vec::new();
        for row in rows {
            let name: String = row.get(0);
            let value: String = row.get(1);
            let order: f64 = row.get(2);
            entries.push(EnumRow { name, value, order });
        }

        Ok(Self::build_enum_definitions(entries))
    }

    fn build_enum_definitions(mut rows: Vec<EnumRow>) -> Vec<EnumDefinition> {
        rows.sort_by(|a, b| {
            let name_cmp = a.name.cmp(&b.name);
            if name_cmp == std::cmp::Ordering::Equal {
                a.order
                    .partial_cmp(&b.order)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                name_cmp
            }
        });

        let mut enums = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_values: Vec<String> = Vec::new();

        for row in rows {
            if current_name.as_deref() != Some(&row.name) {
                if let Some(name) = current_name.take() {
                    enums.push(EnumDefinition {
                        name,
                        values: current_values,
                    });
                    current_values = Vec::new();
                }
                current_name = Some(row.name);
            }
            current_values.push(row.value);
        }

        if let Some(name) = current_name {
            enums.push(EnumDefinition {
                name,
                values: current_values,
            });
        }

        enums
    }

    /// MySQLの型をパース
    fn parse_mysql_type(
        &self,
        data_type: &str,
        char_max_length: Option<i32>,
        numeric_precision: Option<i32>,
    ) -> ColumnType {
        match data_type {
            "int" | "smallint" | "bigint" | "tinyint" => ColumnType::INTEGER {
                precision: numeric_precision.map(|p| p as u32),
            },
            "varchar" => ColumnType::VARCHAR {
                length: char_max_length.unwrap_or(255) as u32,
            },
            "text" | "longtext" | "mediumtext" => ColumnType::TEXT,
            "tinyint(1)" => ColumnType::BOOLEAN,
            "datetime" | "timestamp" => ColumnType::TIMESTAMP {
                with_time_zone: None,
            },
            "json" => ColumnType::JSON,
            _ => ColumnType::TEXT,
        }
    }

    /// インデックス情報を取得
    async fn get_indexes(
        &self,
        pool: &AnyPool,
        table_name: &str,
        dialect: crate::core::config::Dialect,
    ) -> Result<Vec<Index>> {
        match dialect {
            crate::core::config::Dialect::SQLite => self.get_indexes_sqlite(pool, table_name).await,
            _ => Ok(Vec::new()), // PostgreSQLとMySQLは後で実装
        }
    }

    /// SQLiteのインデックス情報を取得
    async fn get_indexes_sqlite(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<Index>> {
        let sql = format!("PRAGMA index_list({})", table_name);
        let rows = sqlx::query(&sql).fetch_all(pool).await?;

        let mut indexes = Vec::new();

        for row in rows {
            let index_name: String = row.get(1);
            let is_unique: i32 = row.get(2);

            // システムインデックスをスキップ
            if index_name.starts_with("sqlite_") {
                continue;
            }

            // インデックスのカラムを取得
            let info_sql = format!("PRAGMA index_info({})", index_name);
            let info_rows = sqlx::query(&info_sql).fetch_all(pool).await?;

            let columns: Vec<String> = info_rows.iter().map(|r| r.get::<String, _>(2)).collect();

            let index = Index {
                name: index_name,
                columns,
                unique: is_unique == 1,
            };

            indexes.push(index);
        }

        Ok(indexes)
    }

    /// 制約情報を取得
    async fn get_constraints(
        &self,
        pool: &AnyPool,
        table_name: &str,
        dialect: crate::core::config::Dialect,
    ) -> Result<Vec<Constraint>> {
        match dialect {
            crate::core::config::Dialect::SQLite => {
                self.get_constraints_sqlite(pool, table_name).await
            }
            _ => Ok(Vec::new()), // PostgreSQLとMySQLは後で実装
        }
    }

    /// SQLiteの制約情報を取得
    async fn get_constraints_sqlite(
        &self,
        pool: &AnyPool,
        table_name: &str,
    ) -> Result<Vec<Constraint>> {
        let mut constraints = Vec::new();

        // PRIMARY KEY制約を取得
        let table_info_sql = format!("PRAGMA table_info({})", table_name);
        let rows = sqlx::query(&table_info_sql).fetch_all(pool).await?;

        let pk_columns: Vec<String> = rows
            .iter()
            .filter(|row| row.get::<i32, _>(5) > 0) // pk列が0より大きい
            .map(|row| row.get::<String, _>(1))
            .collect();

        if !pk_columns.is_empty() {
            constraints.push(Constraint::PRIMARY_KEY {
                columns: pk_columns,
            });
        }

        // FOREIGN KEY制約を取得
        let fk_sql = format!("PRAGMA foreign_key_list({})", table_name);
        let fk_rows = sqlx::query(&fk_sql).fetch_all(pool).await?;

        let mut fk_map: HashMap<i32, (String, Vec<String>, Vec<String>)> = HashMap::new();

        for row in fk_rows {
            let id: i32 = row.get(0);
            let ref_table: String = row.get(2);
            let from_col: String = row.get(3);
            let to_col: String = row.get(4);

            let entry = fk_map
                .entry(id)
                .or_insert_with(|| (ref_table.clone(), Vec::new(), Vec::new()));

            entry.1.push(from_col);
            entry.2.push(to_col);
        }

        for (_id, (ref_table, from_cols, to_cols)) in fk_map {
            constraints.push(Constraint::FOREIGN_KEY {
                columns: from_cols,
                referenced_table: ref_table,
                referenced_columns: to_cols,
            });
        }

        Ok(constraints)
    }

    /// エクスポート結果のサマリーをフォーマット
    pub fn format_export_summary(
        &self,
        table_names: &[String],
        output_dir: Option<&PathBuf>,
    ) -> String {
        let mut output = String::new();

        output.push_str("=== Schema Export Complete ===\n\n");

        output.push_str(&format!("Exported tables: {}\n\n", table_names.len()));

        for table_name in table_names {
            output.push_str(&format!("  - {}\n", table_name));
        }

        output.push('\n');

        if let Some(dir) = output_dir {
            output.push_str(&format!("Output: {:?}\n", dir.join("schema.yaml")));
        } else {
            output.push_str("Output: stdout\n");
        }

        output
    }
}

impl Default for ExportCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_handler() {
        let handler = ExportCommandHandler::new();
        assert!(format!("{:?}", handler).contains("ExportCommandHandler"));
    }

    #[test]
    fn test_parse_sqlite_type() {
        let handler = ExportCommandHandler::new();

        assert!(matches!(
            handler.parse_sqlite_type("INTEGER"),
            ColumnType::INTEGER { .. }
        ));
        assert!(matches!(
            handler.parse_sqlite_type("TEXT"),
            ColumnType::TEXT
        ));
        assert!(matches!(
            handler.parse_sqlite_type("VARCHAR(255)"),
            ColumnType::VARCHAR { length: 255 }
        ));
        // REAL and BLOB are mapped to TEXT in the current schema
        assert!(matches!(
            handler.parse_sqlite_type("REAL"),
            ColumnType::TEXT
        ));
        assert!(matches!(
            handler.parse_sqlite_type("BLOB"),
            ColumnType::TEXT
        ));
    }

    #[test]
    fn test_build_enum_definitions_orders_values() {
        let rows = vec![
            EnumRow {
                name: "status".to_string(),
                value: "inactive".to_string(),
                order: 2.0,
            },
            EnumRow {
                name: "status".to_string(),
                value: "active".to_string(),
                order: 1.0,
            },
            EnumRow {
                name: "role".to_string(),
                value: "admin".to_string(),
                order: 1.0,
            },
        ];

        let enums = ExportCommandHandler::build_enum_definitions(rows);
        assert_eq!(enums.len(), 2);
        assert_eq!(enums[0].name, "role");
        assert_eq!(enums[0].values, vec!["admin".to_string()]);
        assert_eq!(enums[1].name, "status");
        assert_eq!(
            enums[1].values,
            vec!["active".to_string(), "inactive".to_string()]
        );
    }

    #[test]
    fn test_parse_postgres_enum_type() {
        let handler = ExportCommandHandler::new();
        let mut enum_names = HashSet::new();
        enum_names.insert("status".to_string());

        let col_type = handler.parse_postgres_type(
            "USER-DEFINED",
            None,
            None,
            Some("status"),
            Some(&enum_names),
        );

        assert!(matches!(
            col_type,
            ColumnType::Enum { name } if name == "status"
        ));
    }

    #[test]
    fn test_format_export_summary() {
        let handler = ExportCommandHandler::new();

        let table_names = vec!["users".to_string(), "posts".to_string()];
        let output_path = Some(PathBuf::from("/test/output"));

        let summary = handler.format_export_summary(&table_names, output_path.as_ref());

        assert!(summary.contains("Export Complete"));
        assert!(summary.contains("2"));
        assert!(summary.contains("users"));
        assert!(summary.contains("posts"));
    }
}
