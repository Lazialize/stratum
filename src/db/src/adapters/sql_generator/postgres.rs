// PostgreSQL用SQLジェネレーター
//
// スキーマ定義からPostgreSQL用のDDL文を生成します。

use crate::adapters::sql_generator::{
    build_column_definition, format_check_constraint, generate_ck_constraint_name,
    generate_fk_constraint_name, generate_uq_constraint_name, quote_columns_postgres,
    quote_identifier_postgres, quote_regclass_postgres, sanitize_sql_comment,
    validate_check_expression, MigrationDirection, SqlGenerator,
};
use crate::adapters::type_mapping::TypeMappingService;
use crate::core::config::Dialect;
use crate::core::schema::{Column, ColumnType, Constraint, EnumDefinition, Table};
use crate::core::schema_diff::{ColumnDiff, EnumDiff, RenamedColumn};
use crate::core::type_category::TypeCategory;

/// PostgreSQL用SQLジェネレーター
#[derive(Debug, Clone)]
pub struct PostgresSqlGenerator {
    type_mapping: TypeMappingService,
}

impl PostgresSqlGenerator {
    /// 新しいPostgresSqlGeneratorを作成
    pub fn new() -> Self {
        Self {
            type_mapping: TypeMappingService::new(Dialect::PostgreSQL),
        }
    }

    /// ColumnTypeをPostgreSQLの型文字列にマッピング
    ///
    /// TypeMappingServiceに委譲して型変換を行います。
    fn map_column_type(&self, column_type: &ColumnType, auto_increment: Option<bool>) -> String {
        self.type_mapping
            .to_sql_type_with_auto_increment(column_type, auto_increment)
    }

    /// ENUM値をフォーマット
    fn format_enum_values(&self, values: &[String]) -> String {
        values
            .iter()
            .map(|value| format!("'{}'", self.escape_enum_value(value)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// ENUM値をエスケープ
    fn escape_enum_value(&self, value: &str) -> String {
        value.replace('\'', "''")
    }

    /// 型変更SQLを生成
    ///
    /// auto_incrementの変更と同時に型変更がある場合も処理する（例: INTEGER→BIGSERIAL）
    ///
    /// # USING句について
    ///
    /// PostgreSQLではALTER COLUMN TYPEで暗黙のキャストが存在しない型変換を行う場合、
    /// USING句が必要です。現在の実装では `USING "column"::TYPE` 形式の単純キャストのみ
    /// 自動生成します。
    ///
    /// 以下のケースでは、生成されたマイグレーションを手動で修正する必要があります:
    /// - 文字列→タイムスタンプ変換でカスタムフォーマットが必要な場合
    ///   例: `USING to_timestamp("col", 'YYYY-MM-DD HH24:MI:SS')`
    /// - 複合的な変換ロジックが必要な場合
    ///   例: `USING CASE WHEN "col" = 'yes' THEN true ELSE false END`
    /// - ENUM型への変換で中間キャストが必要な場合
    ///   例: `USING "col"::text::new_enum_type`
    ///
    /// 自動生成されるマイグレーションファイルを確認し、
    /// 必要に応じてUSING句を適切な変換ロジックに修正してください。
    #[allow(clippy::too_many_arguments)]
    fn generate_type_change_sql(
        &self,
        source_type: &ColumnType,
        target_type: &ColumnType,
        target_is_auto: bool,
        target_auto_increment: Option<bool>,
        quoted_table: &str,
        quoted_column: &str,
        statements: &mut Vec<String>,
    ) {
        if source_type == target_type {
            return;
        }

        // auto_incrementがtrueの場合、SERIAL系の型名ではなく基底の整数型を使用
        // （シーケンス設定は別途処理）
        let target_type_str = if target_is_auto {
            self.map_column_type(target_type, Some(false))
        } else {
            self.map_column_type(target_type, target_auto_increment)
        };

        let needs_using = self.needs_using_clause(source_type, target_type);

        let sql = if needs_using {
            format!(
                "ALTER TABLE {} ALTER COLUMN {} TYPE {} USING {}::{}",
                quoted_table, quoted_column, target_type_str, quoted_column, target_type_str
            )
        } else {
            format!(
                "ALTER TABLE {} ALTER COLUMN {} TYPE {}",
                quoted_table, quoted_column, target_type_str
            )
        };
        statements.push(sql);
    }

    /// INTEGER → SERIAL (auto_increment: false → true) のSQL生成
    ///
    /// PostgreSQLではALTER COLUMN TYPE SERIALは使用できないため、
    /// シーケンスの作成とDEFAULT設定で対応
    #[allow(clippy::too_many_arguments)]
    fn generate_add_auto_increment_sql(
        &self,
        source_is_auto: bool,
        target_is_auto: bool,
        table_name: &str,
        column_name: &str,
        quoted_table: &str,
        quoted_column: &str,
        statements: &mut Vec<String>,
    ) {
        if source_is_auto || !target_is_auto {
            return;
        }

        let sequence_name = format!("{}_{}_seq", table_name, column_name);
        let quoted_sequence = quote_identifier_postgres(&sequence_name);
        let regclass_literal = quote_regclass_postgres(&sequence_name);
        statements.push(format!("CREATE SEQUENCE IF NOT EXISTS {}", quoted_sequence));
        // 既存データがある場合に備えてシーケンスを最大値に初期化
        // COALESCE(..., 0) により空テーブルでは nextval() が 1 を返す
        // 第3引数 true により次の nextval() は max+1 を返す
        statements.push(format!(
            "SELECT setval({}, COALESCE((SELECT MAX({}) FROM {}), 0), true)",
            regclass_literal, quoted_column, quoted_table
        ));
        statements.push(format!(
            "ALTER TABLE {} ALTER COLUMN {} SET DEFAULT nextval({})",
            quoted_table, quoted_column, regclass_literal
        ));
        statements.push(format!(
            "ALTER SEQUENCE {} OWNED BY {}.{}",
            quoted_sequence, quoted_table, quoted_column
        ));
    }

    /// SERIAL → INTEGER (auto_increment: true → false) のSQL生成
    ///
    /// シーケンスはこのカラム専用として作成されたものと仮定し、
    /// DROP SEQUENCE IF EXISTS CASCADE で安全に削除を試みる
    #[allow(clippy::too_many_arguments)]
    fn generate_remove_auto_increment_sql(
        &self,
        source_is_auto: bool,
        target_is_auto: bool,
        table_name: &str,
        column_name: &str,
        quoted_table: &str,
        quoted_column: &str,
        statements: &mut Vec<String>,
    ) {
        if !source_is_auto || target_is_auto {
            return;
        }

        statements.push(format!(
            "ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT",
            quoted_table, quoted_column
        ));
        let sequence_name = format!("{}_{}_seq", table_name, column_name);
        let quoted_sequence = quote_identifier_postgres(&sequence_name);
        statements.push(format!(
            "DROP SEQUENCE IF EXISTS {} CASCADE",
            quoted_sequence
        ));
    }

    /// USING句が必要かどうかを判定
    ///
    /// TypeCategoryベースでUSING句の自動生成を判定します。
    /// design.mdの「USING句生成ルール」に基づく実装。
    ///
    /// # 制限事項
    ///
    /// USING句が必要と判定された場合、`USING "column"::TARGET_TYPE` 形式の単純キャストを生成します。
    /// 複雑な型変換（例: 文字列→タイムスタンプのフォーマット指定、条件付きキャスト）には対応していません。
    /// そのようなケースでは、生成されたマイグレーションSQLを手動で修正してください。
    fn needs_using_clause(&self, source_type: &ColumnType, target_type: &ColumnType) -> bool {
        let source_category = TypeCategory::from_column_type(source_type);
        let target_category = TypeCategory::from_column_type(target_type);

        use TypeCategory::*;

        match (source_category, target_category) {
            // 同一カテゴリ内: 不要
            (Numeric, Numeric)
            | (String, String)
            | (DateTime, DateTime)
            | (Binary, Binary)
            | (Json, Json)
            | (Boolean, Boolean)
            | (Uuid, Uuid) => false,

            // String → Numeric/Boolean/DateTime/Json: 必要
            (String, Numeric) | (String, Boolean) | (String, DateTime) | (String, Json) => true,

            // Numeric → String: 不要（暗黙変換）
            (Numeric, String) => false,

            // DateTime → String: 不要（暗黙変換）
            (DateTime, String) => false,

            // Boolean → Numeric/String: 不要（暗黙変換）
            (Boolean, Numeric) | (Boolean, String) => false,

            // Uuid → String: 不要（暗黙変換）
            (Uuid, String) => false,

            // Json → String: 不要（暗黙変換）
            (Json, String) => false,

            // Binary → String: 不要（暗黙変換）
            (Binary, String) => false,

            // Otherカテゴリ: 安全のためUSING句を付与
            (Other, _) | (_, Other) => true,

            // その他の変換: 安全のためUSING句を付与
            _ => true,
        }
    }
}

impl SqlGenerator for PostgresSqlGenerator {
    fn quote_identifier(&self, name: &str) -> String {
        quote_identifier_postgres(name)
    }

    fn quote_columns(&self, columns: &[String]) -> String {
        quote_columns_postgres(columns)
    }

    fn generate_column_definition(&self, column: &Column) -> String {
        let type_str = self.map_column_type(&column.column_type, column.auto_increment);
        let quoted_name = quote_identifier_postgres(&column.name);
        build_column_definition(&quoted_name, column, type_str, &[])
    }

    fn generate_constraint_definition(&self, constraint: &Constraint) -> String {
        match constraint {
            Constraint::PRIMARY_KEY { columns } => {
                format!("PRIMARY KEY ({})", quote_columns_postgres(columns))
            }
            Constraint::UNIQUE { columns } => {
                format!("UNIQUE ({})", quote_columns_postgres(columns))
            }
            Constraint::CHECK {
                check_expression, ..
            } => format_check_constraint(check_expression),
            Constraint::FOREIGN_KEY { .. } => {
                // FOREIGN KEY制約はALTER TABLEで追加するため、ここでは空文字列を返す
                String::new()
            }
        }
    }

    fn generate_create_enum_type(&self, enum_def: &EnumDefinition) -> Vec<String> {
        let values = self.format_enum_values(&enum_def.values);
        vec![format!(
            "CREATE TYPE {} AS ENUM ({})",
            quote_identifier_postgres(&enum_def.name),
            values
        )]
    }

    fn generate_add_enum_value(&self, enum_name: &str, value: &str) -> Vec<String> {
        vec![format!(
            "ALTER TYPE {} ADD VALUE '{}'",
            quote_identifier_postgres(enum_name),
            self.escape_enum_value(value)
        )]
    }

    fn generate_recreate_enum_type(&self, enum_diff: &EnumDiff) -> Vec<String> {
        let old_name = format!("{}_old", enum_diff.enum_name);
        let values = self.format_enum_values(&enum_diff.new_values);
        let mut statements = Vec::new();

        statements.push(format!(
            "ALTER TYPE {} RENAME TO {}",
            quote_identifier_postgres(&enum_diff.enum_name),
            quote_identifier_postgres(&old_name)
        ));
        statements.push(format!(
            "CREATE TYPE {} AS ENUM ({})",
            quote_identifier_postgres(&enum_diff.enum_name),
            values
        ));

        for column in &enum_diff.columns {
            statements.push(format!(
                "ALTER TABLE {} ALTER COLUMN {} TYPE {} USING {}::text::{}",
                quote_identifier_postgres(&column.table_name),
                quote_identifier_postgres(&column.column_name),
                quote_identifier_postgres(&enum_diff.enum_name),
                quote_identifier_postgres(&column.column_name),
                quote_identifier_postgres(&enum_diff.enum_name)
            ));
        }

        statements.push(format!(
            "DROP TYPE {}",
            quote_identifier_postgres(&old_name)
        ));
        statements
    }

    fn generate_drop_enum_type(&self, enum_name: &str) -> Vec<String> {
        vec![format!(
            "DROP TYPE {}",
            quote_identifier_postgres(enum_name)
        )]
    }

    fn generate_alter_column_nullable(
        &self,
        table_name: &str,
        column: &Column,
        new_nullable: bool,
    ) -> Vec<String> {
        let action = if new_nullable {
            "DROP NOT NULL"
        } else {
            "SET NOT NULL"
        };
        vec![format!(
            "ALTER TABLE {} ALTER COLUMN {} {}",
            quote_identifier_postgres(table_name),
            quote_identifier_postgres(&column.name),
            action
        )]
    }

    fn generate_alter_column_default(
        &self,
        table_name: &str,
        column: &Column,
        new_default: Option<&str>,
    ) -> Vec<String> {
        let action = match new_default {
            Some(val) => format!("SET DEFAULT {}", val),
            None => "DROP DEFAULT".to_string(),
        };
        vec![format!(
            "ALTER TABLE {} ALTER COLUMN {} {}",
            quote_identifier_postgres(table_name),
            quote_identifier_postgres(&column.name),
            action
        )]
    }

    fn generate_alter_column_type(
        &self,
        table: &Table,
        column_diff: &ColumnDiff,
        direction: MigrationDirection,
    ) -> Vec<String> {
        let column_name = &column_diff.column_name;
        let quoted_table = quote_identifier_postgres(&table.name);
        let quoted_column = quote_identifier_postgres(column_name);

        // 方向に応じて対象の型とauto_incrementフラグを決定
        let (source_type, target_type, source_auto_increment, target_auto_increment) =
            match direction {
                MigrationDirection::Up => (
                    &column_diff.old_column.column_type,
                    &column_diff.new_column.column_type,
                    column_diff.old_column.auto_increment,
                    column_diff.new_column.auto_increment,
                ),
                MigrationDirection::Down => (
                    &column_diff.new_column.column_type,
                    &column_diff.old_column.column_type,
                    column_diff.new_column.auto_increment,
                    column_diff.old_column.auto_increment,
                ),
            };

        let mut statements = Vec::new();

        let source_is_auto = source_auto_increment.unwrap_or(false);
        let target_is_auto = target_auto_increment.unwrap_or(false);

        // 型変更はシーケンス作成より先に実行（型の不一致を避けるため）
        self.generate_type_change_sql(
            source_type,
            target_type,
            target_is_auto,
            target_auto_increment,
            &quoted_table,
            &quoted_column,
            &mut statements,
        );

        self.generate_add_auto_increment_sql(
            source_is_auto,
            target_is_auto,
            &table.name,
            column_name,
            &quoted_table,
            &quoted_column,
            &mut statements,
        );

        self.generate_remove_auto_increment_sql(
            source_is_auto,
            target_is_auto,
            &table.name,
            column_name,
            &quoted_table,
            &quoted_column,
            &mut statements,
        );

        statements
    }

    fn generate_rename_column(
        &self,
        table: &Table,
        renamed_column: &RenamedColumn,
        direction: MigrationDirection,
    ) -> Vec<String> {
        let (from_name, to_name) = match direction {
            MigrationDirection::Up => (&renamed_column.old_name, &renamed_column.new_column.name),
            MigrationDirection::Down => (&renamed_column.new_column.name, &renamed_column.old_name),
        };

        vec![format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {}",
            quote_identifier_postgres(&table.name),
            quote_identifier_postgres(from_name),
            quote_identifier_postgres(to_name)
        )]
    }

    fn generate_add_constraint_for_existing_table(
        &self,
        table_name: &str,
        constraint: &Constraint,
    ) -> String {
        match constraint {
            Constraint::FOREIGN_KEY {
                columns,
                referenced_table,
                referenced_columns,
                on_delete,
                on_update,
            } => {
                let constraint_name =
                    generate_fk_constraint_name(table_name, columns, referenced_table);

                let mut sql = format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
                    quote_identifier_postgres(table_name),
                    quote_identifier_postgres(&constraint_name),
                    quote_columns_postgres(columns),
                    quote_identifier_postgres(referenced_table),
                    quote_columns_postgres(referenced_columns)
                );

                if let Some(action) = on_delete {
                    sql.push_str(&format!(" ON DELETE {}", action.as_sql()));
                }
                if let Some(action) = on_update {
                    sql.push_str(&format!(" ON UPDATE {}", action.as_sql()));
                }

                sql
            }
            Constraint::UNIQUE { columns } => {
                let constraint_name = generate_uq_constraint_name(table_name, columns);

                format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} UNIQUE ({})",
                    quote_identifier_postgres(table_name),
                    quote_identifier_postgres(&constraint_name),
                    quote_columns_postgres(columns)
                )
            }
            Constraint::CHECK {
                columns,
                check_expression,
            } => {
                let constraint_name = generate_ck_constraint_name(table_name, columns);

                if let Err(msg) = validate_check_expression(check_expression) {
                    let sanitized_msg = sanitize_sql_comment(&msg);
                    return format!(
                        "/* ERROR: {} */ ALTER TABLE {} ADD CONSTRAINT {} CHECK (FALSE)",
                        sanitized_msg,
                        quote_identifier_postgres(table_name),
                        quote_identifier_postgres(&constraint_name),
                    );
                }
                format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} CHECK ({})",
                    quote_identifier_postgres(table_name),
                    quote_identifier_postgres(&constraint_name),
                    check_expression
                )
            }
            _ => {
                // PRIMARY_KEYは空文字列を返す
                String::new()
            }
        }
    }

    fn generate_drop_constraint_for_existing_table(
        &self,
        table_name: &str,
        constraint: &Constraint,
    ) -> String {
        match constraint {
            Constraint::FOREIGN_KEY {
                columns,
                referenced_table,
                ..
            } => {
                let constraint_name =
                    generate_fk_constraint_name(table_name, columns, referenced_table);

                format!(
                    "ALTER TABLE {} DROP CONSTRAINT IF EXISTS {}",
                    quote_identifier_postgres(table_name),
                    quote_identifier_postgres(&constraint_name)
                )
            }
            Constraint::UNIQUE { columns } => {
                let constraint_name = generate_uq_constraint_name(table_name, columns);

                format!(
                    "ALTER TABLE {} DROP CONSTRAINT IF EXISTS {}",
                    quote_identifier_postgres(table_name),
                    quote_identifier_postgres(&constraint_name)
                )
            }
            Constraint::CHECK { columns, .. } => {
                let constraint_name = generate_ck_constraint_name(table_name, columns);

                format!(
                    "ALTER TABLE {} DROP CONSTRAINT IF EXISTS {}",
                    quote_identifier_postgres(table_name),
                    quote_identifier_postgres(&constraint_name)
                )
            }
            _ => {
                // PRIMARY_KEYは空文字列を返す
                String::new()
            }
        }
    }
}

impl Default for PostgresSqlGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_generator() {
        let generator = PostgresSqlGenerator::new();
        assert!(format!("{:?}", generator).contains("PostgresSqlGenerator"));
    }

    #[test]
    fn test_map_column_type_integer() {
        let generator = PostgresSqlGenerator::new();
        let col_type = ColumnType::INTEGER { precision: None };
        assert_eq!(generator.map_column_type(&col_type, None), "INTEGER");
    }

    #[test]
    fn test_map_column_type_serial() {
        let generator = PostgresSqlGenerator::new();
        let col_type = ColumnType::INTEGER { precision: None };
        assert_eq!(generator.map_column_type(&col_type, Some(true)), "SERIAL");
    }

    #[test]
    fn test_map_column_type_varchar() {
        let generator = PostgresSqlGenerator::new();
        let col_type = ColumnType::VARCHAR { length: 255 };
        assert_eq!(generator.map_column_type(&col_type, None), "VARCHAR(255)");
    }

    #[test]
    fn test_map_column_type_timestamp_with_tz() {
        let generator = PostgresSqlGenerator::new();
        let col_type = ColumnType::TIMESTAMP {
            with_time_zone: Some(true),
        };
        assert_eq!(
            generator.map_column_type(&col_type, None),
            "TIMESTAMP WITH TIME ZONE"
        );
    }

    #[test]
    fn test_generate_column_definition() {
        let generator = PostgresSqlGenerator::new();
        let column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 100 },
            false,
        );

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, r#""name" VARCHAR(100) NOT NULL"#);
    }

    #[test]
    fn test_generate_column_definition_nullable() {
        let generator = PostgresSqlGenerator::new();
        let column = Column::new(
            "bio".to_string(),
            ColumnType::TEXT,
            true, // nullable
        );

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, r#""bio" TEXT"#);
    }

    #[test]
    fn test_generate_column_definition_with_default() {
        let generator = PostgresSqlGenerator::new();
        let mut column = Column::new(
            "status".to_string(),
            ColumnType::VARCHAR { length: 20 },
            false,
        );
        column.default_value = Some("'active'".to_string());

        let def = generator.generate_column_definition(&column);
        assert_eq!(def, r#""status" VARCHAR(20) NOT NULL DEFAULT 'active'"#);
    }

    #[test]
    fn test_generate_constraint_primary_key() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, r#"PRIMARY KEY ("id")"#);
    }

    #[test]
    fn test_generate_constraint_unique() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, r#"UNIQUE ("email")"#);
    }

    #[test]
    fn test_generate_constraint_check() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        };

        let def = generator.generate_constraint_definition(&constraint);
        assert_eq!(def, "CHECK (price >= 0)");
    }

    // ==========================================
    // generate_alter_column_type のテスト
    // ==========================================

    use crate::adapters::sql_generator::MigrationDirection;
    use crate::core::schema_diff::ColumnDiff;

    fn create_test_table() -> Table {
        let mut table = Table::new("users".to_string());
        table.columns.push(Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        ));
        table.columns.push(Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 255 },
            false,
        ));
        table
    }

    #[test]
    fn test_alter_column_type_same_category_no_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // INTEGER → BIGINT（同じNumericカテゴリ内）
        let old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) },
            false,
        );
        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" ALTER COLUMN "id" TYPE BIGINT"#
        );
    }

    #[test]
    fn test_alter_column_type_numeric_to_string_no_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // INTEGER → TEXT（暗黙変換可能）
        let old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let new_column = Column::new("id".to_string(), ColumnType::TEXT, false);
        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(sql[0], r#"ALTER TABLE "users" ALTER COLUMN "id" TYPE TEXT"#);
    }

    #[test]
    fn test_alter_column_type_string_to_numeric_with_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // TEXT → INTEGER（USING句が必要）
        let old_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let new_column = Column::new(
            "name".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" ALTER COLUMN "name" TYPE INTEGER USING "name"::INTEGER"#
        );
    }

    #[test]
    fn test_alter_column_type_string_to_boolean_with_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // VARCHAR → BOOLEAN（USING句が必要）
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::VARCHAR { length: 10 },
            false,
        );
        let new_column = Column::new("name".to_string(), ColumnType::BOOLEAN, false);
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" ALTER COLUMN "name" TYPE BOOLEAN USING "name"::BOOLEAN"#
        );
    }

    #[test]
    fn test_alter_column_type_string_to_json_with_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // TEXT → JSONB（USING句が必要）
        let old_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let new_column = Column::new("name".to_string(), ColumnType::JSONB, false);
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" ALTER COLUMN "name" TYPE JSONB USING "name"::JSONB"#
        );
    }

    #[test]
    fn test_alter_column_type_down_direction() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // Down方向: old_columnの型に戻す
        let old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        let new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) },
            false,
        );
        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Down);

        assert_eq!(sql.len(), 1);
        // Down方向なので old_type (INTEGER) に戻す
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" ALTER COLUMN "id" TYPE INTEGER"#
        );
    }

    #[test]
    fn test_alter_column_type_datetime_to_string_no_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // TIMESTAMP → TEXT（暗黙変換可能）
        let old_column = Column::new(
            "name".to_string(),
            ColumnType::TIMESTAMP {
                with_time_zone: None,
            },
            false,
        );
        let new_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" ALTER COLUMN "name" TYPE TEXT"#
        );
    }

    #[test]
    fn test_alter_column_type_string_to_datetime_with_using() {
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        // TEXT → TIMESTAMP（USING句が必要）
        let old_column = Column::new("name".to_string(), ColumnType::TEXT, false);
        let new_column = Column::new(
            "name".to_string(),
            ColumnType::TIMESTAMP {
                with_time_zone: None,
            },
            false,
        );
        let diff = ColumnDiff::new("name".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" ALTER COLUMN "name" TYPE TIMESTAMP USING "name"::TIMESTAMP"#
        );
    }

    // ==========================================
    // generate_rename_column のテスト
    // ==========================================

    use crate::core::schema_diff::RenamedColumn;

    #[test]
    fn test_generate_rename_column_up() {
        // Up方向：old_name → new_name
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

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
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let sql = generator.generate_rename_column(&table, &renamed, MigrationDirection::Up);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" RENAME COLUMN "name" TO "user_name""#
        );
    }

    #[test]
    fn test_generate_rename_column_down() {
        // Down方向：new_name → old_name（逆リネーム）
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

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
        let renamed = RenamedColumn {
            old_name: "name".to_string(),
            old_column,
            new_column,
            changes: vec![],
        };

        let sql = generator.generate_rename_column(&table, &renamed, MigrationDirection::Down);

        assert_eq!(sql.len(), 1);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" RENAME COLUMN "user_name" TO "name""#
        );
    }

    // ==========================================
    // SERIAL変換のテスト
    // ==========================================

    #[test]
    fn test_alter_column_integer_to_serial() {
        // INTEGER → SERIAL: 型は同じだがauto_incrementが変わる
        // 型変更SQLは生成されず、シーケンス関連のSQLのみ生成される
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(false);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        new_column.auto_increment = Some(true);

        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        // 型変更SQLは含まれない（INTEGERのまま）
        // シーケンス関連のSQLが4つ生成される
        assert_eq!(sql.len(), 4);
        assert!(sql[0].contains("CREATE SEQUENCE"));
        assert!(sql[1].contains("setval"));
        assert!(sql[2].contains("SET DEFAULT nextval"));
        assert!(sql[3].contains("OWNED BY"));

        // 型変更SQLがないことを確認
        assert!(!sql.iter().any(|s| s.contains("ALTER COLUMN id TYPE")));
    }

    #[test]
    fn test_alter_column_integer_to_bigserial() {
        // INTEGER → BIGSERIAL: 型もauto_incrementも変わる
        // 型変更SQLが1回だけ生成され、その後にシーケンス関連SQLが続く
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(false);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) }, // BIGINT
            false,
        );
        new_column.auto_increment = Some(true);

        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        // 型変更SQL(1) + シーケンス関連SQL(4) = 5
        assert_eq!(sql.len(), 5);

        // 最初は型変更SQL（BIGINT、SERIALではない）
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" ALTER COLUMN "id" TYPE BIGINT"#
        );

        // シーケンス関連SQL
        assert!(sql[1].contains("CREATE SEQUENCE"));
        assert!(sql[2].contains("setval"));
        assert!(sql[3].contains("SET DEFAULT nextval"));
        assert!(sql[4].contains("OWNED BY"));

        // 型変更SQLは1回だけであることを確認
        let type_change_count = sql
            .iter()
            .filter(|s| s.contains(r#"ALTER COLUMN "id" TYPE"#))
            .count();
        assert_eq!(type_change_count, 1);
    }

    #[test]
    fn test_alter_column_serial_to_integer() {
        // SERIAL → INTEGER: auto_incrementがtrueからfalseに
        // DEFAULTドロップとシーケンス削除が生成される
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        old_column.auto_increment = Some(true);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        new_column.auto_increment = Some(false);

        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        // DEFAULTドロップ + シーケンス削除 = 2
        assert_eq!(sql.len(), 2);
        assert!(sql[0].contains("DROP DEFAULT"));
        assert!(sql[1].contains("DROP SEQUENCE IF EXISTS"));
        assert!(sql[1].contains("CASCADE"));
    }

    #[test]
    fn test_alter_column_bigserial_to_integer() {
        // BIGSERIAL → INTEGER: 型もauto_incrementも変わる
        // 型変更SQL + DEFAULTドロップ + シーケンス削除
        let generator = PostgresSqlGenerator::new();
        let table = create_test_table();

        let mut old_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: Some(8) }, // BIGINT
            false,
        );
        old_column.auto_increment = Some(true);

        let mut new_column = Column::new(
            "id".to_string(),
            ColumnType::INTEGER { precision: None },
            false,
        );
        new_column.auto_increment = Some(false);

        let diff = ColumnDiff::new("id".to_string(), old_column, new_column);

        let sql = generator.generate_alter_column_type(&table, &diff, MigrationDirection::Up);

        // 型変更SQL(1) + DEFAULTドロップ(1) + シーケンス削除(1) = 3
        assert_eq!(sql.len(), 3);
        assert_eq!(
            sql[0],
            r#"ALTER TABLE "users" ALTER COLUMN "id" TYPE INTEGER"#
        );
        assert!(sql[1].contains("DROP DEFAULT"));
        assert!(sql[2].contains("DROP SEQUENCE IF EXISTS"));
    }

    // ==========================================
    // 制約メソッドのテスト
    // ==========================================

    #[test]
    fn test_generate_add_constraint_for_existing_table_foreign_key() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        };

        let sql = generator.generate_add_constraint_for_existing_table("posts", &constraint);

        assert_eq!(
            sql,
            r#"ALTER TABLE "posts" ADD CONSTRAINT "fk_posts_user_id_users" FOREIGN KEY ("user_id") REFERENCES "users" ("id")"#
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_composite_foreign_key() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["org_id".to_string(), "user_id".to_string()],
            referenced_table: "org_users".to_string(),
            referenced_columns: vec!["organization_id".to_string(), "user_id".to_string()],
            on_delete: None,
            on_update: None,
        };

        let sql = generator.generate_add_constraint_for_existing_table("posts", &constraint);

        assert_eq!(
            sql,
            r#"ALTER TABLE "posts" ADD CONSTRAINT "fk_posts_org_id_user_id_org_users" FOREIGN KEY ("org_id", "user_id") REFERENCES "org_users" ("organization_id", "user_id")"#
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_unique() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let sql = generator.generate_add_constraint_for_existing_table("users", &constraint);

        assert_eq!(
            sql,
            r#"ALTER TABLE "users" ADD CONSTRAINT "uq_users_email" UNIQUE ("email")"#
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_unique_composite() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["first_name".to_string(), "last_name".to_string()],
        };

        let sql = generator.generate_add_constraint_for_existing_table("users", &constraint);

        assert_eq!(
            sql,
            r#"ALTER TABLE "users" ADD CONSTRAINT "uq_users_first_name_last_name" UNIQUE ("first_name", "last_name")"#
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_check() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        };

        let sql = generator.generate_add_constraint_for_existing_table("products", &constraint);

        assert_eq!(
            sql,
            r#"ALTER TABLE "products" ADD CONSTRAINT "ck_products_price" CHECK (price >= 0)"#
        );
    }

    #[test]
    fn test_generate_add_constraint_for_existing_table_primary_key_returns_empty() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let sql = generator.generate_add_constraint_for_existing_table("users", &constraint);

        assert!(sql.is_empty());
    }

    #[test]
    fn test_generate_drop_constraint_for_existing_table_foreign_key() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::FOREIGN_KEY {
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: None,
            on_update: None,
        };

        let sql = generator.generate_drop_constraint_for_existing_table("posts", &constraint);

        assert_eq!(
            sql,
            r#"ALTER TABLE "posts" DROP CONSTRAINT IF EXISTS "fk_posts_user_id_users""#
        );
    }

    #[test]
    fn test_generate_drop_constraint_for_existing_table_unique() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::UNIQUE {
            columns: vec!["email".to_string()],
        };

        let sql = generator.generate_drop_constraint_for_existing_table("users", &constraint);

        assert_eq!(
            sql,
            r#"ALTER TABLE "users" DROP CONSTRAINT IF EXISTS "uq_users_email""#
        );
    }

    #[test]
    fn test_generate_drop_constraint_for_existing_table_check() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::CHECK {
            columns: vec!["price".to_string()],
            check_expression: "price >= 0".to_string(),
        };

        let sql = generator.generate_drop_constraint_for_existing_table("products", &constraint);

        assert_eq!(
            sql,
            r#"ALTER TABLE "products" DROP CONSTRAINT IF EXISTS "ck_products_price""#
        );
    }

    #[test]
    fn test_generate_drop_constraint_for_existing_table_primary_key_returns_empty() {
        let generator = PostgresSqlGenerator::new();
        let constraint = Constraint::PRIMARY_KEY {
            columns: vec!["id".to_string()],
        };

        let sql = generator.generate_drop_constraint_for_existing_table("users", &constraint);

        assert!(sql.is_empty());
    }
}
