# Design Document: extend-dialect-datatypes

**作成日**: 2026-01-22  
**仕様ID**: extend-dialect-datatypes  
**ステータス**: Draft

---

## 1. 概要

本設計書は、Stratumのデータベース方言サポートを拡張し、新しいデータ型（DECIMAL, FLOAT, DOUBLE, CHAR, DATE, TIME, BLOB, UUID, JSONB）を追加するための技術設計を定義する。

### 1.1 設計目標
- 既存の6種類のデータ型との後方互換性を維持
- 各データベース方言（PostgreSQL, MySQL, SQLite）への適切なマッピング
- 型固有のバリデーションルールの実装
- serdeによるYAMLシリアライズ/デシリアライズの自動対応

---

## 2. アーキテクチャ設計

### 2.1 コンポーネント構成

```
┌─────────────────────────────────────────────────────────────┐
│                     YAML Schema Files                        │
│                   (kind: DECIMAL, etc.)                      │
└─────────────────────┬───────────────────────────────────────┘
                      │ serde deserialization
                      ▼
┌─────────────────────────────────────────────────────────────┐
│              src/core/schema.rs                              │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  enum ColumnType                                     │    │
│  │  ├── INTEGER { precision }      [既存]              │    │
│  │  ├── VARCHAR { length }         [既存]              │    │
│  │  ├── TEXT                       [既存]              │    │
│  │  ├── BOOLEAN                    [既存]              │    │
│  │  ├── TIMESTAMP { with_time_zone } [既存]            │    │
│  │  ├── JSON                       [既存]              │    │
│  │  ├── DECIMAL { precision, scale } [新規: REQ-1]     │    │
│  │  ├── FLOAT                      [新規: REQ-2]       │    │
│  │  ├── DOUBLE                     [新規: REQ-2]       │    │
│  │  ├── CHAR { length }            [新規: REQ-3]       │    │
│  │  ├── DATE                       [新規: REQ-4]       │    │
│  │  ├── TIME { with_time_zone }    [新規: REQ-5]       │    │
│  │  ├── BLOB                       [新規: REQ-6]       │    │
│  │  ├── UUID                       [新規: REQ-7]       │    │
│  │  └── JSONB                      [新規: REQ-8]       │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────────────┘
                      │
        ┌─────────────┼─────────────┐
        ▼             ▼             ▼
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│  PostgreSQL   │ │    MySQL      │ │    SQLite     │
│  SqlGenerator │ │  SqlGenerator │ │  SqlGenerator │
└───────────────┘ └───────────────┘ └───────────────┘
        │             │             │
        └─────────────┼─────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│              src/services/schema_validator.rs                │
│                                                              │
│  validate_column_type()  [新規追加]                          │
│  ├── DECIMAL: precision >= scale                            │
│  ├── DECIMAL: precision <= 65 (MySQL) / 1000 (PostgreSQL)   │
│  └── CHAR: length <= 255                                    │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. 詳細設計

### 3.1 ColumnType Enum 拡張 (src/core/schema.rs)

#### 3.1.1 新規Variant定義

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ColumnType {
    // === 既存型（変更なし） ===
    INTEGER { precision: Option<u32> },
    VARCHAR { length: u32 },
    TEXT,
    BOOLEAN,
    TIMESTAMP { with_time_zone: Option<bool> },
    JSON,

    // === 新規型 ===
    
    /// 固定小数点数型 (REQ-1)
    DECIMAL {
        /// 全体の桁数 (1-65 for MySQL, 1-1000 for PostgreSQL)
        precision: u32,
        /// 小数点以下の桁数 (0 <= scale <= precision)
        scale: u32,
    },

    /// 単精度浮動小数点型 (REQ-2)
    FLOAT,

    /// 倍精度浮動小数点型 (REQ-2)
    DOUBLE,

    /// 固定長文字列型 (REQ-3)
    CHAR {
        /// 固定長 (1-255)
        length: u32,
    },

    /// 日付型 (REQ-4)
    DATE,

    /// 時刻型 (REQ-5)
    TIME {
        /// タイムゾーン付きかどうか (PostgreSQL only)
        with_time_zone: Option<bool>,
    },

    /// バイナリラージオブジェクト型 (REQ-6)
    BLOB,

    /// UUID型 (REQ-7)
    UUID,

    /// バイナリJSON型 (REQ-8, PostgreSQL専用)
    JSONB,
}
```

#### 3.1.2 to_sql_type() メソッド拡張

既存の `to_sql_type()` メソッドに新規型のマッピングを追加：

```rust
impl ColumnType {
    pub fn to_sql_type(&self, dialect: &Dialect) -> String {
        match (self, dialect) {
            // ... 既存のマッピング ...

            // DECIMAL
            (ColumnType::DECIMAL { precision, scale }, _) => {
                format!("DECIMAL({}, {})", precision, scale)
            }

            // FLOAT
            (ColumnType::FLOAT, Dialect::PostgreSQL) => "REAL".to_string(),
            (ColumnType::FLOAT, Dialect::MySQL) => "FLOAT".to_string(),
            (ColumnType::FLOAT, Dialect::SQLite) => "REAL".to_string(),

            // DOUBLE
            (ColumnType::DOUBLE, Dialect::PostgreSQL) => "DOUBLE PRECISION".to_string(),
            (ColumnType::DOUBLE, Dialect::MySQL) => "DOUBLE".to_string(),
            (ColumnType::DOUBLE, Dialect::SQLite) => "REAL".to_string(),

            // CHAR
            (ColumnType::CHAR { length }, Dialect::PostgreSQL | Dialect::MySQL) => {
                format!("CHAR({})", length)
            }
            (ColumnType::CHAR { .. }, Dialect::SQLite) => "TEXT".to_string(),

            // DATE
            (ColumnType::DATE, Dialect::PostgreSQL | Dialect::MySQL) => "DATE".to_string(),
            (ColumnType::DATE, Dialect::SQLite) => "TEXT".to_string(),

            // TIME
            (ColumnType::TIME { with_time_zone }, Dialect::PostgreSQL) => {
                if with_time_zone.unwrap_or(false) {
                    "TIME WITH TIME ZONE".to_string()
                } else {
                    "TIME".to_string()
                }
            }
            (ColumnType::TIME { .. }, Dialect::MySQL) => "TIME".to_string(),
            (ColumnType::TIME { .. }, Dialect::SQLite) => "TEXT".to_string(),

            // BLOB
            (ColumnType::BLOB, Dialect::PostgreSQL) => "BYTEA".to_string(),
            (ColumnType::BLOB, Dialect::MySQL | Dialect::SQLite) => "BLOB".to_string(),

            // UUID
            (ColumnType::UUID, Dialect::PostgreSQL) => "UUID".to_string(),
            (ColumnType::UUID, Dialect::MySQL) => "CHAR(36)".to_string(),
            (ColumnType::UUID, Dialect::SQLite) => "TEXT".to_string(),

            // JSONB
            (ColumnType::JSONB, Dialect::PostgreSQL) => "JSONB".to_string(),
            (ColumnType::JSONB, Dialect::MySQL) => "JSON".to_string(), // フォールバック
            (ColumnType::JSONB, Dialect::SQLite) => "TEXT".to_string(), // フォールバック
        }
    }
}
```

---

### 3.2 SQL Generator 拡張

各方言のSQLジェネレーターの `map_column_type()` メソッドを拡張する。

#### 3.2.1 PostgreSQL (src/adapters/sql_generator/postgres.rs)

```rust
fn map_column_type(&self, column_type: &ColumnType, auto_increment: Option<bool>) -> String {
    match column_type {
        // ... 既存のマッピング ...

        ColumnType::DECIMAL { precision, scale } => {
            format!("NUMERIC({}, {})", precision, scale)
        }
        ColumnType::FLOAT => "REAL".to_string(),
        ColumnType::DOUBLE => "DOUBLE PRECISION".to_string(),
        ColumnType::CHAR { length } => format!("CHAR({})", length),
        ColumnType::DATE => "DATE".to_string(),
        ColumnType::TIME { with_time_zone } => {
            if with_time_zone.unwrap_or(false) {
                "TIME WITH TIME ZONE".to_string()
            } else {
                "TIME".to_string()
            }
        }
        ColumnType::BLOB => "BYTEA".to_string(),
        ColumnType::UUID => "UUID".to_string(),
        ColumnType::JSONB => "JSONB".to_string(),
    }
}
```

#### 3.2.2 MySQL (src/adapters/sql_generator/mysql.rs)

```rust
fn map_column_type(&self, column_type: &ColumnType, _auto_increment: Option<bool>) -> String {
    match column_type {
        // ... 既存のマッピング ...

        ColumnType::DECIMAL { precision, scale } => {
            format!("DECIMAL({}, {})", precision, scale)
        }
        ColumnType::FLOAT => "FLOAT".to_string(),
        ColumnType::DOUBLE => "DOUBLE".to_string(),
        ColumnType::CHAR { length } => format!("CHAR({})", length),
        ColumnType::DATE => "DATE".to_string(),
        ColumnType::TIME { .. } => "TIME".to_string(), // MySQLはTIMEのタイムゾーン非サポート
        ColumnType::BLOB => "BLOB".to_string(),
        ColumnType::UUID => "CHAR(36)".to_string(), // MySQL 8.0未満との互換性
        ColumnType::JSONB => "JSON".to_string(), // JSONへフォールバック
    }
}
```

#### 3.2.3 SQLite (src/adapters/sql_generator/sqlite.rs)

```rust
fn map_column_type(&self, column_type: &ColumnType) -> String {
    match column_type {
        // ... 既存のマッピング ...

        ColumnType::DECIMAL { .. } => "TEXT".to_string(), // 精度保証のためTEXTを使用
        ColumnType::FLOAT | ColumnType::DOUBLE => "REAL".to_string(),
        ColumnType::CHAR { .. } => "TEXT".to_string(),
        ColumnType::DATE => "TEXT".to_string(), // ISO 8601形式
        ColumnType::TIME { .. } => "TEXT".to_string(), // ISO 8601形式
        ColumnType::BLOB => "BLOB".to_string(),
        ColumnType::UUID => "TEXT".to_string(),
        ColumnType::JSONB => "TEXT".to_string(), // TEXTへフォールバック
    }
}
```

---

### 3.3 バリデーション拡張 (src/services/schema_validator.rs)

#### 3.3.1 新規バリデーションメソッド

```rust
impl SchemaValidatorService {
    /// カラム型固有のバリデーションを実行
    fn validate_column_type(
        &self,
        column: &Column,
        table_name: &str,
        dialect: Option<&Dialect>,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        match &column.column_type {
            ColumnType::DECIMAL { precision, scale } => {
                // scale <= precision の検証
                if scale > precision {
                    errors.push(ValidationError::Constraint {
                        message: format!(
                            "DECIMAL scale ({}) cannot be greater than precision ({})",
                            scale, precision
                        ),
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column.name.clone()),
                            line: None,
                        }),
                        suggestion: Some("Ensure scale <= precision".to_string()),
                    });
                }

                // 方言固有の精度制限
                if let Some(d) = dialect {
                    let max_precision = match d {
                        Dialect::MySQL => 65,
                        Dialect::PostgreSQL => 1000,
                        Dialect::SQLite => u32::MAX, // SQLiteは制限なし（TEXTとして保存）
                    };
                    
                    if *precision > max_precision {
                        errors.push(ValidationError::Constraint {
                            message: format!(
                                "DECIMAL precision ({}) exceeds maximum for {:?} ({})",
                                precision, d, max_precision
                            ),
                            location: Some(ErrorLocation {
                                table: Some(table_name.to_string()),
                                column: Some(column.name.clone()),
                                line: None,
                            }),
                            suggestion: Some(format!(
                                "Use precision <= {}", max_precision
                            )),
                        });
                    }
                }
            }

            ColumnType::CHAR { length } => {
                // CHAR長の検証 (1-255)
                if *length == 0 || *length > 255 {
                    errors.push(ValidationError::Constraint {
                        message: format!(
                            "CHAR length ({}) must be between 1 and 255",
                            length
                        ),
                        location: Some(ErrorLocation {
                            table: Some(table_name.to_string()),
                            column: Some(column.name.clone()),
                            line: None,
                        }),
                        suggestion: Some("Use length between 1 and 255".to_string()),
                    });
                }
            }

            _ => {} // その他の型は追加のバリデーション不要
        }

        errors
    }

    /// 方言固有の警告を生成
    fn generate_dialect_warnings(
        &self,
        column: &Column,
        table_name: &str,
        dialect: &Dialect,
    ) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        match (&column.column_type, dialect) {
            // SQLiteでのDECIMAL精度喪失警告
            (ColumnType::DECIMAL { .. }, Dialect::SQLite) => {
                warnings.push(ValidationWarning {
                    message: format!(
                        "DECIMAL type in SQLite will be stored as TEXT; precision may be affected"
                    ),
                    location: Some(ErrorLocation {
                        table: Some(table_name.to_string()),
                        column: Some(column.name.clone()),
                        line: None,
                    }),
                });
            }

            // JSONB非サポート警告
            (ColumnType::JSONB, Dialect::MySQL) => {
                warnings.push(ValidationWarning {
                    message: "JSONB will fall back to JSON in MySQL".to_string(),
                    location: Some(ErrorLocation {
                        table: Some(table_name.to_string()),
                        column: Some(column.name.clone()),
                        line: None,
                    }),
                });
            }
            (ColumnType::JSONB, Dialect::SQLite) => {
                warnings.push(ValidationWarning {
                    message: "JSONB will fall back to TEXT in SQLite".to_string(),
                    location: Some(ErrorLocation {
                        table: Some(table_name.to_string()),
                        column: Some(column.name.clone()),
                        line: None,
                    }),
                });
            }

            // TIME WITH TIME ZONE非サポート警告
            (ColumnType::TIME { with_time_zone: Some(true) }, Dialect::MySQL) => {
                warnings.push(ValidationWarning {
                    message: "TIME WITH TIME ZONE is not supported in MySQL; time zone will be ignored".to_string(),
                    location: Some(ErrorLocation {
                        table: Some(table_name.to_string()),
                        column: Some(column.name.clone()),
                        line: None,
                    }),
                });
            }

            _ => {}
        }

        warnings
    }
}
```

#### 3.3.2 ValidationWarning 構造体追加

```rust
/// バリデーション警告
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub message: String,
    pub location: Option<ErrorLocation>,
}
```

---

### 3.4 YAMLスキーマ形式

#### 3.4.1 新規データ型のYAML記述例

```yaml
version: "1.0"
tables:
  products:
    name: products
    columns:
      # DECIMAL型
      - name: price
        type:
          kind: DECIMAL
          precision: 10
          scale: 2
        nullable: false

      # FLOAT型
      - name: weight
        type:
          kind: FLOAT
        nullable: true

      # DOUBLE型
      - name: coordinates
        type:
          kind: DOUBLE
        nullable: true

      # CHAR型
      - name: country_code
        type:
          kind: CHAR
          length: 2
        nullable: false

      # DATE型
      - name: birth_date
        type:
          kind: DATE
        nullable: true

      # TIME型
      - name: opening_time
        type:
          kind: TIME
          with_time_zone: false
        nullable: true

      # BLOB型
      - name: thumbnail
        type:
          kind: BLOB
        nullable: true

      # UUID型
      - name: external_id
        type:
          kind: UUID
        nullable: false

      # JSONB型
      - name: metadata
        type:
          kind: JSONB
        nullable: true
```

---

## 4. 型マッピング一覧

### 4.1 完全マッピング表

| ColumnType | PostgreSQL | MySQL | SQLite | 備考 |
|------------|------------|-------|--------|------|
| INTEGER | INTEGER/SERIAL | INT | INTEGER | 既存 |
| VARCHAR | VARCHAR(n) | VARCHAR(n) | TEXT | 既存 |
| TEXT | TEXT | TEXT | TEXT | 既存 |
| BOOLEAN | BOOLEAN | BOOLEAN | INTEGER | 既存 |
| TIMESTAMP | TIMESTAMP [WITH TZ] | TIMESTAMP | TEXT | 既存 |
| JSON | JSON | JSON | TEXT | 既存 |
| **DECIMAL** | NUMERIC(p,s) | DECIMAL(p,s) | TEXT | 新規 |
| **FLOAT** | REAL | FLOAT | REAL | 新規 |
| **DOUBLE** | DOUBLE PRECISION | DOUBLE | REAL | 新規 |
| **CHAR** | CHAR(n) | CHAR(n) | TEXT | 新規 |
| **DATE** | DATE | DATE | TEXT | 新規 |
| **TIME** | TIME [WITH TZ] | TIME | TEXT | 新規 |
| **BLOB** | BYTEA | BLOB | BLOB | 新規 |
| **UUID** | UUID | CHAR(36) | TEXT | 新規 |
| **JSONB** | JSONB | JSON | TEXT | 新規 |

---

## 5. テスト計画

### 5.1 ユニットテスト

| テストファイル | テスト内容 | テスト数 |
|---------------|-----------|---------|
| `tests/schema_model_test.rs` | 新規型のシリアライズ/デシリアライズ | 9件 |
| `tests/postgres_sql_generator_test.rs` | PostgreSQLマッピング | 9件 |
| `tests/mysql_sql_generator_test.rs` | MySQLマッピング | 9件 |
| `tests/sqlite_sql_generator_test.rs` | SQLiteマッピング | 9件 |
| `tests/schema_validator_test.rs` | バリデーションルール | 10件 |

### 5.2 統合テスト

- 既存YAMLスキーマファイルの後方互換性確認
- 新規データ型を含むYAMLファイルのパース確認
- マイグレーション生成の動作確認

---

## 6. 移行計画

### 6.1 後方互換性

- 既存の6種類のデータ型は変更なし
- 既存のYAMLスキーマファイルはそのまま動作
- 新規データ型は追加機能として提供

### 6.2 ドキュメント更新

- README.mdのデータ型一覧を更新
- 各データ型のYAML記述例を追加

---

## 7. リスクと軽減策

| リスク | 影響度 | 軽減策 |
|--------|--------|--------|
| SQLiteでのDECIMAL精度喪失 | Medium | 警告メッセージ出力、ドキュメント記載 |
| JSONB非サポート方言での動作 | Low | JSONへの自動フォールバック、警告出力 |
| 既存テストの破損 | High | 既存型のテストを先に実行して確認 |

---

## 8. 要件トレーサビリティ

| 要件ID | 設計セクション | 実装ファイル |
|--------|---------------|-------------|
| REQ-1 | 3.1, 3.2, 3.3 | schema.rs, *_sql_generator.rs, schema_validator.rs |
| REQ-2 | 3.1, 3.2 | schema.rs, *_sql_generator.rs |
| REQ-3 | 3.1, 3.2, 3.3 | schema.rs, *_sql_generator.rs, schema_validator.rs |
| REQ-4 | 3.1, 3.2 | schema.rs, *_sql_generator.rs |
| REQ-5 | 3.1, 3.2 | schema.rs, *_sql_generator.rs |
| REQ-6 | 3.1, 3.2 | schema.rs, *_sql_generator.rs |
| REQ-7 | 3.1, 3.2 | schema.rs, *_sql_generator.rs |
| REQ-8 | 3.1, 3.2, 3.3 | schema.rs, *_sql_generator.rs, schema_validator.rs |
| REQ-9 | 3.3 | schema_validator.rs |
| REQ-10 | 5.1, 5.2 | tests/*.rs |
