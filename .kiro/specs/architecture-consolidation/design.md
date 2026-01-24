# Technical Design: Architecture Consolidation

## Overview

本設計書は、Stratum コードベースのアーキテクチャ統合リファクタリングの技術設計を定義する。Option C（ハイブリッドアプローチ）に基づき、方言型マッピングとDTO変換は新規モジュール化、export/migration/validation は既存拡張で実装する。

### Design Goals

1. **重複排除:** 型マッピング・DTO変換の二重実装を解消
2. **責務分離:** export コマンドの3層分離（introspection/変換/出力）
3. **安全性向上:** SQL組み立てのパラメータバインド化
4. **保守性向上:** バリデーションの粒度分割

### Requirements Traceability

| 要件ID | 設計コンポーネント |
|--------|-------------------|
| 1 | TypeMappingService, TypeMapper trait |
| 2 | MigrationPipeline |
| 3 | DatabaseIntrospector, SchemaConversionService |
| 4 | SchemaValidatorService（関数分割） |
| 5 | DatabaseMigrator（bind方式） |
| 6 | DtoConverter |
| 7 | 全コンポーネント（後方互換性テスト） |
| 8 | 全コンポーネント（品質基準） |

## Architecture Pattern & Boundary Map

### Layer Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                          CLI Layer                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ ExportCommand   │  │ MigrateCommand  │  │ ValidateCmd    │  │
│  │ Handler         │  │ Handler         │  │ Handler        │  │
│  └────────┬────────┘  └────────┬────────┘  └───────┬────────┘  │
└───────────┼─────────────────────┼──────────────────┼────────────┘
            │                     │                  │
┌───────────▼─────────────────────▼──────────────────▼────────────┐
│                        Services Layer                           │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ SchemaConversion│  │ MigrationPipe   │  │ SchemaValidator│  │
│  │ Service         │  │ line            │  │ Service        │  │
│  └────────┬────────┘  └────────┬────────┘  └───────┬────────┘  │
│           │                    │                   │            │
│  ┌────────▼────────┐  ┌────────▼────────┐                      │
│  │ DtoConverter    │  │ TypeChange      │                      │
│  │                 │  │ Validator       │                      │
│  └─────────────────┘  └─────────────────┘                      │
└───────────┬─────────────────────┬───────────────────────────────┘
            │                     │
┌───────────▼─────────────────────▼───────────────────────────────┐
│                        Adapters Layer                           │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ TypeMapping     │  │ SqlGenerator    │  │ Database       │  │
│  │ Service         │  │ (per dialect)   │  │ Introspector   │  │
│  └────────┬────────┘  └────────┬────────┘  └───────┬────────┘  │
│           │                    │                   │            │
│  ┌────────▼────────────────────▼───────────────────▼────────┐  │
│  │                    TypeMapper Trait                       │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐                │  │
│  │  │ Postgres │  │ MySQL    │  │ SQLite   │                │  │
│  │  │ Mapper   │  │ Mapper   │  │ Mapper   │                │  │
│  │  └──────────┘  └──────────┘  └──────────┘                │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
            │
┌───────────▼─────────────────────────────────────────────────────┐
│                          Core Layer                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ Schema          │  │ ColumnType      │  │ Dialect        │  │
│  │                 │  │                 │  │                │  │
│  └─────────────────┘  └─────────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Dependency Rules

- **CLI → Services:** コマンドハンドラは services を呼び出す
- **Services → Adapters:** ビジネスロジックは adapters を通じてDB操作
- **Adapters → Core:** 技術的実装は core の型定義に依存
- **禁止:** Core → 他層、Services → CLI、Adapters → Services/CLI

## Technology Stack & Alignment

| 技術 | バージョン | 用途 |
|------|-----------|------|
| Rust | 1.92+ (2021 edition) | 実装言語 |
| Tokio | 1.49 | 非同期ランタイム |
| sqlx | 0.8 | DB操作（AnyPool） |
| serde | 1.x | シリアライズ/デシリアライズ |
| serde-saphyr | 0.0.16 | パニックフリーYAML処理 |
| anyhow | 1.x | アプリケーションエラー |
| thiserror | 2.x | ライブラリエラー型定義 |

**AnyPool + sqlx 0.8 の制約:**
- `sqlx::query!` マクロはコンパイル時に単一方言のDBへの接続が必要
- AnyPool 使用時は `sqlx::query()` + 動的バインドを使用
- 複数方言サポートのため、コンパイル時検証は方言別CIジョブで実施

**Steering との整合性:**
- Clean Architecture パターン準拠（tech.md）
- Result 型によるエラー伝搬
- unwrap/expect 禁止（本番コード）
- パニックフリー設計（serde-saphyrの採用理由）

## Components & Interface Contracts

### Component 1: TypeMappingService (新規)

**ファイル:** `src/adapters/type_mapping.rs`

**責務:** ColumnType と SQL型文字列の双方向変換（唯一の変換ロジック）

#### 既存 `ColumnType::to_sql_type` との統合方針

**現状の問題:**
- `ColumnType::to_sql_type(&self, dialect: &Dialect)` が core 層に存在
- 各 SqlGenerator の `map_column_type` が adapters 層に存在
- export.rs の `parse_*_type` が CLI 層に存在
- 同じ変換ロジックが3箇所に分散 → 要件1違反

**統合戦略:**
1. `TypeMappingService` を adapters 層の唯一の型変換実装とする
2. `ColumnType::to_sql_type` は `TypeMappingService` への委譲に変更
3. SqlGenerator の `map_column_type` は `TypeMappingService` を呼び出す
4. export.rs のパースロジックは `TypeMappingService::from_sql_type` に移行

```rust
// 移行後の ColumnType::to_sql_type（src/core/schema.rs）
impl ColumnType {
    /// SQL型文字列に変換（TypeMappingServiceに委譲）
    ///
    /// 後方互換性のため維持するが、内部は TypeMappingService を使用
    pub fn to_sql_type(&self, dialect: &Dialect) -> String {
        use crate::adapters::type_mapping::TypeMappingService;
        TypeMappingService::new(*dialect).to_sql_type(self)
    }
}
```

#### TypeMappingService インターフェース

```rust
/// 型マッピングサービス
///
/// 方言に依存しない共通インターフェースで型変換を提供
/// ColumnType ↔ SQL型文字列 の双方向変換を一元管理
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

    /// ColumnType → SQL型文字列
    ///
    /// # Arguments
    /// * `column_type` - 変換対象の内部型
    ///
    /// # Returns
    /// SQL型文字列（例: "VARCHAR(255)", "INTEGER"）
    pub fn to_sql_type(&self, column_type: &ColumnType) -> String;

    /// SQL型文字列 → ColumnType
    ///
    /// # Arguments
    /// * `sql_type` - データベースから取得した型文字列
    /// * `metadata` - 追加メタデータ（precision, scaleなど）
    ///
    /// # Returns
    /// 内部型表現、パース失敗時はエラー
    pub fn from_sql_type(
        &self,
        sql_type: &str,
        metadata: &TypeMetadata
    ) -> Result<ColumnType>;
}

/// 型メタデータ（方言固有の追加情報）
pub struct TypeMetadata {
    pub char_max_length: Option<u32>,
    pub numeric_precision: Option<u32>,
    pub numeric_scale: Option<u32>,
    pub udt_name: Option<String>,
}
```

**方言フック:**

```rust
/// 方言固有の型マッピング拡張
pub trait TypeMapper: Send + Sync {
    /// 方言固有の型をパース
    fn parse_dialect_specific(&self, sql_type: &str, metadata: &TypeMetadata) -> Option<ColumnType>;

    /// 方言固有の型をSQL文字列に変換
    fn format_dialect_specific(&self, column_type: &ColumnType) -> Option<String>;

    /// デフォルト型（パース失敗時のフォールバック）
    fn default_type(&self) -> ColumnType {
        ColumnType::TEXT
    }
}

/// 方言別実装
pub struct PostgresTypeMapper;
pub struct MySqlTypeMapper;
pub struct SqliteTypeMapper;
```

**要件マッピング:** Req 1.1, 1.2, 1.3, 1.4, 1.5

---

### Component 2: MigrationPipeline (既存拡張)

**ファイル:** `src/services/migration_generator.rs`

**責務:** マイグレーションSQL生成の共通パイプライン

```rust
/// マイグレーション生成パイプライン
pub struct MigrationPipeline<'a> {
    diff: &'a SchemaDiff,
    old_schema: Option<&'a Schema>,
    new_schema: Option<&'a Schema>,
    dialect: Dialect,
    generator: Box<dyn SqlGenerator>,
}

impl<'a> MigrationPipeline<'a> {
    /// パイプラインを作成
    pub fn new(diff: &'a SchemaDiff, dialect: Dialect) -> Self;

    /// スキーマ情報を設定（型変更検証用）
    pub fn with_schemas(
        self,
        old_schema: &'a Schema,
        new_schema: &'a Schema
    ) -> Self;

    /// UP SQL を生成
    ///
    /// パイプラインステージ:
    /// 1. prepare - SqlGenerator取得、事前検証
    /// 2. enum_statements - ENUM作成/変更（PostgreSQL）
    /// 3. table_statements - CREATE/ALTER TABLE
    /// 4. index_statements - CREATE INDEX
    /// 5. constraint_statements - 制約追加
    /// 6. cleanup_statements - DROP TABLE/TYPE
    /// 7. finalize - SQL結合
    ///
    /// # Returns
    /// (SQL文字列, ValidationResult)
    pub fn generate_up(&self) -> Result<(String, ValidationResult), String>;

    /// DOWN SQL を生成
    pub fn generate_down(&self) -> Result<(String, ValidationResult), String>;
}
```

**既存メソッドとの互換性:**

```rust
impl MigrationGenerator {
    /// 既存API（後方互換性維持）
    pub fn generate_up_sql(&self, diff: &SchemaDiff, dialect: Dialect) -> Result<String, String> {
        let pipeline = MigrationPipeline::new(diff, dialect);
        pipeline.generate_up().map(|(sql, _)| sql)
    }

    /// 既存API（後方互換性維持）
    pub fn generate_up_sql_with_schemas(
        &self,
        diff: &SchemaDiff,
        old_schema: &Schema,
        new_schema: &Schema,
        dialect: Dialect,
    ) -> Result<(String, ValidationResult), String> {
        let pipeline = MigrationPipeline::new(diff, dialect)
            .with_schemas(old_schema, new_schema);
        pipeline.generate_up()
    }
}
```

**要件マッピング:** Req 2.1, 2.2, 2.3, 2.4, 2.5

---

### Component 3: DatabaseIntrospector (新規)

**ファイル:** `src/adapters/database_introspector.rs`

**責務:** データベースからのスキーマ情報取得

```rust
/// データベーススキーマ取得インターフェース
#[async_trait]
pub trait DatabaseIntrospector {
    /// テーブル名一覧を取得
    async fn get_table_names(&self, pool: &AnyPool) -> Result<Vec<String>>;

    /// カラム情報を取得
    async fn get_columns(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawColumnInfo>>;

    /// インデックス情報を取得
    async fn get_indexes(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawIndexInfo>>;

    /// 制約情報を取得
    async fn get_constraints(&self, pool: &AnyPool, table_name: &str) -> Result<Vec<RawConstraintInfo>>;

    /// ENUM定義を取得（PostgreSQL専用）
    async fn get_enums(&self, pool: &AnyPool) -> Result<Vec<RawEnumInfo>>;
}

/// 生のカラム情報（DB固有フォーマット）
pub struct RawColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub default_value: Option<String>,
    pub char_max_length: Option<i32>,
    pub numeric_precision: Option<i32>,
    pub numeric_scale: Option<i32>,
    pub udt_name: Option<String>,
}

/// 方言別実装
pub struct PostgresIntrospector;
pub struct MySqlIntrospector;
pub struct SqliteIntrospector;
```

**要件マッピング:** Req 3.1, 3.2, 3.3

---

### Component 4: SchemaConversionService (新規)

**ファイル:** `src/services/schema_conversion.rs`

**責務:** Raw情報 → 内部モデル変換

```rust
/// スキーマ変換サービス
pub struct SchemaConversionService {
    type_mapping: TypeMappingService,
}

impl SchemaConversionService {
    pub fn new(dialect: Dialect) -> Self;

    /// 生のカラム情報を内部モデルに変換
    pub fn convert_column(&self, raw: &RawColumnInfo) -> Result<Column>;

    /// 生のインデックス情報を内部モデルに変換
    pub fn convert_index(&self, raw: &RawIndexInfo) -> Result<Index>;

    /// 生の制約情報を内部モデルに変換
    pub fn convert_constraint(&self, raw: &RawConstraintInfo) -> Result<Constraint>;

    /// データベースから取得した情報をSchemaに変換
    pub fn build_schema(&self, raw_tables: Vec<RawTableInfo>) -> Result<Schema>;
}
```

**要件マッピング:** Req 3.4, 3.5

---

### Component 5: SchemaValidatorService (既存拡張)

**ファイル:** `src/services/schema_validator.rs`

**責務:** スキーマ検証（カテゴリ別分割）

```rust
impl SchemaValidatorService {
    /// 統合検証エントリポイント（後方互換性維持）
    pub fn validate(&self, schema: &Schema) -> ValidationResult {
        self.validate_internal(schema, None)
    }

    fn validate_internal(&self, schema: &Schema, dialect: Option<Dialect>) -> ValidationResult {
        let mut result = ValidationResult::new();

        // カテゴリ別に分割実行
        result.merge(self.validate_enums(schema, dialect));
        result.merge(self.validate_column_types(schema));
        result.merge(self.validate_primary_keys(schema));
        result.merge(self.validate_index_references(schema));
        result.merge(self.validate_constraint_references(schema));

        result
    }

    /// ENUM検証（PostgreSQL専用チェック含む）
    fn validate_enums(&self, schema: &Schema, dialect: Option<Dialect>) -> ValidationResult;

    /// カラム型検証（DECIMAL, CHAR等の範囲チェック）
    fn validate_column_types(&self, schema: &Schema) -> ValidationResult;

    /// プライマリキー存在確認
    fn validate_primary_keys(&self, schema: &Schema) -> ValidationResult;

    /// インデックスのカラム参照整合性
    fn validate_index_references(&self, schema: &Schema) -> ValidationResult;

    /// 制約のカラム/テーブル参照整合性
    fn validate_constraint_references(&self, schema: &Schema) -> ValidationResult;
}
```

**要件マッピング:** Req 4.1, 4.2, 4.3, 4.4, 4.5

---

### Component 6: DatabaseMigrator (既存拡張)

**ファイル:** `src/adapters/database_migrator.rs`

**責務:** マイグレーション履歴管理（SQL安全化）

#### sqlx::query! マクロの制約と対応方針

**制約:**
- `sqlx::query!` マクロはコンパイル時にDBへの接続が必要
- AnyPool 使用時はコンパイル時に方言を特定できない
- 動的テーブル名（`migration_table_name`）はマクロで使用不可

**対応方針:**
1. **パラメータバインド:** ユーザー入力値は全て `sqlx::query().bind()` でバインド
2. **テーブル名許可リスト:** 動的テーブル名は許可リストで検証
3. **CI検証:** 各方言用のCIジョブで `sqlx::query!` 相当の検証を実施

#### テーブル名の許可リスト検証

```rust
/// 許可されるマイグレーションテーブル名のパターン
const ALLOWED_TABLE_NAME_PATTERN: &str = r"^[a-zA-Z_][a-zA-Z0-9_]{0,62}$";

/// デフォルトのマイグレーションテーブル名
pub const DEFAULT_MIGRATION_TABLE: &str = "schema_migrations";

impl DatabaseMigrator {
    /// マイグレーションテーブル名を検証
    ///
    /// # Security
    /// SQLインジェクション防止のため、テーブル名は以下を満たす必要がある:
    /// - 英字またはアンダースコアで開始
    /// - 英数字とアンダースコアのみで構成
    /// - 最大63文字（PostgreSQL識別子制限）
    fn validate_table_name(name: &str) -> Result<(), MigrationError> {
        let re = regex::Regex::new(ALLOWED_TABLE_NAME_PATTERN).unwrap();
        if !re.is_match(name) {
            return Err(MigrationError::InvalidTableName {
                name: name.to_string(),
                reason: "Table name must start with letter or underscore, contain only alphanumeric characters and underscores, and be at most 63 characters".to_string(),
            });
        }
        Ok(())
    }

    /// 新しいDatabaseMigratorを作成（テーブル名検証付き）
    pub fn new(dialect: Dialect, table_name: Option<&str>) -> Result<Self> {
        let table_name = table_name.unwrap_or(DEFAULT_MIGRATION_TABLE);
        Self::validate_table_name(table_name)?;
        Ok(Self {
            dialect,
            migration_table_name: table_name.to_string(),
        })
    }
}
```

#### 変更前（危険なパターン）:
```rust
fn generate_record_migration_sql(&self, ...) -> String {
    format!(
        "INSERT INTO {} (version, name, ...) VALUES ('{}', '{}', ...)",
        self.migration_table_name, version, name, ...
    )
}
```

#### 変更後（安全なパターン）:
```rust
/// マイグレーション記録クエリを生成（パラメータバインド対応）
///
/// # Security
/// - テーブル名: コンストラクタで許可リスト検証済み
/// - パラメータ値: bind() でエスケープ
///
/// # Returns
/// (SQL文字列, バインドパラメータ)
fn generate_record_migration_query(&self, ...) -> (String, Vec<SqlParam>) {
    // テーブル名はコンストラクタで検証済みのため、format!で埋め込み可
    let sql = match self.dialect {
        Dialect::PostgreSQL => format!(
            "INSERT INTO {} (version, name, checksum, dialect, applied_at) VALUES ($1, $2, $3, $4, NOW())",
            self.migration_table_name
        ),
        Dialect::MySQL => format!(
            "INSERT INTO {} (version, name, checksum, dialect, applied_at) VALUES (?, ?, ?, ?, NOW())",
            self.migration_table_name
        ),
        Dialect::SQLite => format!(
            "INSERT INTO {} (version, name, checksum, dialect, applied_at) VALUES (?, ?, ?, ?, datetime('now'))",
            self.migration_table_name
        ),
    };

    // ユーザー入力値は全てバインドパラメータとして渡す
    let params = vec![
        SqlParam::String(version.to_string()),
        SqlParam::String(name.to_string()),
        SqlParam::String(checksum.to_string()),
        SqlParam::String(dialect.to_string()),
    ];

    (sql, params)
}

/// マイグレーションを記録
pub async fn record_migration(&self, pool: &AnyPool, ...) -> Result<()> {
    let (sql, params) = self.generate_record_migration_query(...);

    let mut query = sqlx::query(&sql);
    for param in params {
        query = query.bind(param.as_str());
    }

    query.execute(pool).await?;
    Ok(())
}
```

#### CI検証戦略（sqlx::query! 代替）

```yaml
# .github/workflows/sql-verify.yml
jobs:
  verify-postgres:
    services:
      postgres:
        image: postgres:15
    steps:
      - run: cargo sqlx prepare --check --database-url $POSTGRES_URL

  verify-mysql:
    services:
      mysql:
        image: mysql:8
    steps:
      - run: cargo sqlx prepare --check --database-url $MYSQL_URL

  verify-sqlite:
    steps:
      - run: cargo sqlx prepare --check --database-url sqlite:test.db
```

**要件マッピング:** Req 5.1, 5.2, 5.3, 5.4, 5.5

---

### Component 7: DtoConverter (新規)

**ファイル:** `src/services/dto_converter.rs`

**責務:** Schema ↔ DTO の双方向変換

```rust
/// DTO変換サービス
///
/// Schema と SchemaDto の双方向変換を一元管理
pub struct DtoConverterService;

impl DtoConverterService {
    pub fn new() -> Self;

    /// Schema → SchemaDto
    pub fn schema_to_dto(&self, schema: &Schema) -> SchemaDto;

    /// SchemaDto → Schema
    pub fn dto_to_schema(&self, dto: &SchemaDto) -> Result<Schema>;

    /// Table → TableDto
    pub fn table_to_dto(&self, table: &Table) -> TableDto;

    /// TableDto → Table（テーブル名を引数で受け取る）
    pub fn dto_to_table(&self, name: &str, dto: &TableDto) -> Result<Table>;

    /// Constraint → ConstraintDto
    pub fn constraint_to_dto(&self, constraint: &Constraint) -> Option<ConstraintDto>;

    /// ConstraintDto → Constraint
    pub fn dto_to_constraint(&self, dto: &ConstraintDto) -> Constraint;
}
```

**ラウンドトリップ保証:**
```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn roundtrip_schema(schema in arbitrary_schema()) {
            let converter = DtoConverterService::new();
            let dto = converter.schema_to_dto(&schema);
            let restored = converter.dto_to_schema(&dto).unwrap();

            // 構造的等価性を検証
            assert_eq!(schema.version, restored.version);
            assert_eq!(schema.tables.len(), restored.tables.len());
            // ... 詳細比較
        }
    }
}
```

**要件マッピング:** Req 6.1, 6.2, 6.3, 6.4, 6.5

## Data Models

### 既存モデル（変更なし）

```rust
// src/core/schema.rs - 変更なし
pub struct Schema { ... }
pub struct Table { ... }
pub struct Column { ... }
pub enum ColumnType { ... }
pub enum Constraint { ... }
```

### 新規モデル

```rust
// src/services/dto.rs への追加なし（既存DTOを再利用）

// src/adapters/type_mapping.rs
pub struct TypeMetadata {
    pub char_max_length: Option<u32>,
    pub numeric_precision: Option<u32>,
    pub numeric_scale: Option<u32>,
    pub udt_name: Option<String>,
}

// src/adapters/database_migrator.rs
pub enum SqlParam {
    String(String),
    Int(i64),
    Bool(bool),
}
```

## Error Handling

### エラー型

```rust
// 型マッピングエラー
#[derive(Debug, thiserror::Error)]
pub enum TypeMappingError {
    #[error("Unknown SQL type: {0}")]
    UnknownType(String),

    #[error("Invalid type parameters for {type_name}: {reason}")]
    InvalidParameters { type_name: String, reason: String },
}

// イントロスペクションエラー
#[derive(Debug, thiserror::Error)]
pub enum IntrospectionError {
    #[error("Database query failed: {0}")]
    QueryFailed(#[from] sqlx::Error),

    #[error("Unsupported dialect feature: {0}")]
    UnsupportedFeature(String),
}
```

### エラー伝搬

- 全ての公開メソッドは `Result<T, E>` を返す
- `anyhow::Context` で呼び出し元コンテキストを追加
- unwrap/expect は禁止（テストコードを除く）

## Testing Strategy

### 単体テスト

| コンポーネント | テスト対象 | カバレッジ目標 |
|---------------|-----------|---------------|
| TypeMappingService | 各方言の型変換 | 100% |
| MigrationPipeline | 各ステージの出力 | 90% |
| DatabaseIntrospector | SQLクエリ生成 | 80% |
| SchemaValidatorService | 各検証カテゴリ | 95% |
| DtoConverterService | ラウンドトリップ | 100% |

### 統合テスト

```rust
// tests/integration/migration_roundtrip.rs
#[tokio::test]
async fn test_export_import_roundtrip() {
    // 1. テストDBにスキーマを作成
    // 2. export でスキーマをエクスポート
    // 3. 新しいDBにインポート
    // 4. 両DBのスキーマが一致することを確認
}
```

### ゴールデンテスト

```rust
// tests/golden/migration_output.rs
#[test]
fn test_migration_sql_output_unchanged() {
    let diff = load_test_diff("add_table.json");
    let sql = MigrationGenerator::new()
        .generate_up_sql(&diff, Dialect::PostgreSQL)
        .unwrap();

    insta::assert_snapshot!(sql);
}
```

## Migration Plan

### Phase 1: 基盤整備（TypeMapping, DtoConverter）
1. `src/adapters/type_mapping.rs` を作成
2. `src/services/dto_converter.rs` を作成
3. 既存コードから呼び出しを段階的に移行
4. 重複コードを削除

### Phase 2: export 責務分離
1. `src/adapters/database_introspector.rs` を作成
2. `src/services/schema_conversion.rs` を作成
3. `export.rs` から introspection/変換ロジックを移行
4. テスト追加

### Phase 3: マイグレーションパイプライン統合
1. `MigrationPipeline` 構造体を追加
2. 既存メソッドをラッパーとして維持
3. 内部実装をパイプラインに移行
4. ゴールデンテストで出力互換性を確認

### Phase 4: バリデーション分割 & SQL安全化
1. `validate_internal` を関数分割
2. 外部APIは維持（後方互換性）
3. `DatabaseMigrator` のSQL組み立てをbind方式に変更
4. 全テスト実行で回帰確認

## Appendix

### 参考資料

- [research.md](./research.md) - 調査ログ
- [requirements.md](./requirements.md) - 要件定義
- [tech.md](../../steering/tech.md) - 技術スタック
- [structure.md](../../steering/structure.md) - プロジェクト構造
