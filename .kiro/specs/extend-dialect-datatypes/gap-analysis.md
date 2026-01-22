# Gap Analysis Report: extend-dialect-datatypes

**生成日時**: 2026-01-22  
**対象仕様**: extend-dialect-datatypes  
**分析対象**: 既存コードベース vs 要件定義

---

## Executive Summary

現在のコードベースは **6種類** のデータ型のみをサポートしています。要件で定義された **8種類** の新規データ型を追加するには、以下の主要ファイルの修正が必要です。

| カテゴリ | 現状 | 要件 | ギャップ |
|----------|------|------|----------|
| データ型定義 | 6種類 | 14種類 | +8種類 |
| バリデーションルール | 基本的 | 型固有 | 新規追加必要 |
| SQLジェネレーター | 既存型のみ | 全型対応 | 各方言で拡張必要 |

---

## 詳細ギャップ分析

### 1. ColumnType Enum (`src/core/schema.rs`)

**現状のサポート型**:
```rust
pub enum ColumnType {
    INTEGER { precision: Option<u32> },
    VARCHAR { length: u32 },
    TEXT,
    BOOLEAN,
    TIMESTAMP { with_time_zone: Option<bool> },
    JSON,
}
```

**追加が必要な型**:

| 要件ID | 型名 | フィールド | 実装難易度 |
|--------|------|-----------|-----------|
| REQ-1 | DECIMAL | precision: u32, scale: u32 | Medium |
| REQ-2 | FLOAT | - | Low |
| REQ-2 | DOUBLE | - | Low |
| REQ-3 | CHAR | length: u32 | Low |
| REQ-4 | DATE | - | Low |
| REQ-5 | TIME | with_time_zone: Option<bool> | Low |
| REQ-6 | BLOB | - | Low |
| REQ-7 | UUID | - | Low |
| REQ-8 | JSONB | - | Low |

**影響範囲**:
- `ColumnType::to_sql_type()` メソッドの拡張
- serde タグ付きenum のシリアライズ対応

---

### 2. SQL Generators (`src/adapters/sql_generator/`)

各ジェネレーターの `map_column_type()` メソッドの拡張が必要です。

#### PostgreSQL (`postgres.rs`)
**現状**: 43行目から定義、6型のマッピング  
**追加必要**:
| 型 | マッピング先 |
|----|-------------|
| DECIMAL | DECIMAL(p, s) / NUMERIC(p, s) |
| FLOAT | REAL |
| DOUBLE | DOUBLE PRECISION |
| CHAR | CHAR(n) |
| DATE | DATE |
| TIME | TIME [WITH TIME ZONE] |
| BLOB | BYTEA |
| UUID | UUID |
| JSONB | JSONB |

#### MySQL (`mysql.rs`)
**現状**: 46行目から定義、6型のマッピング  
**追加必要**:
| 型 | マッピング先 |
|----|-------------|
| DECIMAL | DECIMAL(p, s) |
| FLOAT | FLOAT |
| DOUBLE | DOUBLE |
| CHAR | CHAR(n) |
| DATE | DATE |
| TIME | TIME |
| BLOB | BLOB |
| UUID | CHAR(36) |
| JSONB | JSON (フォールバック) |

#### SQLite (`sqlite.rs`)
**現状**: 44行目から定義、6型のマッピング  
**追加必要**:
| 型 | マッピング先 | 注意事項 |
|----|-------------|----------|
| DECIMAL | TEXT | 精度保証のため |
| FLOAT | REAL | - |
| DOUBLE | REAL | - |
| CHAR | TEXT | - |
| DATE | TEXT | ISO 8601形式 |
| TIME | TEXT | ISO 8601形式 |
| BLOB | BLOB | - |
| UUID | TEXT | - |
| JSONB | TEXT (フォールバック) | - |

---

### 3. Schema Validator (`src/services/schema_validator.rs`)

**現状**: 基本的な構造検証のみ
- テーブルのカラム存在確認
- プライマリキー存在確認
- インデックス・制約のカラム参照確認
- 外部キー参照整合性

**追加が必要なバリデーション**:

| 要件ID | バリデーションルール | 優先度 |
|--------|---------------------|--------|
| REQ-9 | DECIMAL: precision >= scale | High |
| REQ-9 | DECIMAL: precision <= 65 (MySQL) / 1000 (PostgreSQL) | High |
| REQ-9 | CHAR: length <= 255 | Medium |
| REQ-9 | 方言固有の警告（SQLiteでのDECIMAL精度喪失等） | Medium |

**実装方針**: `validate()` メソッド内にデータ型固有の検証ロジックを追加

---

### 4. Schema Parser (`src/services/schema_parser.rs`)

**現状**: serde_saphyr を使用した YAML パース
- 新規データ型は `ColumnType` enum への variant 追加で自動対応
- `#[serde(tag = "kind")]` により kind フィールドで型を識別

**対応状況**: ✅ **自動対応可能**
- serdeのタグ付きenum機構により、新規型追加は ColumnType への variant 追加のみで対応

---

### 5. テストコード

**既存テストファイル**:
- `tests/schema_model_test.rs` - スキーマモデルのシリアライズ/デシリアライズ
- `tests/schema_parser_test.rs` - YAML パースのテスト
- `tests/postgres_sql_generator_test.rs` - PostgreSQL SQL生成
- `tests/mysql_sql_generator_test.rs` - MySQL SQL生成
- `tests/sqlite_sql_generator_test.rs` - SQLite SQL生成
- `tests/schema_validator_test.rs` - バリデーションテスト

**追加が必要なテスト**:
| テスト対象 | テストケース数（概算） |
|-----------|----------------------|
| 新規データ型のシリアライズ | 9件（各型1件） |
| PostgreSQL型マッピング | 9件 |
| MySQL型マッピング | 9件 |
| SQLite型マッピング | 9件 |
| DECIMAL バリデーション | 5件以上 |
| CHAR バリデーション | 3件以上 |

---

## リスク評価

### High Risk
1. **DECIMAL精度問題**: SQLiteでの精度喪失に関する適切な警告/ドキュメントが必要
2. **後方互換性**: 既存YAMLスキーマとの互換性維持が必須

### Medium Risk
1. **JSONB方言差異**: 非PostgreSQL環境でのフォールバック動作の明確化
2. **UUIDストレージ**: MySQLでのCHAR(36) vs BINARY(16)の選択

### Low Risk
1. **テスト追加量**: 約45件のテストケース追加が必要だが、パターン化可能

---

## 推奨実装順序

1. **Phase 1**: 基本数値型（DECIMAL, FLOAT, DOUBLE）- REQ-1, REQ-2
2. **Phase 2**: 日付時刻型（DATE, TIME）- REQ-4, REQ-5
3. **Phase 3**: 文字列・バイナリ型（CHAR, BLOB）- REQ-3, REQ-6
4. **Phase 4**: 特殊型（UUID, JSONB）- REQ-7, REQ-8
5. **Phase 5**: バリデーション拡張 - REQ-9
6. **Phase 6**: 互換性テスト - REQ-10

---

## 結論

要件の実装は **技術的に実現可能** であり、既存アーキテクチャとの整合性も高いです。主な作業は以下の通り：

- `ColumnType` enum への 9 variant 追加
- 3つのSQLジェネレーターの `map_column_type()` 拡張
- `SchemaValidatorService` への型固有バリデーション追加
- 約45件のユニットテスト追加

**推定工数**: 中規模（設計・実装・テストで約2-3日）
