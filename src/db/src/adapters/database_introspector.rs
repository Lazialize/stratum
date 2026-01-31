// データベースイントロスペクター
//
// データベースからスキーマ情報を取得するための抽象化レイヤー。
// 各方言固有のINFORMATION_SCHEMA/PRAGMAクエリを実装します。

use anyhow::Result;
use async_trait::async_trait;
use sqlx::AnyPool;
use sqlx::Row;

/// MySQL の information_schema は多くのカラムを BLOB/VARBINARY 型で返す。
/// sqlx の Any ドライバは String として直接デコードできないため、
/// まず String を試し、失敗したら Vec<u8> → String 変換にフォールバックする。
fn mysql_get_string(row: &sqlx::any::AnyRow, index: usize) -> String {
    row.try_get::<String, _>(index).unwrap_or_else(|_| {
        let bytes: Vec<u8> = row.get(index);
        String::from_utf8_lossy(&bytes).to_string()
    })
}

/// MySQL 向け: NULL 可能な文字列カラムを安全に取得する
fn mysql_get_optional_string(row: &sqlx::any::AnyRow, index: usize) -> Option<String> {
    // まず Option<String> を試す
    if let Ok(val) = row.try_get::<Option<String>, _>(index) {
        return val;
    }
    // BLOB の場合は Option<Vec<u8>> → String 変換
    if let Ok(Some(bytes)) = row.try_get::<Option<Vec<u8>>, _>(index) {
        return Some(String::from_utf8_lossy(&bytes).to_string());
    }
    None
}

/// 生のカラム情報（DB固有フォーマット）
///
/// データベースから取得したカラム情報を保持する構造体。
/// TypeMappingService で ColumnType に変換されます。
#[derive(Debug, Clone)]
pub struct RawColumnInfo {
    /// カラム名
    pub name: String,
    /// データ型（DB固有の型文字列）
    pub data_type: String,
    /// NULL許可フラグ
    pub is_nullable: bool,
    /// デフォルト値
    pub default_value: Option<String>,
    /// 文字型の最大長
    pub char_max_length: Option<i32>,
    /// 数値型の精度
    pub numeric_precision: Option<i32>,
    /// 数値型のスケール
    pub numeric_scale: Option<i32>,
    /// ユーザー定義型名（PostgreSQLのENUM等）
    pub udt_name: Option<String>,
    /// 自動増分フラグ（SQLite AUTOINCREMENT検出用）
    pub auto_increment: Option<bool>,
}

/// 生のインデックス情報（DB固有フォーマット）
#[derive(Debug, Clone)]
pub struct RawIndexInfo {
    /// インデックス名
    pub name: String,
    /// インデックス対象のカラム
    pub columns: Vec<String>,
    /// ユニーク制約フラグ
    pub unique: bool,
}

/// 生の制約情報（DB固有フォーマット）
#[derive(Debug, Clone)]
pub enum RawConstraintInfo {
    /// プライマリキー制約
    PrimaryKey { columns: Vec<String> },
    /// 外部キー制約
    ForeignKey {
        columns: Vec<String>,
        referenced_table: String,
        referenced_columns: Vec<String>,
        /// ON DELETE アクション（例: "CASCADE", "SET NULL", "RESTRICT", "NO ACTION"）
        on_delete: Option<String>,
    },
    /// ユニーク制約
    Unique { columns: Vec<String> },
    /// CHECK制約
    Check {
        columns: Vec<String>,
        expression: String,
    },
}

/// 生のENUM情報（PostgreSQL専用）
#[derive(Debug, Clone)]
pub struct RawEnumInfo {
    /// ENUM型名
    pub name: String,
    /// ENUM値のリスト（順序付き）
    pub values: Vec<String>,
}

/// データベーススキーマ取得インターフェース
///
/// 各データベース方言固有のイントロスペクション処理を抽象化します。
#[async_trait]
pub trait DatabaseIntrospector: Send + Sync {
    /// テーブル名一覧を取得
    async fn get_table_names(&self, pool: &AnyPool) -> Result<Vec<String>>;

    /// カラム情報を取得
    async fn get_columns(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawColumnInfo>>;

    /// インデックス情報を取得
    async fn get_indexes(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawIndexInfo>>;

    /// 制約情報を取得
    async fn get_constraints(
        &self,
        pool: &AnyPool,
        table_name: &str,
    ) -> Result<Vec<RawConstraintInfo>>;

    /// ENUM定義を取得（PostgreSQL専用、他方言では空を返す）
    async fn get_enums(&self, pool: &AnyPool) -> Result<Vec<RawEnumInfo>>;
}

/// PostgreSQL用イントロスペクター
pub struct PostgresIntrospector;

/// MySQL用イントロスペクター
pub struct MySqlIntrospector;

/// SQLite用イントロスペクター
pub struct SqliteIntrospector;

/// 方言に応じたイントロスペクターを作成
pub fn create_introspector(dialect: crate::core::config::Dialect) -> Box<dyn DatabaseIntrospector> {
    match dialect {
        crate::core::config::Dialect::PostgreSQL => Box::new(PostgresIntrospector),
        crate::core::config::Dialect::MySQL => Box::new(MySqlIntrospector),
        crate::core::config::Dialect::SQLite => Box::new(SqliteIntrospector),
    }
}

// =============================================================================
// PostgreSQL イントロスペクター実装
// =============================================================================

#[async_trait]
impl DatabaseIntrospector for PostgresIntrospector {
    async fn get_table_names(&self, pool: &AnyPool) -> Result<Vec<String>> {
        use sqlx::Row;

        let sql = r#"
            SELECT table_name::text
            FROM information_schema.tables
            WHERE table_schema = 'public'
                AND table_name != 'schema_migrations'
            ORDER BY table_name
        "#;

        let rows = sqlx::query(sql).fetch_all(pool).await?;
        let table_names = rows.iter().map(|row| row.get::<String, _>(0)).collect();

        Ok(table_names)
    }

    async fn get_columns(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawColumnInfo>> {
        use sqlx::Row;

        let sql = r#"
            SELECT
                column_name::text,
                data_type::text,
                is_nullable::text,
                column_default::text,
                character_maximum_length::integer,
                numeric_precision::integer,
                numeric_scale::integer,
                udt_name::text
            FROM information_schema.columns
            WHERE table_name = $1 AND table_schema = 'public'
            ORDER BY ordinal_position
        "#;

        let rows = sqlx::query(sql).bind(table_name).fetch_all(pool).await?;

        let columns = rows
            .iter()
            .map(|row| RawColumnInfo {
                name: row.get(0),
                data_type: row.get(1),
                is_nullable: row.get::<String, _>(2) == "YES",
                default_value: row.get(3),
                char_max_length: row.get(4),
                numeric_precision: row.get(5),
                numeric_scale: row.get(6),
                udt_name: row.get(7),
                auto_increment: None,
            })
            .collect();

        Ok(columns)
    }

    async fn get_indexes(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawIndexInfo>> {
        use sqlx::Row;

        let sql = r#"
            SELECT
                i.relname::text as index_name,
                a.attname::text as column_name,
                ix.indisunique as is_unique
            FROM pg_class t
            JOIN pg_index ix ON t.oid = ix.indrelid
            JOIN pg_class i ON i.oid = ix.indexrelid
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
            JOIN pg_namespace n ON n.oid = t.relnamespace
            WHERE t.relkind = 'r'
                AND t.relname = $1
                AND n.nspname = 'public'
                AND NOT ix.indisprimary
            ORDER BY i.relname, a.attnum
        "#;

        let rows = sqlx::query(sql).bind(table_name).fetch_all(pool).await?;

        // グループ化してインデックスごとにまとめる
        let mut index_map: std::collections::HashMap<String, (Vec<String>, bool)> =
            std::collections::HashMap::new();

        for row in rows {
            let index_name: String = row.get(0);
            let column_name: String = row.get(1);
            let is_unique: bool = row.get(2);

            let entry = index_map
                .entry(index_name)
                .or_insert_with(|| (Vec::new(), is_unique));
            entry.0.push(column_name);
        }

        let indexes = index_map
            .into_iter()
            .map(|(name, (columns, unique))| RawIndexInfo {
                name,
                columns,
                unique,
            })
            .collect();

        Ok(indexes)
    }

    async fn get_constraints(
        &self,
        pool: &AnyPool,
        table_name: &str,
    ) -> Result<Vec<RawConstraintInfo>> {
        use sqlx::Row;

        let mut constraints = Vec::new();

        // PRIMARY KEY
        let pk_sql = r#"
            SELECT a.attname::text
            FROM pg_index i
            JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
            JOIN pg_class c ON c.oid = i.indrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE i.indisprimary
                AND c.relname = $1
                AND n.nspname = 'public'
            ORDER BY array_position(i.indkey, a.attnum)
        "#;

        let pk_rows = sqlx::query(pk_sql).bind(table_name).fetch_all(pool).await?;
        let pk_columns: Vec<String> = pk_rows.iter().map(|row| row.get(0)).collect();

        if !pk_columns.is_empty() {
            constraints.push(RawConstraintInfo::PrimaryKey {
                columns: pk_columns,
            });
        }

        // FOREIGN KEY
        // 制約名でグループ化して、同一テーブルへの複数FKを正しく区別する
        // pg_constraint から on_delete アクションも取得
        let fk_sql = r#"
            SELECT
                tc.constraint_name::text,
                kcu.column_name::text,
                ccu.table_name::text AS referenced_table,
                ccu.column_name::text AS referenced_column,
                CASE pgc.confdeltype
                    WHEN 'a' THEN 'NO ACTION'
                    WHEN 'r' THEN 'RESTRICT'
                    WHEN 'c' THEN 'CASCADE'
                    WHEN 'n' THEN 'SET NULL'
                    WHEN 'd' THEN 'SET DEFAULT'
                    ELSE 'NO ACTION'
                END::text AS on_delete
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name
                AND tc.table_schema = kcu.table_schema
            JOIN information_schema.constraint_column_usage ccu
                ON ccu.constraint_name = tc.constraint_name
                AND ccu.table_schema = tc.table_schema
            LEFT JOIN pg_constraint pgc
                ON pgc.conname = tc.constraint_name::name
                AND pgc.connamespace = (SELECT oid FROM pg_namespace WHERE nspname = 'public')
            WHERE tc.constraint_type = 'FOREIGN KEY'
                AND tc.table_name = $1
                AND tc.table_schema = 'public'
            ORDER BY tc.constraint_name, kcu.ordinal_position
        "#;

        let fk_rows = sqlx::query(fk_sql).bind(table_name).fetch_all(pool).await?;

        // 制約名でグループ化（複合外部キー対応）
        // (referenced_table, columns, referenced_columns, on_delete)
        let mut fk_map: std::collections::HashMap<
            String,
            (String, Vec<String>, Vec<String>, Option<String>),
        > = std::collections::HashMap::new();

        for row in &fk_rows {
            let constraint_name: String = row.get(0);
            let column: String = row.get(1);
            let ref_table: String = row.get(2);
            let ref_column: String = row.get(3);
            let on_delete: Option<String> = row.get(4);

            let entry = fk_map
                .entry(constraint_name)
                .or_insert_with(|| (ref_table.clone(), Vec::new(), Vec::new(), on_delete));
            entry.1.push(column);
            entry.2.push(ref_column);
        }

        for (_constraint_name, (ref_table, columns, ref_columns, on_delete)) in fk_map {
            constraints.push(RawConstraintInfo::ForeignKey {
                columns,
                referenced_table: ref_table,
                referenced_columns: ref_columns,
                on_delete,
            });
        }

        // UNIQUE (インデックスとは別の制約として取得)
        // 制約名でグループ化して、複数のUNIQUE制約を正しく区別する
        let unique_sql = r#"
            SELECT tc.constraint_name::text, kcu.column_name::text
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name
                AND tc.table_schema = kcu.table_schema
            WHERE tc.constraint_type = 'UNIQUE'
                AND tc.table_name = $1
                AND tc.table_schema = 'public'
            ORDER BY tc.constraint_name, kcu.ordinal_position
        "#;

        let unique_rows = sqlx::query(unique_sql)
            .bind(table_name)
            .fetch_all(pool)
            .await?;

        // 制約名でグループ化
        let mut unique_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for row in unique_rows {
            let constraint_name: String = row.get(0);
            let column: String = row.get(1);

            unique_map.entry(constraint_name).or_default().push(column);
        }

        for (_constraint_name, columns) in unique_map {
            constraints.push(RawConstraintInfo::Unique { columns });
        }

        Ok(constraints)
    }

    async fn get_enums(&self, pool: &AnyPool) -> Result<Vec<RawEnumInfo>> {
        use sqlx::Row;

        let sql = r#"
            SELECT t.typname::text, e.enumlabel::text, e.enumsortorder::double precision
            FROM pg_type t
            JOIN pg_enum e ON t.oid = e.enumtypid
            JOIN pg_namespace n ON n.oid = t.typnamespace
            WHERE n.nspname = 'public'
            ORDER BY t.typname, e.enumsortorder
        "#;

        let rows = sqlx::query(sql).fetch_all(pool).await?;

        // ENUM名ごとにグループ化
        let mut enum_map: std::collections::HashMap<String, Vec<(String, f64)>> =
            std::collections::HashMap::new();

        for row in rows {
            let name: String = row.get(0);
            let value: String = row.get(1);
            let order: f64 = row.get(2);

            enum_map.entry(name).or_default().push((value, order));
        }

        let enums = enum_map
            .into_iter()
            .map(|(name, mut values)| {
                values.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                RawEnumInfo {
                    name,
                    values: values.into_iter().map(|(v, _)| v).collect(),
                }
            })
            .collect();

        Ok(enums)
    }
}

// =============================================================================
// MySQL イントロスペクター実装
// =============================================================================

#[async_trait]
impl DatabaseIntrospector for MySqlIntrospector {
    async fn get_table_names(&self, pool: &AnyPool) -> Result<Vec<String>> {
        let sql = r#"
            SELECT table_name
            FROM information_schema.tables
            WHERE table_schema = DATABASE()
                AND table_name != 'schema_migrations'
            ORDER BY table_name
        "#;

        let rows = sqlx::query(sql).fetch_all(pool).await?;
        let table_names = rows.iter().map(|row| mysql_get_string(row, 0)).collect();

        Ok(table_names)
    }

    async fn get_columns(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawColumnInfo>> {
        use sqlx::Row;

        let sql = r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                column_default,
                character_maximum_length,
                numeric_precision,
                numeric_scale
            FROM information_schema.columns
            WHERE table_name = ? AND table_schema = DATABASE()
            ORDER BY ordinal_position
        "#;

        let rows = sqlx::query(sql).bind(table_name).fetch_all(pool).await?;

        let columns = rows
            .iter()
            .map(|row| RawColumnInfo {
                name: mysql_get_string(row, 0),
                data_type: mysql_get_string(row, 1),
                is_nullable: mysql_get_string(row, 2) == "YES",
                default_value: mysql_get_optional_string(row, 3),
                char_max_length: row.get(4),
                numeric_precision: row.get(5),
                numeric_scale: row.get(6),
                udt_name: None,
                auto_increment: None,
            })
            .collect();

        Ok(columns)
    }

    async fn get_indexes(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawIndexInfo>> {
        use sqlx::Row;

        let sql = r#"
            SELECT
                index_name,
                column_name,
                non_unique
            FROM information_schema.statistics
            WHERE table_name = ? AND table_schema = DATABASE()
                AND index_name != 'PRIMARY'
            ORDER BY index_name, seq_in_index
        "#;

        let rows = sqlx::query(sql).bind(table_name).fetch_all(pool).await?;

        let mut index_map: std::collections::HashMap<String, (Vec<String>, bool)> =
            std::collections::HashMap::new();

        for row in rows {
            let index_name = mysql_get_string(&row, 0);
            let column_name = mysql_get_string(&row, 1);
            let non_unique: i32 = row.get(2);

            let entry = index_map
                .entry(index_name)
                .or_insert_with(|| (Vec::new(), non_unique == 0));
            entry.0.push(column_name);
        }

        let indexes = index_map
            .into_iter()
            .map(|(name, (columns, unique))| RawIndexInfo {
                name,
                columns,
                unique,
            })
            .collect();

        Ok(indexes)
    }

    async fn get_constraints(
        &self,
        pool: &AnyPool,
        table_name: &str,
    ) -> Result<Vec<RawConstraintInfo>> {
        let mut constraints = Vec::new();

        // PRIMARY KEY
        let pk_sql = r#"
            SELECT column_name
            FROM information_schema.statistics
            WHERE table_name = ? AND table_schema = DATABASE()
                AND index_name = 'PRIMARY'
            ORDER BY seq_in_index
        "#;

        let pk_rows = sqlx::query(pk_sql).bind(table_name).fetch_all(pool).await?;
        let pk_columns: Vec<String> = pk_rows.iter().map(|row| mysql_get_string(row, 0)).collect();

        if !pk_columns.is_empty() {
            constraints.push(RawConstraintInfo::PrimaryKey {
                columns: pk_columns,
            });
        }

        // FOREIGN KEY
        // 制約名でグループ化して、同一テーブルへの複数FKを正しく区別する
        // REFERENTIAL_CONSTRAINTS テーブルから ON DELETE アクションも取得
        let fk_sql = r#"
            SELECT
                kcu.constraint_name,
                kcu.column_name,
                kcu.referenced_table_name,
                kcu.referenced_column_name,
                rc.delete_rule
            FROM information_schema.key_column_usage kcu
            JOIN information_schema.referential_constraints rc
                ON kcu.constraint_name = rc.constraint_name
                AND kcu.table_schema = rc.constraint_schema
            WHERE kcu.table_name = ? AND kcu.table_schema = DATABASE()
                AND kcu.referenced_table_name IS NOT NULL
            ORDER BY kcu.constraint_name, kcu.ordinal_position
        "#;

        let fk_rows = sqlx::query(fk_sql).bind(table_name).fetch_all(pool).await?;

        // 制約名でグループ化（複合外部キー対応）
        // (referenced_table, columns, referenced_columns, on_delete)
        let mut fk_map: std::collections::HashMap<
            String,
            (String, Vec<String>, Vec<String>, Option<String>),
        > = std::collections::HashMap::new();

        for row in &fk_rows {
            let constraint_name = mysql_get_string(row, 0);
            let column = mysql_get_string(row, 1);
            let ref_table = mysql_get_string(row, 2);
            let ref_column = mysql_get_string(row, 3);
            let delete_rule = mysql_get_optional_string(row, 4);

            let entry = fk_map.entry(constraint_name).or_insert_with(|| {
                let on_delete = delete_rule.and_then(|rule| {
                    if rule == "NO ACTION" || rule == "RESTRICT" {
                        None
                    } else {
                        Some(rule)
                    }
                });
                (ref_table.clone(), Vec::new(), Vec::new(), on_delete)
            });
            entry.1.push(column);
            entry.2.push(ref_column);
        }

        for (_constraint_name, (ref_table, columns, ref_columns, on_delete)) in fk_map {
            constraints.push(RawConstraintInfo::ForeignKey {
                columns,
                referenced_table: ref_table,
                referenced_columns: ref_columns,
                on_delete,
            });
        }

        // UNIQUE
        // インデックス名でグループ化して、複数のUNIQUE制約を正しく区別する
        let unique_sql = r#"
            SELECT index_name, column_name
            FROM information_schema.statistics
            WHERE table_name = ? AND table_schema = DATABASE()
                AND non_unique = 0
                AND index_name != 'PRIMARY'
            ORDER BY index_name, seq_in_index
        "#;

        let unique_rows = sqlx::query(unique_sql)
            .bind(table_name)
            .fetch_all(pool)
            .await?;

        // インデックス名でグループ化
        let mut unique_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for row in unique_rows {
            let index_name = mysql_get_string(&row, 0);
            let column = mysql_get_string(&row, 1);

            unique_map.entry(index_name).or_default().push(column);
        }

        for (_index_name, columns) in unique_map {
            constraints.push(RawConstraintInfo::Unique { columns });
        }

        Ok(constraints)
    }

    async fn get_enums(&self, _pool: &AnyPool) -> Result<Vec<RawEnumInfo>> {
        // MySQLではENUMはカラム定義に埋め込まれるため、
        // 独立したENUM定義は取得できない
        Ok(Vec::new())
    }
}

// =============================================================================
// SQLite イントロスペクター実装
// =============================================================================

#[async_trait]
impl DatabaseIntrospector for SqliteIntrospector {
    async fn get_table_names(&self, pool: &AnyPool) -> Result<Vec<String>> {
        use sqlx::Row;

        let sql = r#"
            SELECT name
            FROM sqlite_master
            WHERE type = 'table'
                AND name NOT LIKE 'sqlite_%'
                AND name != 'schema_migrations'
            ORDER BY name
        "#;

        let rows = sqlx::query(sql).fetch_all(pool).await?;
        let table_names = rows.iter().map(|row| row.get::<String, _>(0)).collect();

        Ok(table_names)
    }

    async fn get_columns(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawColumnInfo>> {
        use crate::adapters::sql_quote::quote_identifier_sqlite;
        use sqlx::Row;

        // CREATE TABLE SQL を取得して AUTOINCREMENT を検出
        let create_sql_query = "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?";
        let create_sql_row = sqlx::query(create_sql_query)
            .bind(table_name)
            .fetch_optional(pool)
            .await?;
        let has_autoincrement = create_sql_row
            .and_then(|row| row.try_get::<Option<String>, _>(0).ok().flatten())
            .map(|sql| sql.to_uppercase().contains("AUTOINCREMENT"))
            .unwrap_or(false);

        let quoted_name = quote_identifier_sqlite(table_name);
        let sql = format!("PRAGMA table_info({})", quoted_name);
        let rows = sqlx::query(&sql).fetch_all(pool).await?;

        let columns = rows
            .iter()
            .map(|row| {
                let not_null: i32 = row.get(3);
                let is_pk: i32 = row.get(5);
                let data_type: String = row.get(2);
                // SQLite の INTEGER PRIMARY KEY AUTOINCREMENT を検出
                let auto_increment = if has_autoincrement
                    && is_pk > 0
                    && data_type.to_uppercase() == "INTEGER"
                {
                    Some(true)
                } else {
                    None
                };
                RawColumnInfo {
                    name: row.get(1),
                    data_type,
                    is_nullable: not_null == 0,
                    default_value: row.get(4),
                    char_max_length: None,
                    numeric_precision: None,
                    numeric_scale: None,
                    udt_name: None,
                    auto_increment,
                }
            })
            .collect();

        Ok(columns)
    }

    async fn get_indexes(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawIndexInfo>> {
        use crate::adapters::sql_quote::quote_identifier_sqlite;
        use sqlx::Row;

        let quoted_table = quote_identifier_sqlite(table_name);
        let sql = format!("PRAGMA index_list({})", quoted_table);
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
            let quoted_index = quote_identifier_sqlite(&index_name);
            let info_sql = format!("PRAGMA index_info({})", quoted_index);
            let info_rows = sqlx::query(&info_sql).fetch_all(pool).await?;

            let columns: Vec<String> = info_rows.iter().map(|r| r.get::<String, _>(2)).collect();

            indexes.push(RawIndexInfo {
                name: index_name,
                columns,
                unique: is_unique == 1,
            });
        }

        Ok(indexes)
    }

    async fn get_constraints(
        &self,
        pool: &AnyPool,
        table_name: &str,
    ) -> Result<Vec<RawConstraintInfo>> {
        use crate::adapters::sql_quote::quote_identifier_sqlite;
        use sqlx::Row;

        let mut constraints = Vec::new();

        // PRIMARY KEY
        let quoted_table = quote_identifier_sqlite(table_name);
        let table_info_sql = format!("PRAGMA table_info({})", quoted_table);
        let rows = sqlx::query(&table_info_sql).fetch_all(pool).await?;

        let pk_columns: Vec<String> = rows
            .iter()
            .filter(|row| row.get::<i32, _>(5) > 0) // pk列が0より大きい
            .map(|row| row.get::<String, _>(1))
            .collect();

        if !pk_columns.is_empty() {
            constraints.push(RawConstraintInfo::PrimaryKey {
                columns: pk_columns,
            });
        }

        // FOREIGN KEY
        let fk_sql = format!("PRAGMA foreign_key_list({})", quoted_table);
        let fk_rows = sqlx::query(&fk_sql).fetch_all(pool).await?;

        // PRAGMA foreign_key_list columns: id, seq, table, from, to, on_update, on_delete, match
        let mut fk_map: std::collections::HashMap<
            i32,
            (String, Vec<String>, Vec<String>, Option<String>),
        > = std::collections::HashMap::new();

        for row in fk_rows {
            let id: i32 = row.get(0);
            let ref_table: String = row.get(2);
            let from_col: String = row.get(3);
            let to_col: String = row.get(4);
            let on_delete: String = row.get(6);

            let entry = fk_map.entry(id).or_insert_with(|| {
                let od = if on_delete == "NO ACTION" {
                    None
                } else {
                    Some(on_delete.clone())
                };
                (ref_table.clone(), Vec::new(), Vec::new(), od)
            });

            entry.1.push(from_col);
            entry.2.push(to_col);
        }

        for (_id, (ref_table, from_cols, to_cols, on_delete)) in fk_map {
            constraints.push(RawConstraintInfo::ForeignKey {
                columns: from_cols,
                referenced_table: ref_table,
                referenced_columns: to_cols,
                on_delete,
            });
        }

        Ok(constraints)
    }

    async fn get_enums(&self, _pool: &AnyPool) -> Result<Vec<RawEnumInfo>> {
        // SQLiteはENUM型をサポートしていない
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Dialect;

    // =========================================================================
    // create_introspector テスト
    // =========================================================================

    #[test]
    fn test_create_introspector_postgres() {
        let _introspector = create_introspector(Dialect::PostgreSQL);
        // 型の確認のみ（実際のDB接続は統合テストで行う）
    }

    #[test]
    fn test_create_introspector_mysql() {
        let _introspector = create_introspector(Dialect::MySQL);
    }

    #[test]
    fn test_create_introspector_sqlite() {
        let _introspector = create_introspector(Dialect::SQLite);
    }

    // =========================================================================
    // RawColumnInfo 構造体テスト
    // =========================================================================

    #[test]
    fn test_raw_column_info_debug() {
        let column = RawColumnInfo {
            name: "id".to_string(),
            data_type: "INTEGER".to_string(),
            is_nullable: false,
            default_value: None,
            char_max_length: None,
            numeric_precision: None,
            numeric_scale: None,
            udt_name: None,
            auto_increment: None,
        };
        assert!(format!("{:?}", column).contains("id"));
    }

    #[test]
    fn test_raw_column_info_clone() {
        let column = RawColumnInfo {
            name: "email".to_string(),
            data_type: "VARCHAR".to_string(),
            is_nullable: true,
            default_value: Some("''".to_string()),
            char_max_length: Some(255),
            numeric_precision: None,
            numeric_scale: None,
            udt_name: None,
            auto_increment: None,
        };
        let cloned = column.clone();
        assert_eq!(cloned.name, "email");
        assert_eq!(cloned.char_max_length, Some(255));
    }

    // =========================================================================
    // RawIndexInfo 構造体テスト
    // =========================================================================

    #[test]
    fn test_raw_index_info_debug() {
        let index = RawIndexInfo {
            name: "idx_email".to_string(),
            columns: vec!["email".to_string()],
            unique: true,
        };
        assert!(format!("{:?}", index).contains("idx_email"));
    }

    #[test]
    fn test_raw_index_info_clone() {
        let index = RawIndexInfo {
            name: "idx_composite".to_string(),
            columns: vec!["col1".to_string(), "col2".to_string()],
            unique: false,
        };
        let cloned = index.clone();
        assert_eq!(cloned.columns.len(), 2);
    }

    // =========================================================================
    // RawConstraintInfo 構造体テスト
    // =========================================================================

    #[test]
    fn test_raw_constraint_info_primary_key() {
        let pk = RawConstraintInfo::PrimaryKey {
            columns: vec!["id".to_string()],
        };
        assert!(format!("{:?}", pk).contains("PrimaryKey"));
    }

    #[test]
    fn test_raw_constraint_info_foreign_key() {
        let fk = RawConstraintInfo::ForeignKey {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
        };
        assert!(format!("{:?}", fk).contains("ForeignKey"));
    }

    #[test]
    fn test_raw_constraint_info_unique() {
        let unique = RawConstraintInfo::Unique {
            columns: vec!["email".to_string()],
        };
        assert!(format!("{:?}", unique).contains("Unique"));
    }

    #[test]
    fn test_raw_constraint_info_check() {
        let check = RawConstraintInfo::Check {
            columns: vec!["age".to_string()],
            expression: "age >= 0".to_string(),
        };
        assert!(format!("{:?}", check).contains("Check"));
    }

    // =========================================================================
    // RawEnumInfo 構造体テスト
    // =========================================================================

    #[test]
    fn test_raw_enum_info_debug() {
        let enum_info = RawEnumInfo {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        };
        assert!(format!("{:?}", enum_info).contains("status"));
    }

    #[test]
    fn test_raw_enum_info_clone() {
        let enum_info = RawEnumInfo {
            name: "role".to_string(),
            values: vec!["admin".to_string(), "user".to_string()],
        };
        let cloned = enum_info.clone();
        assert_eq!(cloned.values.len(), 2);
    }
}
