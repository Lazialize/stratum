// データベースイントロスペクター
//
// データベースからスキーマ情報を取得するための抽象化レイヤー。
// 各方言固有のINFORMATION_SCHEMA/PRAGMAクエリを実装します。

use std::sync::LazyLock;

use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use sqlx::AnyPool;
use sqlx::Row;

/// 識別子検出用の正規表現（コンパイル済みキャッシュ）
static IDENTIFIER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\b").unwrap());

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

/// MySQL の COLUMN_TYPE から ENUM 値を抽出する
/// 例: "enum('draft','published','archived')" -> ["draft", "published", "archived"]
/// 空文字列のENUM値もサポート: "enum('')" -> [""], "enum('a','','b')" -> ["a", "", "b"]
fn parse_mysql_enum_values(column_type: &str) -> Option<Vec<String>> {
    // enum('value1','value2',...) の形式をパース
    let trimmed = column_type.trim();
    if !trimmed.to_lowercase().starts_with("enum(") {
        return None;
    }

    // 括弧内の内容を取得
    let start = trimmed.find('(')?;
    let end = trimmed.rfind(')')?;
    if start >= end {
        return None;
    }

    let content = &trimmed[start + 1..end];
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut value_closed = false; // クォートが閉じられたかを追跡（空文字列対応）
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_quote => {
                in_quote = true;
                value_closed = false;
            }
            '\'' if in_quote => {
                // エスケープされたシングルクォート ('')をチェック
                if chars.peek() == Some(&'\'') {
                    current.push('\'');
                    chars.next();
                } else {
                    in_quote = false;
                    value_closed = true;
                }
            }
            ',' if !in_quote => {
                // クォートが閉じられた値のみ追加（空文字列も含む）
                if value_closed {
                    values.push(current);
                    current = String::new();
                    value_closed = false;
                }
            }
            _ if in_quote => {
                current.push(c);
            }
            _ => {
                // クォート外の空白はスキップ
            }
        }
    }

    // 最後の値を追加（空文字列も含む）
    if value_closed {
        values.push(current);
    }

    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// MySQL の CHECK 式からカラム名を推定する
///
/// MySQL の check_clause にはバッククォートで囲まれたカラム名が含まれる。
/// エスケープされたバッククォート（``）にも対応する。
/// 例: "(`balance` >= 0)" -> ["balance"]
fn extract_columns_from_check_expression(expression: &str) -> Vec<String> {
    let mut columns = Vec::new();
    let mut chars = expression.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '`' {
            let mut name = String::new();
            // バッククォート内の識別子をパース（`` エスケープ対応）
            while let Some(c) = chars.next() {
                if c == '`' {
                    // 連続バッククォートはエスケープ: リテラルの ` として追加
                    if chars.peek() == Some(&'`') {
                        chars.next();
                        name.push('`');
                        continue;
                    }
                    break;
                }
                name.push(c);
            }
            if !name.is_empty() && !columns.contains(&name) {
                columns.push(name);
            }
        }
    }

    columns
}

/// MySQL の COLUMN_TYPE から SET 値を抽出する
/// 例: "set('read','write','execute')" -> ["read", "write", "execute"]
fn parse_mysql_set_values(column_type: &str) -> Option<Vec<String>> {
    let trimmed = column_type.trim();
    if !trimmed.to_lowercase().starts_with("set(") {
        return None;
    }

    let start = trimmed.find('(')?;
    let end = trimmed.rfind(')')?;
    if start >= end {
        return None;
    }

    let content = &trimmed[start + 1..end];
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut value_closed = false;
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_quote => {
                in_quote = true;
                value_closed = false;
            }
            '\'' if in_quote => {
                if chars.peek() == Some(&'\'') {
                    current.push('\'');
                    chars.next();
                } else {
                    in_quote = false;
                    value_closed = true;
                }
            }
            ',' if !in_quote => {
                if value_closed {
                    values.push(current.clone());
                    current.clear();
                    value_closed = false;
                }
            }
            _ if in_quote => {
                current.push(c);
            }
            _ => {}
        }
    }

    if value_closed {
        values.push(current);
    }

    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// MySQL の COLUMN_TYPE から UNSIGNED 修飾子を検出する
/// 例: "tinyint(3) unsigned" -> true, "int(11)" -> false
fn is_mysql_unsigned(column_type: &str) -> bool {
    column_type.to_lowercase().contains("unsigned")
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
    /// ENUM値のリスト（MySQL用）
    pub enum_values: Option<Vec<String>>,
    /// SET値のリスト（MySQL用）
    pub set_values: Option<Vec<String>>,
    /// UNSIGNED修飾子（MySQL用）
    pub is_unsigned: bool,
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

/// 生のView情報（DB固有フォーマット）
#[derive(Debug, Clone)]
pub struct RawViewInfo {
    /// ビュー名
    pub name: String,
    /// ビュー定義（SELECT文）
    pub definition: String,
    /// マテリアライズドビューかどうか
    pub is_materialized: bool,
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

    /// View定義を取得
    async fn get_views(&self, pool: &AnyPool) -> Result<Vec<RawViewInfo>>;
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
                enum_values: None, // PostgreSQLはget_enums()で別途取得
                set_values: None,
                is_unsigned: false,
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

        // CHECK制約
        // pg_constraintからCHECK制約を取得（contype = 'c'）
        // LEFT JOIN で式のみの制約（conkey が空）にも対応
        // string_agg でカラム名をカンマ区切りで返す（Any ドライバは配列非対応）
        let check_sql = r#"
            SELECT
                con.conname::text,
                pg_get_constraintdef(con.oid)::text AS check_expression,
                COALESCE(string_agg(a.attname::text, ',' ORDER BY u.ord), '') AS columns
            FROM pg_constraint con
            JOIN pg_class c ON c.oid = con.conrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            LEFT JOIN LATERAL unnest(con.conkey) WITH ORDINALITY AS u(attnum, ord) ON true
            LEFT JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = u.attnum
            WHERE con.contype = 'c'
                AND c.relname = $1
                AND n.nspname = 'public'
            GROUP BY con.conname, con.oid
            ORDER BY con.conname
        "#;

        let check_rows = sqlx::query(check_sql)
            .bind(table_name)
            .fetch_all(pool)
            .await?;

        for row in check_rows {
            let _constraint_name: String = row.get(0);
            let raw_expression: String = row.get(1);
            let columns_str: String = row.get(2);
            let columns: Vec<String> = if columns_str.is_empty() {
                Vec::new()
            } else {
                columns_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect()
            };

            // pg_get_constraintdef returns "CHECK ((expression))" or
            // "CHECK ((expression)) NOT VALID" / "CHECK (...) NO INHERIT"
            // "CHECK (" 以降の括弧ペアをバランス取りで抽出し、末尾トークンに対応する
            let expression = extract_pg_check_expression(&raw_expression);

            // PostgreSQL wraps simple expressions in extra parentheses.
            // Only strip if the entire expression is wrapped in a single matching pair.
            let expression = strip_outer_parens(&expression);

            constraints.push(RawConstraintInfo::Check {
                columns,
                expression,
            });
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

    async fn get_views(&self, pool: &AnyPool) -> Result<Vec<RawViewInfo>> {
        use sqlx::Row;

        let mut views = Vec::new();

        // 通常のビューを取得
        let sql = r#"
            SELECT
                table_name::text,
                view_definition::text
            FROM information_schema.views
            WHERE table_schema = 'public'
            ORDER BY table_name
        "#;

        let rows = sqlx::query(sql).fetch_all(pool).await?;

        for row in rows {
            let name: String = row.get(0);
            let definition: String = row.get(1);
            views.push(RawViewInfo {
                name,
                definition,
                is_materialized: false,
            });
        }

        // マテリアライズドビューを検出（未サポート警告用）
        let matview_sql = r#"
            SELECT matviewname::text, definition::text
            FROM pg_matviews
            WHERE schemaname = 'public'
            ORDER BY matviewname
        "#;

        let matview_rows = sqlx::query(matview_sql).fetch_all(pool).await?;

        for row in matview_rows {
            let name: String = row.get(0);
            let definition: String = row.get(1);
            views.push(RawViewInfo {
                name,
                definition,
                is_materialized: true,
            });
        }

        Ok(views)
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
                numeric_scale,
                extra,
                column_type
            FROM information_schema.columns
            WHERE table_name = ? AND table_schema = DATABASE()
            ORDER BY ordinal_position
        "#;

        let rows = sqlx::query(sql).bind(table_name).fetch_all(pool).await?;

        let columns = rows
            .iter()
            .map(|row| {
                // EXTRA カラムから auto_increment を検出
                let extra = mysql_get_optional_string(row, 7);
                let auto_increment = extra
                    .as_ref()
                    .map(|e| e.to_lowercase().contains("auto_increment"))
                    .filter(|&b| b)
                    .map(|_| true);

                // column_type から追加情報を抽出
                let data_type = mysql_get_string(row, 1);
                let column_type = mysql_get_string(row, 8);
                let data_type_lower = data_type.to_lowercase();

                // data_type が enum の場合、column_type から値を抽出
                let enum_values = if data_type_lower == "enum" {
                    parse_mysql_enum_values(&column_type)
                } else {
                    None
                };

                // data_type が set の場合、column_type から値を抽出
                let set_values = if data_type_lower == "set" {
                    parse_mysql_set_values(&column_type)
                } else {
                    None
                };

                // column_type から UNSIGNED 修飾子を検出
                let is_unsigned = is_mysql_unsigned(&column_type);

                RawColumnInfo {
                    name: mysql_get_string(row, 0),
                    data_type,
                    is_nullable: mysql_get_string(row, 2) == "YES",
                    default_value: mysql_get_optional_string(row, 3),
                    char_max_length: row.get(4),
                    numeric_precision: row.get(5),
                    numeric_scale: row.get(6),
                    udt_name: None,
                    auto_increment,
                    enum_values,
                    set_values,
                    is_unsigned,
                }
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

        // CHECK制約 (MySQL 8.0.16+)
        // information_schema.check_constraints と table_constraints を結合して取得
        let check_sql = r#"
            SELECT
                cc.constraint_name,
                cc.check_clause
            FROM information_schema.check_constraints cc
            JOIN information_schema.table_constraints tc
                ON cc.constraint_name = tc.constraint_name
                AND cc.constraint_schema = tc.constraint_schema
            WHERE tc.table_name = ? AND tc.table_schema = DATABASE()
                AND tc.constraint_type = 'CHECK'
            ORDER BY cc.constraint_name
        "#;

        let check_rows = sqlx::query(check_sql)
            .bind(table_name)
            .fetch_all(pool)
            .await?;

        for row in &check_rows {
            let constraint_name = mysql_get_string(row, 0);
            let check_clause = mysql_get_string(row, 1);

            // MySQL の自動生成制約をフィルタリング:
            // 1. NOT NULL チェック（ENUM カラムに自動付与される）
            // 2. ENUM バリデーション（_chk_N の名前で IN (...) を含む）
            // NOT NULL チェックは `(`col` is not null)` のように括弧で囲まれることがあるため、
            // 外側の括弧を剥がした上で判定する
            let lower = check_clause.trim().to_lowercase();
            let mut normalized = lower.as_str();
            while normalized.starts_with('(') && normalized.ends_with(')') {
                let inner = &normalized[1..normalized.len() - 1];
                // strip_outer_parens と同様に、先頭の ( と末尾の ) が対応しているか確認
                let mut depth = 0i32;
                let mut matched = true;
                for (i, ch) in inner.char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth < 0 && i < inner.len() - 1 {
                                matched = false;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if matched && depth == 0 {
                    normalized = inner.trim();
                } else {
                    break;
                }
            }
            // 単純な `<col> is not null` パターンのみをフィルタ（複合式は除外しない）
            // 例: "`col` is not null" → フィルタ, "`a` > 0 and `b` is not null" → 保持
            let is_not_null_check = {
                let trimmed_norm = normalized.trim();
                trimmed_norm.ends_with("is not null")
                    && !trimmed_norm.contains(" and ")
                    && !trimmed_norm.contains(" or ")
            };
            let is_enum_validation = {
                // MySQL は _chk_1, _chk_2, ... の名前で ENUM バリデーションを自動生成する
                let has_chk_suffix = constraint_name
                    .rfind("_chk_")
                    .map(|pos| {
                        constraint_name[pos + 5..]
                            .chars()
                            .all(|c| c.is_ascii_digit())
                    })
                    .unwrap_or(false);
                has_chk_suffix && (normalized.contains("in (") || normalized.contains("in("))
            };
            if is_not_null_check || is_enum_validation {
                continue;
            }

            // バッククォートで囲まれたカラム名を抽出
            let columns = extract_columns_from_check_expression(&check_clause);

            // MySQL の check_clause は外側に括弧が付く (例: "(`balance` >= 0)")
            // 他方言と統一するため strip_outer_parens で正規化する
            let expression = strip_outer_parens(&check_clause);

            constraints.push(RawConstraintInfo::Check {
                columns,
                expression,
            });
        }

        Ok(constraints)
    }

    async fn get_enums(&self, _pool: &AnyPool) -> Result<Vec<RawEnumInfo>> {
        // MySQLではENUMはカラム定義に埋め込まれるため、
        // 独立したENUM定義は取得できない
        Ok(Vec::new())
    }

    async fn get_views(&self, pool: &AnyPool) -> Result<Vec<RawViewInfo>> {
        let sql = r#"
            SELECT
                table_name,
                view_definition
            FROM information_schema.views
            WHERE table_schema = DATABASE()
            ORDER BY table_name
        "#;

        let rows = sqlx::query(sql).fetch_all(pool).await?;

        let views = rows
            .iter()
            .map(|row| {
                let name = mysql_get_string(row, 0);
                let definition = mysql_get_string(row, 1);
                RawViewInfo {
                    name,
                    definition,
                    is_materialized: false,
                }
            })
            .collect();

        Ok(views)
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
                let auto_increment =
                    if has_autoincrement && is_pk > 0 && data_type.to_uppercase() == "INTEGER" {
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
                    enum_values: None, // SQLiteはENUM型をサポートしない
                    set_values: None,
                    is_unsigned: false,
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

        // CHECK制約
        // sqlite_masterからCREATE TABLE文を取得してCHECK制約をパースする
        let create_sql_query = "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?";
        let create_row = sqlx::query(create_sql_query)
            .bind(table_name)
            .fetch_optional(pool)
            .await?;

        if let Some(row) = create_row {
            let create_sql: Option<String> = row.get(0);
            if let Some(sql) = create_sql {
                let check_constraints = parse_sqlite_check_constraints(&sql);
                constraints.extend(check_constraints);
            }
        }

        Ok(constraints)
    }

    async fn get_enums(&self, _pool: &AnyPool) -> Result<Vec<RawEnumInfo>> {
        // SQLiteはENUM型をサポートしていない
        Ok(Vec::new())
    }

    async fn get_views(&self, pool: &AnyPool) -> Result<Vec<RawViewInfo>> {
        use sqlx::Row;

        let sql = r#"
            SELECT name, sql
            FROM sqlite_master
            WHERE type = 'view'
            ORDER BY name
        "#;

        let rows = sqlx::query(sql).fetch_all(pool).await?;

        let views = rows
            .iter()
            .filter_map(|row| {
                let name: String = row.get(0);
                let create_sql: Option<String> = row.get(1);
                // SQLite の sql カラムには CREATE VIEW ... AS ... が入る
                // AS 以降を抽出して definition とする
                create_sql.map(|sql| {
                    let definition = extract_view_definition_from_create_sql(&sql);
                    RawViewInfo {
                        name,
                        definition,
                        is_materialized: false,
                    }
                })
            })
            .collect();

        Ok(views)
    }
}

/// PostgreSQL の pg_get_constraintdef() 出力から CHECK 式を抽出する
///
/// "CHECK ((expression))" から expression 部分を取り出す。
/// "CHECK (...) NOT VALID" や "CHECK (...) NO INHERIT" のように
/// 末尾にトークンが付くケースにも対応する（括弧のバランスで式の範囲を特定）。
fn extract_pg_check_expression(raw: &str) -> String {
    let prefix = "CHECK (";
    let Some(start) = raw.find(prefix) else {
        return raw.to_string();
    };
    let after_prefix = start + prefix.len();

    // "CHECK (" の直後の '(' を含む位置から括弧のバランスを追跡
    // depth は既に 1（"CHECK (" の '(' を含む）
    let mut depth = 1i32;
    let mut end = None;
    for (i, ch) in raw[after_prefix..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(after_prefix + i);
                    break;
                }
            }
            _ => {}
        }
    }

    match end {
        Some(pos) => raw[after_prefix..pos].to_string(),
        None => raw.to_string(),
    }
}

/// 式全体が一対の括弧で囲まれている場合のみ外側の括弧を除去する
///
/// `(balance >= 0)` → `balance >= 0` (除去)
/// `(val >= 0) AND (val <= 100)` → そのまま (除去しない: 先頭の `(` と末尾の `)` が対応していない)
fn strip_outer_parens(expr: &str) -> String {
    let trimmed = expr.trim();
    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return trimmed.to_string();
    }

    // 先頭の '(' に対応する ')' が末尾であることを確認
    let mut depth = 0;
    for (i, ch) in trimmed.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    // 先頭の '(' の対応が末尾の ')' と一致するか
                    return if i == trimmed.len() - 1 {
                        trimmed[1..i].trim().to_string()
                    } else {
                        trimmed.to_string()
                    };
                }
            }
            _ => {}
        }
    }

    trimmed.to_string()
}

/// CREATE VIEW 文からビュー定義（AS以降）を抽出する
fn extract_view_definition_from_create_sql(create_sql: &str) -> String {
    // 大文字小文字を無視して \s+AS\s+ パターンを検索（改行・タブにも対応）
    let re = regex::Regex::new(r"(?i)\bAS\s").unwrap();
    if let Some(m) = re.find(create_sql) {
        create_sql[m.end()..].trim().to_string()
    } else {
        // フォールバック: そのまま返す
        create_sql.to_string()
    }
}

/// SQLite の CREATE TABLE 文からCHECK制約をパースする
///
/// テーブルレベルおよびカラム定義内の両方のCHECK制約を抽出する。
/// 文字列リテラル（'...'）およびダブルクォート識別子（"..."）内の CHECK は無視する。
/// 例（テーブルレベル）: `CREATE TABLE t (id INTEGER, balance REAL, CHECK (balance >= 0))`
/// 例（カラムレベル）  : `CREATE TABLE t (id INTEGER CHECK (id > 0), balance REAL)`
fn parse_sqlite_check_constraints(create_sql: &str) -> Vec<RawConstraintInfo> {
    let mut results = Vec::new();
    let chars: Vec<(usize, char)> = create_sql.char_indices().collect();
    let len = chars.len();

    let mut i = 0;
    while i < len {
        let (_, ch) = chars[i];

        // シングルクォート文字列リテラルをスキップ（'' エスケープ対応）
        if ch == '\'' {
            i += 1;
            while i < len {
                if chars[i].1 == '\'' {
                    i += 1;
                    // '' はエスケープ: 文字列継続
                    if i < len && chars[i].1 == '\'' {
                        i += 1;
                        continue;
                    }
                    break;
                }
                i += 1;
            }
            continue;
        }

        // ダブルクォート識別子をスキップ（"" エスケープ対応）
        if ch == '"' {
            i += 1;
            while i < len {
                if chars[i].1 == '"' {
                    i += 1;
                    if i < len && chars[i].1 == '"' {
                        i += 1;
                        continue;
                    }
                    break;
                }
                i += 1;
            }
            continue;
        }

        // CHECK キーワードを検出（大文字小文字無視、単語境界チェック）
        if ch.eq_ignore_ascii_case(&'C') && i + 4 < len {
            let is_check = chars[i + 1].1.eq_ignore_ascii_case(&'H')
                && chars[i + 2].1.eq_ignore_ascii_case(&'E')
                && chars[i + 3].1.eq_ignore_ascii_case(&'C')
                && chars[i + 4].1.eq_ignore_ascii_case(&'K');

            if is_check {
                // 単語境界の確認
                let prev_is_ident = i > 0 && {
                    let prev = chars[i - 1].1;
                    prev == '_' || prev.is_ascii_alphanumeric()
                };
                let next_is_ident = i + 5 < len && {
                    let next_ch = chars[i + 5].1;
                    next_ch == '_' || next_ch.is_ascii_alphanumeric()
                };

                if !prev_is_ident && !next_is_ident {
                    // CHECK の後の空白をスキップして '(' を探す
                    let mut k = i + 5;
                    while k < len && chars[k].1.is_whitespace() {
                        k += 1;
                    }

                    if k < len && chars[k].1 == '(' {
                        let paren_start = chars[k].0 + chars[k].1.len_utf8();

                        // 対応する閉じ括弧を見つける（ネスト・クォート対応）
                        let mut depth = 1;
                        let mut expr_end = None;
                        let mut m = k + 1;
                        let mut in_sq = false;
                        let mut in_dq = false;

                        while m < len {
                            let (m_byte, mch) = chars[m];

                            if mch == '\'' && !in_dq {
                                if in_sq {
                                    if m + 1 < len && chars[m + 1].1 == '\'' {
                                        m += 2;
                                        continue;
                                    }
                                    in_sq = false;
                                } else {
                                    in_sq = true;
                                }
                                m += 1;
                                continue;
                            } else if mch == '"' && !in_sq {
                                if in_dq {
                                    if m + 1 < len && chars[m + 1].1 == '"' {
                                        m += 2;
                                        continue;
                                    }
                                    in_dq = false;
                                } else {
                                    in_dq = true;
                                }
                                m += 1;
                                continue;
                            }

                            if in_sq || in_dq {
                                m += 1;
                                continue;
                            }

                            match mch {
                                '(' => depth += 1,
                                ')' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        expr_end = Some(m_byte);
                                        break;
                                    }
                                }
                                _ => {}
                            }

                            m += 1;
                        }

                        if let Some(end_byte) = expr_end {
                            let expression = create_sql[paren_start..end_byte].trim().to_string();
                            let columns = extract_columns_from_sqlite_check(&expression);

                            results.push(RawConstraintInfo::Check {
                                columns,
                                expression,
                            });
                        }

                        i = m + 1;
                        continue;
                    }
                }

                i += 5;
                continue;
            }
        }

        i += 1;
    }

    results
}

/// SQLite CHECK式からカラム名を推定する
///
/// 文字列リテラル（'...'）内の単語は無視し、
/// SQLキーワード・関数名・データ型を除外して識別子を抽出する。
fn extract_columns_from_sqlite_check(expression: &str) -> Vec<String> {
    // 文字列リテラルを除去してからパース
    let stripped = strip_string_literals(expression);

    let keywords = [
        // 論理演算子・比較・制御構文
        "AND",
        "OR",
        "NOT",
        "IN",
        "IS",
        "LIKE",
        "BETWEEN",
        "EXISTS",
        "CASE",
        "WHEN",
        "THEN",
        "ELSE",
        "END",
        // リテラル・真偽値
        "NULL",
        "TRUE",
        "FALSE",
        // 関数
        "LENGTH",
        "LOWER",
        "UPPER",
        "SUBSTR",
        "ABS",
        "ROUND",
        "COALESCE",
        "IFNULL",
        "NULLIF",
        "TRIM",
        "LTRIM",
        "RTRIM",
        "MIN",
        "MAX",
        "AVG",
        "COUNT",
        "SUM",
        "RANDOM",
        "CHAR",
        "HEX",
        // その他
        "AS",
        "CAST",
        "COLLATE",
        "GLOB",
        "MATCH",
        "REGEXP",
        "CHECK",
        "CONSTRAINT",
    ];

    // CAST(... AS <type>) パターンで <type> の位置を収集
    // 例: "CAST(x AS INTEGER)" → "INTEGER" のバイト開始位置を記録し、カラム名から除外する
    // これによりデータ型名をキーワードリストに含めずとも、CAST 式内の型名を安全に除外できる
    let upper_stripped = stripped.to_uppercase();
    let mut cast_type_positions = std::collections::HashSet::new();
    for m in upper_stripped.match_indices(" AS ") {
        let after_as = m.0 + m.1.len();
        // AS の直後の空白をスキップ
        let type_start = upper_stripped[after_as..]
            .find(|c: char| !c.is_whitespace())
            .map(|p| after_as + p)
            .unwrap_or(after_as);
        cast_type_positions.insert(type_start);
    }

    let mut columns = Vec::new();

    for cap in IDENTIFIER_REGEX.captures_iter(&stripped) {
        let word = &cap[1];
        let upper = word.to_uppercase();
        let start = cap.get(1).unwrap().start();
        if keywords.contains(&upper.as_str())
            || columns.contains(&word.to_string())
            || cast_type_positions.contains(&start)
        {
            continue;
        }
        columns.push(word.to_string());
    }

    columns
}

/// SQL文字列リテラル（シングルクォート）を除去する
///
/// `status IN ('pending', 'active')` → `status IN (, )`
fn strip_string_literals(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            // 文字列リテラルをスキップ
            loop {
                match chars.next() {
                    Some('\'') => {
                        // '' エスケープのチェック
                        if chars.peek() == Some(&'\'') {
                            chars.next();
                            continue;
                        }
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
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
            enum_values: None,
            set_values: None,
            is_unsigned: false,
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
            enum_values: None,
            set_values: None,
            is_unsigned: false,
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

    // =========================================================================
    // RawViewInfo 構造体テスト
    // =========================================================================

    #[test]
    fn test_raw_view_info_debug() {
        let view = RawViewInfo {
            name: "active_users".to_string(),
            definition: "SELECT * FROM users WHERE active = true".to_string(),
            is_materialized: false,
        };
        assert!(format!("{:?}", view).contains("active_users"));
    }

    #[test]
    fn test_raw_view_info_clone() {
        let view = RawViewInfo {
            name: "user_stats".to_string(),
            definition: "SELECT count(*) FROM users".to_string(),
            is_materialized: true,
        };
        let cloned = view.clone();
        assert_eq!(cloned.name, "user_stats");
        assert!(cloned.is_materialized);
    }

    // =========================================================================
    // extract_view_definition_from_create_sql テスト
    // =========================================================================

    #[test]
    fn test_extract_view_definition_simple() {
        let sql = "CREATE VIEW active_users AS SELECT * FROM users WHERE active = 1";
        let definition = super::extract_view_definition_from_create_sql(sql);
        assert_eq!(definition, "SELECT * FROM users WHERE active = 1");
    }

    #[test]
    fn test_extract_view_definition_case_insensitive() {
        let sql = "CREATE VIEW my_view as select id from users";
        let definition = super::extract_view_definition_from_create_sql(sql);
        assert_eq!(definition, "select id from users");
    }

    #[test]
    fn test_extract_view_definition_with_extra_whitespace() {
        let sql = "CREATE VIEW  my_view  AS  SELECT id FROM users";
        let definition = super::extract_view_definition_from_create_sql(sql);
        assert_eq!(definition, "SELECT id FROM users");
    }

    #[test]
    fn test_extract_view_definition_no_as_fallback() {
        let sql = "some weird sql without the keyword";
        let definition = super::extract_view_definition_from_create_sql(sql);
        assert_eq!(definition, sql);
    }

    #[test]
    fn test_extract_view_definition_newline_after_as() {
        let sql = "CREATE VIEW my_view AS\nSELECT id FROM users";
        let definition = super::extract_view_definition_from_create_sql(sql);
        assert_eq!(definition, "SELECT id FROM users");
    }

    #[test]
    fn test_extract_view_definition_tab_after_as() {
        let sql = "CREATE VIEW my_view AS\tSELECT id FROM users";
        let definition = super::extract_view_definition_from_create_sql(sql);
        assert_eq!(definition, "SELECT id FROM users");
    }

    // =========================================================================
    // parse_mysql_enum_values テスト
    // =========================================================================

    #[test]
    fn test_parse_mysql_enum_values_simple() {
        let result = super::parse_mysql_enum_values("enum('draft','published','archived')");
        assert_eq!(
            result,
            Some(vec![
                "draft".to_string(),
                "published".to_string(),
                "archived".to_string()
            ])
        );
    }

    #[test]
    fn test_parse_mysql_enum_values_with_spaces() {
        let result = super::parse_mysql_enum_values("enum('a', 'b', 'c')");
        assert_eq!(
            result,
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn test_parse_mysql_enum_values_case_insensitive() {
        let result = super::parse_mysql_enum_values("ENUM('yes','no')");
        assert_eq!(result, Some(vec!["yes".to_string(), "no".to_string()]));
    }

    #[test]
    fn test_parse_mysql_enum_values_escaped_quote() {
        // MySQL では内部のシングルクォートを '' でエスケープ
        let result = super::parse_mysql_enum_values("enum('it''s','ok')");
        assert_eq!(result, Some(vec!["it's".to_string(), "ok".to_string()]));
    }

    #[test]
    fn test_parse_mysql_enum_values_single_value() {
        let result = super::parse_mysql_enum_values("enum('only')");
        assert_eq!(result, Some(vec!["only".to_string()]));
    }

    #[test]
    fn test_parse_mysql_enum_values_not_enum() {
        let result = super::parse_mysql_enum_values("varchar(255)");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_mysql_enum_values_empty() {
        let result = super::parse_mysql_enum_values("enum()");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_mysql_enum_values_empty_string_single() {
        // 空文字列のみのENUM
        let result = super::parse_mysql_enum_values("enum('')");
        assert_eq!(result, Some(vec!["".to_string()]));
    }

    #[test]
    fn test_parse_mysql_enum_values_empty_string_mixed() {
        // 空文字列を含む複数値のENUM
        let result = super::parse_mysql_enum_values("enum('a','','b')");
        assert_eq!(
            result,
            Some(vec!["a".to_string(), "".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_parse_mysql_enum_values_empty_string_at_end() {
        // 末尾に空文字列
        let result = super::parse_mysql_enum_values("enum('a','b','')");
        assert_eq!(
            result,
            Some(vec!["a".to_string(), "b".to_string(), "".to_string()])
        );
    }

    // =========================================================================
    // parse_sqlite_check_constraints テスト
    // =========================================================================

    #[test]
    fn test_parse_sqlite_check_simple() {
        let sql = "CREATE TABLE t (id INTEGER, balance REAL, CHECK (balance >= 0))";
        let checks = super::parse_sqlite_check_constraints(sql);
        assert_eq!(checks.len(), 1);
        match &checks[0] {
            RawConstraintInfo::Check {
                columns,
                expression,
            } => {
                assert_eq!(expression, "balance >= 0");
                assert!(columns.contains(&"balance".to_string()));
            }
            _ => panic!("Expected Check constraint"),
        }
    }

    #[test]
    fn test_parse_sqlite_check_multiple() {
        let sql = "CREATE TABLE t (id INTEGER, age INTEGER, balance REAL, CHECK (age >= 0), CHECK (balance > 0))";
        let checks = super::parse_sqlite_check_constraints(sql);
        assert_eq!(checks.len(), 2);
    }

    #[test]
    fn test_parse_sqlite_check_nested_parens() {
        let sql = "CREATE TABLE t (id INTEGER, val INTEGER, CHECK ((val >= 0) AND (val <= 100)))";
        let checks = super::parse_sqlite_check_constraints(sql);
        assert_eq!(checks.len(), 1);
        match &checks[0] {
            RawConstraintInfo::Check { expression, .. } => {
                assert_eq!(expression, "(val >= 0) AND (val <= 100)");
            }
            _ => panic!("Expected Check constraint"),
        }
    }

    #[test]
    fn test_parse_sqlite_check_case_insensitive() {
        let sql = "CREATE TABLE t (id INTEGER, x INTEGER, check (x > 0))";
        let checks = super::parse_sqlite_check_constraints(sql);
        assert_eq!(checks.len(), 1);
    }

    #[test]
    fn test_parse_sqlite_check_no_checks() {
        let sql = "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)";
        let checks = super::parse_sqlite_check_constraints(sql);
        assert!(checks.is_empty());
    }

    // =========================================================================
    // parse_sqlite_check_constraints 追加テスト（クォート対応）
    // =========================================================================

    #[test]
    fn test_parse_sqlite_check_ignores_check_in_string_literal() {
        // 文字列リテラル内の 'CHECK' は無視する
        let sql = "CREATE TABLE t (val TEXT, CHECK (val != 'CHECK'))";
        let checks = super::parse_sqlite_check_constraints(sql);
        assert_eq!(checks.len(), 1);
        match &checks[0] {
            RawConstraintInfo::Check { expression, .. } => {
                assert_eq!(expression, "val != 'CHECK'");
            }
            _ => panic!("Expected Check constraint"),
        }
    }

    #[test]
    fn test_parse_sqlite_check_column_named_check_prefix() {
        // check_date のようなカラム名は CHECK として誤検出しない
        let sql = "CREATE TABLE t (check_date TEXT, CHECK (check_date IS NOT NULL))";
        let checks = super::parse_sqlite_check_constraints(sql);
        assert_eq!(checks.len(), 1);
    }

    // =========================================================================
    // strip_outer_parens テスト
    // =========================================================================

    #[test]
    fn test_strip_outer_parens_simple() {
        assert_eq!(super::strip_outer_parens("(balance >= 0)"), "balance >= 0");
    }

    #[test]
    fn test_strip_outer_parens_no_parens() {
        assert_eq!(super::strip_outer_parens("balance >= 0"), "balance >= 0");
    }

    #[test]
    fn test_strip_outer_parens_non_matching() {
        // 先頭の ( と末尾の ) が対応していないケース
        let expr = "(val >= 0) AND (val <= 100)";
        assert_eq!(super::strip_outer_parens(expr), expr);
    }

    #[test]
    fn test_strip_outer_parens_nested_matching() {
        // 全体が一対の括弧で囲まれたネスト式
        assert_eq!(
            super::strip_outer_parens("((a >= 0) AND (b <= 100))"),
            "(a >= 0) AND (b <= 100)"
        );
    }

    // =========================================================================
    // extract_columns_from_check_expression テスト (MySQL)
    // =========================================================================

    #[test]
    fn test_extract_columns_from_mysql_check_single() {
        let columns = super::extract_columns_from_check_expression("`balance` >= 0");
        assert_eq!(columns, vec!["balance".to_string()]);
    }

    #[test]
    fn test_extract_columns_from_mysql_check_multiple() {
        let columns = super::extract_columns_from_check_expression("`start_date` < `end_date`");
        assert_eq!(
            columns,
            vec!["start_date".to_string(), "end_date".to_string()]
        );
    }

    #[test]
    fn test_extract_columns_from_mysql_check_no_backticks() {
        let columns = super::extract_columns_from_check_expression("balance >= 0");
        assert!(columns.is_empty());
    }

    #[test]
    fn test_extract_columns_from_mysql_check_escaped_backtick() {
        // エスケープされたバッククォート（``）を含むカラム名
        let columns = super::extract_columns_from_check_expression("`my``col` >= 0");
        assert_eq!(columns, vec!["my`col".to_string()]);
    }

    // =========================================================================
    // extract_columns_from_sqlite_check テスト
    // =========================================================================

    #[test]
    fn test_extract_columns_from_sqlite_check_simple() {
        let columns = super::extract_columns_from_sqlite_check("balance >= 0");
        assert_eq!(columns, vec!["balance".to_string()]);
    }

    #[test]
    fn test_extract_columns_from_sqlite_check_with_and() {
        let columns = super::extract_columns_from_sqlite_check("age >= 0 AND age <= 150");
        assert_eq!(columns, vec!["age".to_string()]);
    }

    #[test]
    fn test_extract_columns_from_sqlite_check_multiple_columns() {
        let columns = super::extract_columns_from_sqlite_check("start_date < end_date");
        assert_eq!(
            columns,
            vec!["start_date".to_string(), "end_date".to_string()]
        );
    }

    #[test]
    fn test_extract_columns_from_sqlite_check_ignores_string_literals() {
        // 文字列リテラル内の単語はカラム名として抽出しない
        let columns = super::extract_columns_from_sqlite_check("status IN ('pending', 'active')");
        assert_eq!(columns, vec!["status".to_string()]);
    }

    #[test]
    fn test_extract_columns_from_sqlite_check_ignores_keywords() {
        // CASE/WHEN/THEN/ELSE/END はキーワードとして除外される
        let columns =
            super::extract_columns_from_sqlite_check("CASE WHEN val > 0 THEN 1 ELSE 0 END = 1");
        assert_eq!(columns, vec!["val".to_string()]);
    }

    #[test]
    fn test_extract_columns_from_sqlite_check_cast_as_type() {
        // CAST(x AS INTEGER) の INTEGER はカラム名として抽出しない
        let columns = super::extract_columns_from_sqlite_check("CAST(val AS INTEGER) > 0");
        assert_eq!(columns, vec!["val".to_string()]);
    }

    #[test]
    fn test_extract_columns_from_sqlite_check_date_column() {
        // date はデータ型名だがカラム名としても使われるため、除外しない
        let columns = super::extract_columns_from_sqlite_check("date >= '2020-01-01'");
        assert_eq!(columns, vec!["date".to_string()]);
    }

    // =========================================================================
    // MySQL 自動生成制約フィルタ テスト（ユニット的検証）
    // =========================================================================

    /// MySQL の NOT NULL / ENUM フィルタロジックを再現するヘルパー
    fn should_filter_mysql_check(constraint_name: &str, check_clause: &str) -> bool {
        let lower = check_clause.trim().to_lowercase();
        let mut normalized = lower.as_str();
        loop {
            if normalized.starts_with('(') && normalized.ends_with(')') {
                let inner = &normalized[1..normalized.len() - 1];
                let mut depth = 0i32;
                let mut matched = true;
                for (i, ch) in inner.char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth < 0 && i < inner.len() - 1 {
                                matched = false;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if matched && depth == 0 {
                    normalized = inner.trim();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        let is_not_null_check = {
            let trimmed_norm = normalized.trim();
            trimmed_norm.ends_with("is not null")
                && !trimmed_norm.contains(" and ")
                && !trimmed_norm.contains(" or ")
        };
        let is_enum_validation = {
            let has_chk_suffix = constraint_name
                .rfind("_chk_")
                .map(|pos| {
                    constraint_name[pos + 5..]
                        .chars()
                        .all(|c| c.is_ascii_digit())
                })
                .unwrap_or(false);
            has_chk_suffix && (normalized.contains("in (") || normalized.contains("in("))
        };
        is_not_null_check || is_enum_validation
    }

    #[test]
    fn test_mysql_filter_not_null_simple() {
        // 単純な NOT NULL は自動生成としてフィルタされる
        assert!(should_filter_mysql_check(
            "users_chk_1",
            "(`role` is not null)"
        ));
    }

    #[test]
    fn test_mysql_filter_not_null_without_parens() {
        assert!(should_filter_mysql_check(
            "users_chk_1",
            "`col` is not null"
        ));
    }

    #[test]
    fn test_mysql_filter_compound_not_null_preserved() {
        // 複合式は NOT NULL で終わっていてもフィルタしない
        assert!(!should_filter_mysql_check(
            "users_chk_1",
            "(`a` > 0 AND `b` IS NOT NULL)"
        ));
    }

    #[test]
    fn test_mysql_filter_enum_validation() {
        // ENUM バリデーション制約は _chk_N + IN (...) でフィルタ
        assert!(should_filter_mysql_check(
            "users_chk_2",
            "(`role` in ('admin','user','guest'))"
        ));
    }

    #[test]
    fn test_mysql_filter_enum_validation_chk_3() {
        // _chk_3 パターンもフィルタされる
        assert!(should_filter_mysql_check(
            "table_chk_3",
            "(`status` in('active','inactive'))"
        ));
    }

    #[test]
    fn test_mysql_filter_user_defined_preserved() {
        // ユーザー定義の CHECK 制約はフィルタしない
        assert!(!should_filter_mysql_check(
            "users_balance_check",
            "(`balance` >= 0)"
        ));
    }

    #[test]
    fn test_mysql_filter_user_defined_with_in_preserved() {
        // _chk_ パターンでなければ IN を含んでいてもフィルタしない
        assert!(!should_filter_mysql_check(
            "custom_check",
            "(`val` in (1, 2, 3))"
        ));
    }

    // =========================================================================
    // strip_string_literals テスト
    // =========================================================================

    #[test]
    fn test_strip_string_literals_simple() {
        assert_eq!(
            super::strip_string_literals("status IN ('pending', 'active')"),
            "status IN (, )"
        );
    }

    #[test]
    fn test_strip_string_literals_escaped_quote() {
        assert_eq!(super::strip_string_literals("val != 'it''s'"), "val != ");
    }

    #[test]
    fn test_strip_string_literals_no_strings() {
        assert_eq!(super::strip_string_literals("balance >= 0"), "balance >= 0");
    }

    // =========================================================================
    // extract_pg_check_expression テスト
    // =========================================================================

    #[test]
    fn test_extract_pg_check_expression_simple() {
        assert_eq!(
            super::extract_pg_check_expression("CHECK ((balance >= 0))"),
            "(balance >= 0)"
        );
    }

    #[test]
    fn test_extract_pg_check_expression_not_valid() {
        // NOT VALID 末尾トークンがあっても式部分だけ抽出
        assert_eq!(
            super::extract_pg_check_expression("CHECK ((balance >= 0)) NOT VALID"),
            "(balance >= 0)"
        );
    }

    #[test]
    fn test_extract_pg_check_expression_no_inherit() {
        assert_eq!(
            super::extract_pg_check_expression("CHECK ((val > 0)) NO INHERIT"),
            "(val > 0)"
        );
    }

    #[test]
    fn test_extract_pg_check_expression_complex() {
        assert_eq!(
            super::extract_pg_check_expression("CHECK (((val >= 0) AND (val <= 100))) NOT VALID"),
            "((val >= 0) AND (val <= 100))"
        );
    }

    #[test]
    fn test_extract_pg_check_expression_no_prefix() {
        // CHECK プレフィックスがない場合はそのまま返す
        let raw = "something else";
        assert_eq!(super::extract_pg_check_expression(raw), raw);
    }

    // =========================================================================
    // parse_mysql_set_values テスト
    // =========================================================================

    #[test]
    fn test_parse_mysql_set_values_simple() {
        let result = super::parse_mysql_set_values("set('read','write','execute','admin')");
        assert_eq!(
            result,
            Some(vec![
                "read".to_string(),
                "write".to_string(),
                "execute".to_string(),
                "admin".to_string(),
            ])
        );
    }

    #[test]
    fn test_parse_mysql_set_values_with_spaces() {
        let result = super::parse_mysql_set_values("set('a', 'b', 'c')");
        assert_eq!(
            result,
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn test_parse_mysql_set_values_case_insensitive() {
        let result = super::parse_mysql_set_values("SET('yes','no')");
        assert_eq!(result, Some(vec!["yes".to_string(), "no".to_string()]));
    }

    #[test]
    fn test_parse_mysql_set_values_not_set() {
        let result = super::parse_mysql_set_values("varchar(255)");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_mysql_set_values_empty() {
        let result = super::parse_mysql_set_values("set()");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_mysql_set_values_single() {
        let result = super::parse_mysql_set_values("set('only')");
        assert_eq!(result, Some(vec!["only".to_string()]));
    }

    #[test]
    fn test_parse_mysql_set_values_escaped_quote() {
        let result = super::parse_mysql_set_values("set('it''s','ok')");
        assert_eq!(result, Some(vec!["it's".to_string(), "ok".to_string()]));
    }

    // =========================================================================
    // is_mysql_unsigned テスト
    // =========================================================================

    #[test]
    fn test_is_mysql_unsigned_true() {
        assert!(super::is_mysql_unsigned("tinyint(3) unsigned"));
        assert!(super::is_mysql_unsigned("mediumint(7) unsigned"));
        assert!(super::is_mysql_unsigned("int(10) unsigned"));
    }

    #[test]
    fn test_is_mysql_unsigned_false() {
        assert!(!super::is_mysql_unsigned("tinyint(3)"));
        assert!(!super::is_mysql_unsigned("int(11)"));
        assert!(!super::is_mysql_unsigned("varchar(255)"));
    }
}
