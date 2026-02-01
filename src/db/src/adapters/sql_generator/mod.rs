// SQL生成アダプター
//
// スキーマ定義から各データベース方言用のDDL文を生成するアダプター層。

pub mod mysql;
pub mod postgres;
pub mod sqlite;
pub mod sqlite_table_recreator;

use crate::core::error::ValidationError;
use crate::core::schema::{Column, ColumnType, Constraint, EnumDefinition, Index, Table};
use crate::core::schema_diff::{ColumnDiff, EnumDiff, RenamedColumn};
use sha2::{Digest, Sha256};

// sql_quoteモジュールから識別子クォート関数を再エクスポート
pub(crate) use crate::adapters::sql_quote::{
    quote_columns_mysql, quote_columns_postgres, quote_columns_sqlite, quote_identifier_mysql,
    quote_identifier_postgres, quote_identifier_sqlite, quote_regclass_postgres,
};

/// PostgreSQL/MySQLの識別子最大長
const MAX_IDENTIFIER_LENGTH: usize = 63;

/// 制約名を生成する共通ヘルパー
///
/// `{prefix}_{body}`形式で名前を組み立て、63文字（`MAX_IDENTIFIER_LENGTH`）を超える場合は
/// SHA-256ハッシュ付きで切り詰めます。
///
/// # Arguments
///
/// * `prefix` - 制約名のプレフィックス（例: "fk", "uq", "ck"）
/// * `body` - プレフィックス以降の本体部分（テーブル名、カラム名等を結合済み）
///
/// # Returns
///
/// 63文字以内の制約名
fn generate_constraint_name(prefix: &str, body: &str) -> String {
    let base_name = format!("{}_{}", prefix, body);

    if base_name.len() <= MAX_IDENTIFIER_LENGTH {
        return base_name;
    }

    // 長すぎる場合はハッシュを付けて切り詰める
    // ハッシュは元の完全な名前から生成するため、同じ入力には同じ出力を保証
    let mut hasher = Sha256::new();
    hasher.update(base_name.as_bytes());
    let hash = hasher.finalize();
    let hash_suffix = format!(
        "_{:x}",
        &hash[..4].iter().fold(0u32, |acc, &b| acc << 8 | b as u32)
    );

    let prefix_with_sep = format!("{}_", prefix);
    let available_length = MAX_IDENTIFIER_LENGTH - prefix_with_sep.len() - hash_suffix.len();

    if body.len() <= available_length {
        format!("{}{}{}", prefix_with_sep, body, hash_suffix)
    } else {
        format!(
            "{}{}{}",
            prefix_with_sep,
            &body[..available_length],
            hash_suffix
        )
    }
}

/// 外部キー制約名を生成
///
/// `fk_{table_name}_{columns}_{referenced_table}`形式で名前を生成します。
/// 63文字を超える場合は、末尾にハッシュを付けて切り詰めます。
pub(crate) fn generate_fk_constraint_name(
    table_name: &str,
    columns: &[String],
    referenced_table: &str,
) -> String {
    let body = format!("{}_{}", table_name, columns.join("_"));
    let body_with_ref = format!("{}_{}", body, referenced_table);
    generate_constraint_name("fk", &body_with_ref)
}

/// UNIQUE制約名を生成
///
/// `uq_{table_name}_{columns}`形式で名前を生成します。
/// 63文字を超える場合は、末尾にハッシュを付けて切り詰めます。
pub(crate) fn generate_uq_constraint_name(table_name: &str, columns: &[String]) -> String {
    let body = format!("{}_{}", table_name, columns.join("_"));
    generate_constraint_name("uq", &body)
}

/// CHECK制約名を生成
///
/// `ck_{table_name}_{columns}`形式で名前を生成します。
/// 63文字を超える場合は、末尾にハッシュを付けて切り詰めます。
pub(crate) fn generate_ck_constraint_name(table_name: &str, columns: &[String]) -> String {
    let body = format!("{}_{}", table_name, columns.join("_"));
    generate_constraint_name("ck", &body)
}

/// CHECK式のバリデーション
///
/// defense-in-depth として、CHECK式に危険なDML/DDLキーワードが含まれていないか検証します。
/// スキーマ定義ファイル由来の値のみが渡される想定ですが、万一の不正入力を防ぎます。
///
/// # Returns
///
/// バリデーションに失敗した場合はエラーメッセージを返します。
pub(crate) fn validate_check_expression(expr: &str) -> Result<(), ValidationError> {
    // 大文字に正規化してキーワードを検査
    let upper = expr.to_uppercase();
    // トークン境界を考慮するため、単語単位で検出
    let tokens: Vec<&str> = upper.split_whitespace().collect();

    const FORBIDDEN_KEYWORDS: &[&str] = &[
        "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "TRUNCATE", "GRANT", "REVOKE",
        "EXEC", "EXECUTE", "CALL",
    ];

    for token in &tokens {
        // セミコロンを含むトークンも検出（例: "DELETE;"）
        let clean_token = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        for keyword in FORBIDDEN_KEYWORDS {
            if clean_token == *keyword {
                return Err(ValidationError::Constraint {
                    message: format!(
                        "CHECK式に禁止キーワード '{}' が含まれています: {}",
                        keyword, expr
                    ),
                    location: None,
                    suggestion: Some("CHECK式からDML/DDLキーワードを除去してください".to_string()),
                });
            }
        }
    }

    // セミコロンの検出（ステートメント区切りによるインジェクション防止）
    if expr.contains(';') {
        return Err(ValidationError::Constraint {
            message: format!("CHECK式にセミコロンが含まれています: {}", expr),
            location: None,
            suggestion: Some("CHECK式からセミコロンを除去してください".to_string()),
        });
    }

    Ok(())
}

/// CHECK式をバリデーション付きでSQL化
///
/// バリデーションに失敗した場合はコメント付きのSQLを生成し、
/// 不正なCHECK式がそのまま実行されることを防ぎます。
pub(crate) fn format_check_constraint(expr: &str) -> String {
    if let Err(err) = validate_check_expression(expr) {
        let sanitized_msg = sanitize_sql_comment(&err.to_string());
        format!("/* ERROR: {} */ CHECK (FALSE)", sanitized_msg)
    } else {
        format!("CHECK ({})", expr)
    }
}

/// SQLコメント内に埋め込む文字列をサニタイズ
///
/// `*/` を `* /` に置換して、SQLコメント `/* ... */` が壊れるのを防ぎます。
pub(crate) fn sanitize_sql_comment(s: &str) -> String {
    s.replace("*/", "* /")
}

/// カラム定義の共通組み立てヘルパー
///
/// # Arguments
///
/// * `quoted_name` - クォート済みのカラム名
/// * `column` - カラム定義（nullable, default_valueなどを参照）
/// * `type_str` - SQL型文字列
/// * `extra_parts` - 追加の修飾子（AUTO_INCREMENTなど）
pub(crate) fn build_column_definition(
    quoted_name: &str,
    column: &Column,
    type_str: String,
    extra_parts: &[&str],
) -> String {
    let mut parts = Vec::new();

    parts.push(quoted_name.to_string());
    parts.push(type_str);

    if !column.nullable {
        parts.push("NOT NULL".to_string());
    }

    for part in extra_parts {
        if !part.is_empty() {
            parts.push((*part).to_string());
        }
    }

    if let Some(ref default_value) = column.default_value {
        parts.push(format!("DEFAULT {}", default_value));
    }

    parts.join(" ")
}

/// マイグレーション方向
///
/// マイグレーションの適用方向を表現します。
/// up/down SQLの生成時に使用されます。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationDirection {
    /// 順方向のマイグレーション（up.sql）
    Up,
    /// 逆方向のマイグレーション（down.sql）
    Down,
}

impl MigrationDirection {
    /// 対象の型を取得
    ///
    /// Up方向では新しい型、Down方向では古い型を返します。
    pub fn target_type<'a>(
        &self,
        old_type: &'a ColumnType,
        new_type: &'a ColumnType,
    ) -> &'a ColumnType {
        match self {
            MigrationDirection::Up => new_type,
            MigrationDirection::Down => old_type,
        }
    }

    /// ソースの型を取得
    ///
    /// Up方向では古い型、Down方向では新しい型を返します。
    pub fn source_type<'a>(
        &self,
        old_type: &'a ColumnType,
        new_type: &'a ColumnType,
    ) -> &'a ColumnType {
        match self {
            MigrationDirection::Up => old_type,
            MigrationDirection::Down => new_type,
        }
    }
}

/// SQLジェネレータートレイト
///
/// 各データベース方言用のSQLジェネレーターが実装すべきインターフェース。
/// プリミティブメソッド（quote_identifier, quote_columns, generate_column_definition,
/// generate_constraint_definition）を各実装が提供し、共通アルゴリズムはデフォルト実装で提供。
pub trait SqlGenerator {
    // ===========================================
    // プリミティブメソッド（各実装が提供）
    // ===========================================

    /// ダイアレクト固有の識別子クォート
    fn quote_identifier(&self, name: &str) -> String;

    /// ダイアレクト固有のカラムリストクォート
    fn quote_columns(&self, columns: &[String]) -> String;

    /// ダイアレクト固有のカラム定義生成
    fn generate_column_definition(&self, column: &Column) -> String;

    /// ダイアレクト固有の制約定義生成
    fn generate_constraint_definition(&self, constraint: &Constraint) -> String;

    // ===========================================
    // デフォルト実装付きメソッド
    // ===========================================

    /// テーブル制約としてCREATE TABLE内に含めるかの判定
    ///
    /// デフォルト: FOREIGN_KEY以外はtrue。
    /// SQLiteはオーバーライドして全制約をCREATE TABLE内に定義。
    fn should_add_as_table_constraint(&self, constraint: &Constraint) -> bool {
        !matches!(constraint, Constraint::FOREIGN_KEY { .. })
    }

    /// CREATE TABLE文を生成
    fn generate_create_table(&self, table: &Table) -> String {
        let mut parts = Vec::new();

        parts.push(format!(
            "CREATE TABLE {}",
            self.quote_identifier(&table.name)
        ));
        parts.push("(".to_string());

        let mut elements = Vec::new();

        // カラム定義
        for column in &table.columns {
            elements.push(format!("    {}", self.generate_column_definition(column)));
        }

        // テーブル制約
        for constraint in &table.constraints {
            if self.should_add_as_table_constraint(constraint) {
                let constraint_def = self.generate_constraint_definition(constraint);
                if !constraint_def.is_empty() {
                    elements.push(format!("    {}", constraint_def));
                }
            }
        }

        parts.push(elements.join(",\n"));
        parts.push(")".to_string());

        parts.join("\n")
    }

    /// CREATE INDEX文を生成
    fn generate_create_index(&self, table: &Table, index: &Index) -> String {
        let index_type = if index.unique {
            "UNIQUE INDEX"
        } else {
            "INDEX"
        };

        format!(
            "CREATE {} {} ON {} ({})",
            index_type,
            self.quote_identifier(&index.name),
            self.quote_identifier(&table.name),
            self.quote_columns(&index.columns)
        )
    }

    /// ALTER TABLE ADD COLUMN文を生成
    fn generate_add_column(&self, table_name: &str, column: &Column) -> String {
        format!(
            "ALTER TABLE {} ADD COLUMN {}",
            self.quote_identifier(table_name),
            self.generate_column_definition(column)
        )
    }

    /// ALTER TABLE DROP COLUMN文を生成
    fn generate_drop_column(&self, table_name: &str, column_name: &str) -> String {
        format!(
            "ALTER TABLE {} DROP COLUMN {}",
            self.quote_identifier(table_name),
            self.quote_identifier(column_name)
        )
    }

    /// DROP TABLE文を生成
    fn generate_drop_table(&self, table_name: &str) -> String {
        format!("DROP TABLE {}", self.quote_identifier(table_name))
    }

    /// DROP INDEX文を生成
    ///
    /// MySQLは `ON table_name` が必要なためオーバーライド。
    fn generate_drop_index(&self, _table_name: &str, index_name: &str) -> String {
        format!("DROP INDEX {}", self.quote_identifier(index_name))
    }

    /// テーブルリネームSQL文を生成
    ///
    /// # Arguments
    ///
    /// * `old_name` - 旧テーブル名
    /// * `new_name` - 新テーブル名
    ///
    /// # Returns
    ///
    /// ALTER TABLE RENAME文のSQL文字列
    fn generate_rename_table(&self, old_name: &str, new_name: &str) -> String {
        format!(
            "ALTER TABLE {} RENAME TO {}",
            self.quote_identifier(old_name),
            self.quote_identifier(new_name)
        )
    }

    /// DOWN時に復元が必要なテーブルの注意コメントを生成
    fn generate_missing_table_notice(&self, table_name: &str) -> String {
        format!(
            "-- NOTE: Manually add CREATE TABLE statement for '{}' if rollback is needed",
            table_name
        )
    }

    /// ALTER TABLE文（制約追加）を生成
    ///
    /// FOREIGN KEY制約のみALTER TABLEで追加。それ以外は空文字列。
    fn generate_alter_table_add_constraint(
        &self,
        table: &Table,
        constraint_index: usize,
    ) -> String {
        if let Some(constraint) = table.constraints.get(constraint_index) {
            match constraint {
                Constraint::FOREIGN_KEY {
                    columns,
                    referenced_table,
                    referenced_columns,
                    on_delete,
                    on_update,
                } => {
                    let constraint_name =
                        generate_fk_constraint_name(&table.name, columns, referenced_table);

                    let mut sql = format!(
                        "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
                        self.quote_identifier(&table.name),
                        self.quote_identifier(&constraint_name),
                        self.quote_columns(columns),
                        self.quote_identifier(referenced_table),
                        self.quote_columns(referenced_columns)
                    );

                    if let Some(action) = on_delete {
                        sql.push_str(&format!(" ON DELETE {}", action.as_sql()));
                    }
                    if let Some(action) = on_update {
                        sql.push_str(&format!(" ON UPDATE {}", action.as_sql()));
                    }

                    sql
                }
                _ => {
                    // FOREIGN KEY以外の制約はCREATE TABLEで定義されるため、ここでは空文字列
                    String::new()
                }
            }
        } else {
            String::new()
        }
    }

    /// 既存テーブルへの制約追加SQL文を生成
    ///
    /// # Arguments
    ///
    /// * `table_name` - テーブル名
    /// * `constraint` - 追加する制約
    ///
    /// # Returns
    ///
    /// ALTER TABLE ADD CONSTRAINT文のSQL文字列
    fn generate_add_constraint_for_existing_table(
        &self,
        table_name: &str,
        constraint: &crate::core::schema::Constraint,
    ) -> String {
        // デフォルト実装：空文字列
        let _ = (table_name, constraint);
        String::new()
    }

    /// 既存テーブルからの制約削除SQL文を生成
    ///
    /// # Arguments
    ///
    /// * `table_name` - テーブル名
    /// * `constraint` - 削除する制約
    ///
    /// # Returns
    ///
    /// ALTER TABLE DROP CONSTRAINT文のSQL文字列
    fn generate_drop_constraint_for_existing_table(
        &self,
        table_name: &str,
        constraint: &crate::core::schema::Constraint,
    ) -> String {
        // デフォルト実装：空文字列
        let _ = (table_name, constraint);
        String::new()
    }

    /// カラム型変更のALTER TABLE文を生成
    ///
    /// # Arguments
    ///
    /// * `table` - 対象テーブルの完全な定義（direction=Upなら新定義、Downなら旧定義）
    /// * `column_diff` - カラム差分情報
    /// * `direction` - マイグレーション方向（Up/Down）
    ///
    /// # Returns
    ///
    /// ALTER TABLE文のベクター（SQLiteは複数文）
    fn generate_alter_column_type(
        &self,
        _table: &Table,
        _column_diff: &ColumnDiff,
        _direction: MigrationDirection,
    ) -> Vec<String> {
        // デフォルト実装：空のベクター
        // 各方言の実装でオーバーライド
        Vec::new()
    }

    /// カラム型変更のALTER TABLE文を生成（旧テーブル情報付き）
    ///
    /// SQLiteなど、カラム追加/削除を伴うテーブル再作成が必要な場合に使用します。
    /// 旧テーブルのカラム情報を基に、列交差ロジックでデータコピーSQLを生成します。
    ///
    /// # Arguments
    ///
    /// * `table` - 対象テーブルの新しい定義（direction=Upなら新定義、Downなら旧定義）
    /// * `old_table` - 対象テーブルの古い定義（列交差のための参照）
    /// * `column_diff` - カラム差分情報
    /// * `direction` - マイグレーション方向（Up/Down）
    ///
    /// # Returns
    ///
    /// ALTER TABLE文のベクター（SQLiteは複数文）
    fn generate_alter_column_type_with_old_table(
        &self,
        table: &Table,
        _old_table: Option<&Table>,
        column_diff: &ColumnDiff,
        direction: MigrationDirection,
    ) -> Vec<String> {
        // デフォルト実装：old_tableを無視して通常のgenerate_alter_column_typeを呼び出す
        self.generate_alter_column_type(table, column_diff, direction)
    }

    /// カラムリネームのALTER TABLE文を生成
    ///
    /// # Arguments
    ///
    /// * `table` - 対象テーブル
    /// * `renamed_column` - リネームされたカラム情報
    /// * `direction` - マイグレーション方向（Up/Down）
    ///
    /// # Returns
    ///
    /// ALTER TABLE RENAME COLUMN文のベクター
    fn generate_rename_column(
        &self,
        _table: &Table,
        _renamed_column: &RenamedColumn,
        _direction: MigrationDirection,
    ) -> Vec<String> {
        // デフォルト実装：空のベクター
        // 各方言の実装でオーバーライド
        Vec::new()
    }

    /// カラムのNULL制約変更SQL生成
    ///
    /// # Arguments
    ///
    /// * `table_name` - テーブル名
    /// * `column` - 変更対象のカラム（MySQL用の完全な定義を含む）
    /// * `new_nullable` - 新しいnullable値
    fn generate_alter_column_nullable(
        &self,
        _table_name: &str,
        _column: &Column,
        _new_nullable: bool,
    ) -> Vec<String> {
        Vec::new()
    }

    /// カラムのデフォルト値変更SQL生成
    ///
    /// # Arguments
    ///
    /// * `table_name` - テーブル名
    /// * `column` - 変更対象のカラム（MySQL用の完全な定義を含む）
    /// * `new_default` - 新しいデフォルト値（Noneの場合はDROP DEFAULT）
    fn generate_alter_column_default(
        &self,
        _table_name: &str,
        _column: &Column,
        _new_default: Option<&str>,
    ) -> Vec<String> {
        Vec::new()
    }

    /// ENUM型の作成（PostgreSQL専用）
    fn generate_create_enum_type(&self, _enum_def: &EnumDefinition) -> Vec<String> {
        Vec::new()
    }

    /// ENUM値追加（PostgreSQL専用）
    fn generate_add_enum_value(&self, _enum_name: &str, _value: &str) -> Vec<String> {
        Vec::new()
    }

    /// ENUM再作成（PostgreSQL専用）
    fn generate_recreate_enum_type(&self, _enum_diff: &EnumDiff) -> Vec<String> {
        Vec::new()
    }

    /// ENUM削除（PostgreSQL専用）
    fn generate_drop_enum_type(&self, _enum_name: &str) -> Vec<String> {
        Vec::new()
    }

    // ===========================================
    // ビュー関連メソッド
    // ===========================================

    /// CREATE VIEW文を生成
    ///
    /// PostgreSQL/MySQL: CREATE OR REPLACE VIEW
    /// SQLite: CREATE VIEW (CREATE OR REPLACE 非対応)
    fn generate_create_view(&self, view_name: &str, definition: &str) -> String {
        format!(
            "CREATE OR REPLACE VIEW {} AS\n{}",
            self.quote_identifier(view_name),
            definition
        )
    }

    /// DROP VIEW文を生成
    fn generate_drop_view(&self, view_name: &str) -> String {
        format!("DROP VIEW IF EXISTS {}", self.quote_identifier(view_name))
    }

    /// ビューリネームSQL文を生成
    fn generate_rename_view(&self, old_name: &str, new_name: &str) -> String {
        format!(
            "ALTER VIEW {} RENAME TO {}",
            self.quote_identifier(old_name),
            self.quote_identifier(new_name)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{Column, Constraint};
    use crate::core::schema_diff::RenamedColumn;

    // ダミー実装（デフォルト実装のテスト用）
    struct DummySqlGenerator;

    impl SqlGenerator for DummySqlGenerator {
        fn quote_identifier(&self, name: &str) -> String {
            format!("\"{}\"", name)
        }

        fn quote_columns(&self, columns: &[String]) -> String {
            columns
                .iter()
                .map(|c| self.quote_identifier(c))
                .collect::<Vec<_>>()
                .join(", ")
        }

        fn generate_column_definition(&self, _column: &Column) -> String {
            String::new()
        }

        fn generate_constraint_definition(&self, _constraint: &Constraint) -> String {
            String::new()
        }
    }

    #[test]
    fn test_generate_rename_column_default_returns_empty() {
        // デフォルト実装は空のベクターを返す
        let generator = DummySqlGenerator;
        let table = Table::new("users".to_string());
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let new_column = Column::new(
            "user_name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let renamed_column = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let result =
            generator.generate_rename_column(&table, &renamed_column, MigrationDirection::Up);
        assert!(result.is_empty());

        let result =
            generator.generate_rename_column(&table, &renamed_column, MigrationDirection::Down);
        assert!(result.is_empty());
    }

    #[test]
    fn test_migration_direction_target_type() {
        let old_type = ColumnType::INTEGER { precision: None };
        let new_type = ColumnType::VARCHAR { length: 255 };

        // Up方向では新しい型が対象
        assert_eq!(
            MigrationDirection::Up.target_type(&old_type, &new_type),
            &new_type
        );

        // Down方向では古い型が対象
        assert_eq!(
            MigrationDirection::Down.target_type(&old_type, &new_type),
            &old_type
        );
    }

    #[test]
    fn test_migration_direction_source_type() {
        let old_type = ColumnType::INTEGER { precision: None };
        let new_type = ColumnType::VARCHAR { length: 255 };

        // Up方向では古い型がソース
        assert_eq!(
            MigrationDirection::Up.source_type(&old_type, &new_type),
            &old_type
        );

        // Down方向では新しい型がソース
        assert_eq!(
            MigrationDirection::Down.source_type(&old_type, &new_type),
            &new_type
        );
    }

    // ==========================================
    // generate_fk_constraint_name のテスト
    // ==========================================

    #[test]
    fn test_generate_fk_constraint_name_short() {
        // 63文字以下の場合はそのまま返す
        let name = generate_fk_constraint_name("posts", &["user_id".to_string()], "users");
        assert_eq!(name, "fk_posts_user_id_users");
        assert!(name.len() <= 63);
    }

    #[test]
    fn test_generate_fk_constraint_name_composite() {
        // 複合キーの場合
        let name = generate_fk_constraint_name(
            "order_items",
            &["order_id".to_string(), "product_id".to_string()],
            "orders",
        );
        assert_eq!(name, "fk_order_items_order_id_product_id_orders");
        assert!(name.len() <= 63);
    }

    #[test]
    fn test_generate_fk_constraint_name_truncated() {
        // 63文字を超える場合はハッシュ付きで切り詰め
        let name = generate_fk_constraint_name(
            "very_long_table_name_with_many_characters",
            &["organization_id".to_string(), "department_id".to_string()],
            "another_very_long_table_name_here",
        );

        // 63文字以下であることを確認
        assert!(
            name.len() <= 63,
            "Constraint name '{}' exceeds 63 characters (len={})",
            name,
            name.len()
        );

        // fk_プレフィックスで始まることを確認
        assert!(name.starts_with("fk_"));

        // ハッシュサフィックスが付いていることを確認（_xxxxxxxx形式）
        assert!(name.contains("_"), "Expected hash suffix in '{}'", name);
    }

    #[test]
    fn test_generate_fk_constraint_name_deterministic() {
        // 同じ入力には同じ出力を保証
        let name1 = generate_fk_constraint_name(
            "very_long_table_name_with_many_characters",
            &["organization_id".to_string()],
            "another_very_long_table_name_here",
        );
        let name2 = generate_fk_constraint_name(
            "very_long_table_name_with_many_characters",
            &["organization_id".to_string()],
            "another_very_long_table_name_here",
        );
        assert_eq!(name1, name2);
    }

    // ==========================================
    // generate_uq_constraint_name のテスト
    // ==========================================

    #[test]
    fn test_generate_uq_constraint_name_short() {
        // 63文字以下の場合はそのまま返す
        let name = generate_uq_constraint_name("users", &["email".to_string()]);
        assert_eq!(name, "uq_users_email");
        assert!(name.len() <= 63);
    }

    #[test]
    fn test_generate_uq_constraint_name_composite() {
        // 複合カラムの場合
        let name = generate_uq_constraint_name(
            "order_items",
            &["order_id".to_string(), "product_id".to_string()],
        );
        assert_eq!(name, "uq_order_items_order_id_product_id");
        assert!(name.len() <= 63);
    }

    #[test]
    fn test_generate_uq_constraint_name_truncated() {
        // 63文字を超える場合はハッシュ付きで切り詰め
        let name = generate_uq_constraint_name(
            "very_long_table_name_with_many_characters",
            &[
                "organization_id".to_string(),
                "department_id".to_string(),
                "another_long_column".to_string(),
            ],
        );

        assert!(
            name.len() <= 63,
            "Constraint name '{}' exceeds 63 characters (len={})",
            name,
            name.len()
        );
        assert!(name.starts_with("uq_"));
    }

    #[test]
    fn test_generate_uq_constraint_name_deterministic() {
        // 同じ入力には同じ出力を保証
        let name1 = generate_uq_constraint_name(
            "very_long_table_name_with_many_characters",
            &["organization_id".to_string()],
        );
        let name2 = generate_uq_constraint_name(
            "very_long_table_name_with_many_characters",
            &["organization_id".to_string()],
        );
        assert_eq!(name1, name2);
    }

    // ==========================================
    // generate_ck_constraint_name のテスト
    // ==========================================

    #[test]
    fn test_generate_ck_constraint_name_short() {
        // 63文字以下の場合はそのまま返す
        let name = generate_ck_constraint_name("users", &["age".to_string()]);
        assert_eq!(name, "ck_users_age");
        assert!(name.len() <= 63);
    }

    #[test]
    fn test_generate_ck_constraint_name_composite() {
        // 複合カラムの場合
        let name =
            generate_ck_constraint_name("products", &["price".to_string(), "discount".to_string()]);
        assert_eq!(name, "ck_products_price_discount");
        assert!(name.len() <= 63);
    }

    #[test]
    fn test_generate_ck_constraint_name_truncated() {
        // 63文字を超える場合はハッシュ付きで切り詰め
        let name = generate_ck_constraint_name(
            "very_long_table_name_with_many_characters",
            &[
                "organization_id".to_string(),
                "department_id".to_string(),
                "another_long_column".to_string(),
            ],
        );

        assert!(
            name.len() <= 63,
            "Constraint name '{}' exceeds 63 characters (len={})",
            name,
            name.len()
        );
        assert!(name.starts_with("ck_"));
    }

    #[test]
    fn test_generate_ck_constraint_name_deterministic() {
        // 同じ入力には同じ出力を保証
        let name1 = generate_ck_constraint_name(
            "very_long_table_name_with_many_characters",
            &["organization_id".to_string()],
        );
        let name2 = generate_ck_constraint_name(
            "very_long_table_name_with_many_characters",
            &["organization_id".to_string()],
        );
        assert_eq!(name1, name2);
    }

    // ==========================================
    // generate_constraint_name 共通ヘルパーのテスト
    // ==========================================

    #[test]
    fn test_different_prefixes_produce_different_names() {
        // uq_ と ck_ は同じ入力でも異なる名前を生成する
        let uq_name = generate_uq_constraint_name("users", &["email".to_string()]);
        let ck_name = generate_ck_constraint_name("users", &["email".to_string()]);
        assert_ne!(uq_name, ck_name);
        assert!(uq_name.starts_with("uq_"));
        assert!(ck_name.starts_with("ck_"));
    }

    #[test]
    fn test_generate_fk_constraint_name_different_inputs_different_outputs() {
        // 異なる入力には異なる出力（ハッシュが異なる）
        let name1 = generate_fk_constraint_name(
            "very_long_table_name_with_many_characters_a",
            &["column_id".to_string()],
            "referenced_table",
        );
        let name2 = generate_fk_constraint_name(
            "very_long_table_name_with_many_characters_b",
            &["column_id".to_string()],
            "referenced_table",
        );

        // 両方63文字以下
        assert!(name1.len() <= 63);
        assert!(name2.len() <= 63);

        // 異なる名前が生成される
        assert_ne!(name1, name2);
    }

    // ==========================================
    // validate_check_expression のテスト
    // ==========================================

    #[test]
    fn test_validate_check_expression_valid_simple() {
        assert!(validate_check_expression("age > 0").is_ok());
    }

    #[test]
    fn test_validate_check_expression_valid_complex() {
        assert!(validate_check_expression("price >= 0 AND discount <= 100").is_ok());
    }

    #[test]
    fn test_validate_check_expression_valid_in_clause() {
        assert!(validate_check_expression("status IN ('active', 'inactive')").is_ok());
    }

    #[test]
    fn test_validate_check_expression_rejects_insert() {
        let result = validate_check_expression("1); INSERT INTO t VALUES (1");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("INSERT"));
    }

    #[test]
    fn test_validate_check_expression_rejects_update() {
        let result = validate_check_expression("1); UPDATE t SET x = 1");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("UPDATE"));
    }

    #[test]
    fn test_validate_check_expression_rejects_delete() {
        let result = validate_check_expression("1); DELETE FROM t");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("DELETE"));
    }

    #[test]
    fn test_validate_check_expression_rejects_drop() {
        let result = validate_check_expression("1); DROP TABLE users");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("DROP"));
    }

    #[test]
    fn test_validate_check_expression_rejects_alter() {
        let result = validate_check_expression("1); ALTER TABLE users ADD COLUMN x INT");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ALTER"));
    }

    #[test]
    fn test_validate_check_expression_rejects_create() {
        let result = validate_check_expression("1); CREATE TABLE evil (id INT)");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CREATE"));
    }

    #[test]
    fn test_validate_check_expression_rejects_semicolon() {
        let result = validate_check_expression("age > 0; SELECT 1");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("セミコロン"));
    }

    #[test]
    fn test_validate_check_expression_rejects_truncate() {
        let result = validate_check_expression("1); TRUNCATE users");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("TRUNCATE"));
    }

    #[test]
    fn test_validate_check_expression_case_insensitive() {
        // 大文字・小文字を問わず検出
        assert!(validate_check_expression("1); drop table users").is_err());
        assert!(validate_check_expression("1); Drop Table users").is_err());
    }

    #[test]
    fn test_validate_check_expression_keyword_as_substring_allowed() {
        // "updated_at" のようなカラム名に含まれるサブストリングは許可
        // "updated_at" は split_whitespace でトークン化され、trim後 "updated_at" ≠ "UPDATE"
        assert!(validate_check_expression("updated_at IS NOT NULL").is_ok());
        assert!(validate_check_expression("created_by IS NOT NULL").is_ok());
    }

    // ==========================================
    // format_check_constraint のテスト
    // ==========================================

    #[test]
    fn test_format_check_constraint_valid() {
        assert_eq!(format_check_constraint("price >= 0"), "CHECK (price >= 0)");
    }

    #[test]
    fn test_format_check_constraint_invalid_produces_false() {
        let result = format_check_constraint("1); DROP TABLE users");
        assert!(result.contains("CHECK (FALSE)"));
        assert!(result.contains("ERROR"));
    }

    #[test]
    fn test_format_check_constraint_sanitizes_comment_close() {
        // エラーメッセージに */ が含まれる入力でもSQLコメントが壊れないこと
        let result = format_check_constraint("1; */ DROP TABLE users");
        assert!(result.contains("CHECK (FALSE)"));
        // 正当なコメント閉じ `*/` は1つだけ（フォーマットの `/* ... */` 由来）
        // ユーザー入力由来の `*/` は `* /` にサニタイズされている
        assert!(
            result.contains("* /"),
            "Expected sanitized '* /' in: {}",
            result
        );
        // `*/` の出現回数が1回（フォーマット由来のみ）であること
        let close_count = result.matches("*/").count();
        assert_eq!(close_count, 1, "Expected exactly 1 '*/' in: {}", result);
    }

    // ==========================================
    // sanitize_sql_comment のテスト
    // ==========================================

    #[test]
    fn test_sanitize_sql_comment_no_change() {
        assert_eq!(sanitize_sql_comment("Hello world"), "Hello world");
    }

    #[test]
    fn test_sanitize_sql_comment_replaces_close() {
        assert_eq!(sanitize_sql_comment("a */ b"), "a * / b");
    }

    #[test]
    fn test_sanitize_sql_comment_multiple_closes() {
        assert_eq!(sanitize_sql_comment("a */ b */ c"), "a * / b * / c");
    }

    // ==========================================
    // build_column_definition のテスト
    // ==========================================

    #[test]
    fn test_build_column_definition_basic() {
        let column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );
        let result = build_column_definition("\"name\"", &column, "VARCHAR(100)".to_string(), &[]);
        assert_eq!(result, "\"name\" VARCHAR(100) NOT NULL");
    }

    #[test]
    fn test_build_column_definition_nullable() {
        let column = Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            true,
        );
        let result = build_column_definition("\"email\"", &column, "VARCHAR(255)".to_string(), &[]);
        assert_eq!(result, "\"email\" VARCHAR(255)");
    }

    #[test]
    fn test_build_column_definition_with_default() {
        let mut column = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 20 },
            false,
        );
        column.default_value = Some("'active'".to_string());
        let result = build_column_definition("\"status\"", &column, "VARCHAR(20)".to_string(), &[]);
        assert_eq!(result, "\"status\" VARCHAR(20) NOT NULL DEFAULT 'active'");
    }

    #[test]
    fn test_build_column_definition_with_extra_parts() {
        let column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let result = build_column_definition(
            "\"id\"",
            &column,
            "INTEGER".to_string(),
            &["AUTO_INCREMENT"],
        );
        assert_eq!(result, "\"id\" INTEGER NOT NULL AUTO_INCREMENT");
    }

    #[test]
    fn test_build_column_definition_empty_extra_parts_skipped() {
        let column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let result = build_column_definition(
            "\"id\"",
            &column,
            "INTEGER".to_string(),
            &["", "PRIMARY KEY", ""],
        );
        assert_eq!(result, "\"id\" INTEGER NOT NULL PRIMARY KEY");
    }

    // ==========================================
    // SqlGenerator trait デフォルト実装のテスト
    // ==========================================

    #[test]
    fn test_should_add_as_table_constraint_default() {
        let gen = DummySqlGenerator;
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };
        // デフォルトはFK以外true
        assert!(gen.should_add_as_table_constraint(&constraint));
    }

    #[test]
    fn test_generate_drop_table() {
        let gen = DummySqlGenerator;
        let result = gen.generate_drop_table("users");
        assert_eq!(result, "DROP TABLE \"users\"");
    }

    #[test]
    fn test_generate_drop_index() {
        let gen = DummySqlGenerator;
        let result = gen.generate_drop_index("users", "idx_users_email");
        assert_eq!(result, "DROP INDEX \"idx_users_email\"");
    }

    #[test]
    fn test_generate_rename_table() {
        let gen = DummySqlGenerator;
        let result = gen.generate_rename_table("old_name", "new_name");
        assert_eq!(result, "ALTER TABLE \"old_name\" RENAME TO \"new_name\"");
    }

    #[test]
    fn test_generate_missing_table_notice() {
        let gen = DummySqlGenerator;
        let result = gen.generate_missing_table_notice("users");
        assert!(result.contains("users"));
        assert!(result.contains("NOTE"));
    }

    #[test]
    fn test_generate_create_table() {
        let gen = DummySqlGenerator;
        let mut table = Table::new("users".to_string());
        table.add_column(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.constraints.push(Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        });

        let result = gen.generate_create_table(&table);
        assert!(result.contains("CREATE TABLE"));
        assert!(result.contains("\"users\""));
    }

    #[test]
    fn test_generate_create_index() {
        let gen = DummySqlGenerator;
        let table = Table::new("users".to_string());
        let index = Index {
            name: "idx_users_email".to_string(),
            columns: vec!["email".to_string()],
            unique: false,
        };
        let result = gen.generate_create_index(&table, &index);
        assert!(result.contains("CREATE INDEX"));
        assert!(result.contains("\"idx_users_email\""));
    }

    #[test]
    fn test_generate_create_index_unique() {
        let gen = DummySqlGenerator;
        let table = Table::new("users".to_string());
        let index = Index {
            name: "idx_users_email".to_string(),
            columns: vec!["email".to_string()],
            unique: true,
        };
        let result = gen.generate_create_index(&table, &index);
        assert!(result.contains("CREATE UNIQUE INDEX"));
    }

    #[test]
    fn test_generate_add_column() {
        let gen = DummySqlGenerator;
        let column = Column::new(
            "email".to_string(),
            ColumnType::VARCHAR { length: 255 },
            true,
        );
        let result = gen.generate_add_column("users", &column);
        assert!(result.contains("ALTER TABLE"));
        assert!(result.contains("ADD COLUMN"));
    }

    #[test]
    fn test_generate_drop_column() {
        let gen = DummySqlGenerator;
        let result = gen.generate_drop_column("users", "email");
        assert!(result.contains("ALTER TABLE"));
        assert!(result.contains("DROP COLUMN"));
        assert!(result.contains("\"email\""));
    }
}
