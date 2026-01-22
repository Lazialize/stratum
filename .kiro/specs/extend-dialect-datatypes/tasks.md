# Tasks Document: extend-dialect-datatypes

**作成日**: 2026-01-22  
**仕様ID**: extend-dialect-datatypes  
**総タスク数**: 18件

---

## タスク概要

| フェーズ | タスク数 | 推定時間 |
|----------|---------|---------|
| Phase 1: コアモデル拡張 | 3件 | 2時間 |
| Phase 2: SQLジェネレーター拡張 | 3件 | 2時間 |
| Phase 3: バリデーション実装 | 2件 | 1.5時間 |
| Phase 4: テスト実装 | 6件 | 3時間 |
| Phase 5: ドキュメント更新 | 2件 | 1時間 |
| Phase 6: 統合・検証 | 2件 | 1時間 |
| **合計** | **18件** | **10.5時間** |

---

## Phase 1: コアモデル拡張

### Task 1.1: ColumnType enum に新規バリアント追加 ✅
**要件**: REQ-1, REQ-2, REQ-3, REQ-4, REQ-5, REQ-6, REQ-7, REQ-8  
**ファイル**: `src/core/schema.rs`  
**優先度**: Critical  
**推定時間**: 45分  
**完了日**: 2026-01-22

**作業内容**:
- [x] `ColumnType` enum に以下のバリアントを追加:
  - `DECIMAL { precision: u32, scale: u32 }`
  - `FLOAT`
  - `DOUBLE`
  - `CHAR { length: u32 }`
  - `DATE`
  - `TIME { with_time_zone: Option<bool> }`
  - `BLOB`
  - `UUID`
  - `JSONB`
- [x] 各バリアントにドキュメントコメント追加
- [x] serde属性の確認（既存の `#[serde(tag = "kind")]` で自動対応）

**受け入れ条件**:
- [x] コンパイルが成功する
- [x] 既存の6種類の型に影響がない

---

### Task 1.2: to_sql_type() メソッドの拡張 ✅
**要件**: REQ-1 ~ REQ-8  
**ファイル**: `src/core/schema.rs`  
**優先度**: High  
**推定時間**: 30分  
**完了日**: 2026-01-22

**作業内容**:
- [x] `ColumnType::to_sql_type()` メソッドに新規型のマッピングを追加
- [x] 各方言（PostgreSQL, MySQL, SQLite）への適切なマッピング実装
- [x] マッピング表:
  | 型 | PostgreSQL | MySQL | SQLite |
  |---|---|---|---|
  | DECIMAL | NUMERIC(p,s) | DECIMAL(p,s) | TEXT |
  | FLOAT | REAL | FLOAT | REAL |
  | DOUBLE | DOUBLE PRECISION | DOUBLE | REAL |
  | CHAR | CHAR(n) | CHAR(n) | TEXT |
  | DATE | DATE | DATE | TEXT |
  | TIME | TIME [WITH TZ] | TIME | TEXT |
  | BLOB | BYTEA | BLOB | BLOB |
  | UUID | UUID | CHAR(36) | TEXT |
  | JSONB | JSONB | JSON | TEXT |

**受け入れ条件**:
- [x] すべての新規型が正しくマッピングされる
- [x] 既存型のマッピングに変更がない

---

### Task 1.3: ColumnType のユニットテスト追加 ✅
**要件**: REQ-10  
**ファイル**: `src/core/schema.rs` (mod tests)  
**優先度**: High  
**推定時間**: 30分  
**完了日**: 2026-01-22

**作業内容**:
- [x] 新規バリアントの基本テスト追加
- [x] シリアライズ/デシリアライズテスト追加
- [x] `to_sql_type()` のテスト追加

**受け入れ条件**:
- [x] `cargo test` でスキーマ関連テストがすべてパス（45個のテストが成功）

---

## Phase 2: SQLジェネレーター拡張

### Task 2.1: PostgreSQL SQLジェネレーター拡張 ✅
**要件**: REQ-1 ~ REQ-8  
**ファイル**: `src/adapters/sql_generator/postgres.rs`  
**優先度**: High  
**推定時間**: 40分  
**完了日**: 2026-01-22

**作業内容**:
- [x] `map_column_type()` メソッドに新規型のマッピング追加:
  - DECIMAL → `NUMERIC(p, s)`
  - FLOAT → `REAL`
  - DOUBLE → `DOUBLE PRECISION`
  - CHAR → `CHAR(n)`
  - DATE → `DATE`
  - TIME → `TIME` / `TIME WITH TIME ZONE`
  - BLOB → `BYTEA`
  - UUID → `UUID`
  - JSONB → `JSONB`
- [x] 既存のmatchアームの後に新規型を追加

**受け入れ条件**:
- [x] コンパイルが成功する
- [x] 既存のPostgreSQL生成に影響がない（16個のテストが成功）

---

### Task 2.2: MySQL SQLジェネレーター拡張 ✅
**要件**: REQ-1 ~ REQ-8  
**ファイル**: `src/adapters/sql_generator/mysql.rs`  
**優先度**: High  
**推定時間**: 40分  
**完了日**: 2026-01-22

**作業内容**:
- [x] `map_column_type()` メソッドに新規型のマッピング追加:
  - DECIMAL → `DECIMAL(p, s)`
  - FLOAT → `FLOAT`
  - DOUBLE → `DOUBLE`
  - CHAR → `CHAR(n)`
  - DATE → `DATE`
  - TIME → `TIME`（タイムゾーン無視）
  - BLOB → `BLOB`
  - UUID → `CHAR(36)`
  - JSONB → `JSON`（フォールバック）

**受け入れ条件**:
- [x] コンパイルが成功する
- [x] 既存のMySQL生成に影響がない（17個のテストが成功）

---

### Task 2.3: SQLite SQLジェネレーター拡張 ✅
**要件**: REQ-1 ~ REQ-8  
**ファイル**: `src/adapters/sql_generator/sqlite.rs`  
**優先度**: High  
**推定時間**: 40分  
**完了日**: 2026-01-22

**作業内容**:
- [x] `map_column_type()` メソッドに新規型のマッピング追加:
  - DECIMAL → `TEXT`（精度保証のため）
  - FLOAT → `REAL`
  - DOUBLE → `REAL`
  - CHAR → `TEXT`
  - DATE → `TEXT`
  - TIME → `TEXT`
  - BLOB → `BLOB`
  - UUID → `TEXT`
  - JSONB → `TEXT`（フォールバック）

**受け入れ条件**:
- [x] コンパイルが成功する
- [x] 既存のSQLite生成に影響がない（17個のテストが成功）

---

## Phase 3: バリデーション実装

### Task 3.1: 型固有バリデーションルール実装 ✅
**要件**: REQ-9  
**ファイル**: `src/services/schema_validator.rs`  
**優先度**: High  
**推定時間**: 45分  
**完了日**: 2026-01-22

**作業内容**:
- [x] `validate_column_type()` メソッドを新規追加
- [x] DECIMALバリデーション実装:
  - `scale <= precision` の検証
  - `precision <= 65`（MySQL）/ `precision <= 1000`（PostgreSQL）の検証
- [x] CHARバリデーション実装:
  - `length >= 1 && length <= 255` の検証
- [x] `validate()` メソッドから `validate_column_type()` を呼び出し

**受け入れ条件**:
- [x] 不正なDECIMAL定義でエラーが返される（テスト実装済み）
- [x] 不正なCHAR定義でエラーが返される（テスト実装済み）

---

### Task 3.2: 方言固有警告機能実装 ✅
**要件**: REQ-9  
**ファイル**: `src/services/schema_validator.rs`, `src/core/error.rs`  
**優先度**: Medium  
**推定時間**: 45分  
**完了日**: 2026-01-22

**作業内容**:
- [x] `ValidationWarning` 構造体を追加（`src/core/error.rs`）
- [x] `ValidationResult` に `warnings` フィールドと `add_warning()` メソッドを追加
- [x] `generate_dialect_warnings()` メソッドを新規追加
- [x] 方言固有の警告ロジック実装:
  - MySQL: UUID→CHAR(36), JSONB→JSON
  - SQLite: DECIMAL→TEXT, UUID→TEXT, JSONB→TEXT, DATE→TEXT
  - MySQL/SQLite: TIME WITH TIME ZONEのタイムゾーン情報損失

**受け入れ条件**:
- [x] 方言ごとに適切な警告が生成される（テスト実装済み）
- [x] 警告はエラーとは異なる扱い（ValidationResult.is_valid()に影響しない）
**ファイル**: `src/services/schema_validator.rs`, `src/core/error.rs`  
**優先度**: Medium  
**推定時間**: 45分

**作業内容**:
- [ ] `ValidationWarning` 構造体を追加（`src/core/error.rs`）
- [ ] `ValidationResult` に警告リストを追加
- [ ] `generate_dialect_warnings()` メソッドを実装:
  - SQLiteでのDECIMAL精度喪失警告
  - MySQL/SQLiteでのJSONBフォールバック警告
  - MySQLでのTIME WITH TIME ZONE無視警告

**受け入れ条件**:
- [ ] 方言固有の警告が適切に生成される
- [ ] 警告はエラーとして扱われない

---

## Phase 4: テスト実装

### Task 4.1: スキーマモデルテスト追加
**要件**: REQ-10  
**ファイル**: `tests/schema_model_test.rs`  
**優先度**: High  
**推定時間**: 30分

**作業内容**:
- [ ] 新規型のシリアライズテスト（9件）:
  - `test_decimal_type_serialization`
  - `test_float_type_serialization`
  - `test_double_type_serialization`
  - `test_char_type_serialization`
  - `test_date_type_serialization`
  - `test_time_type_serialization`
  - `test_blob_type_serialization`
  - `test_uuid_type_serialization`
  - `test_jsonb_type_serialization`

**受け入れ条件**:
- [ ] すべてのテストがパス

---

### Task 4.2: PostgreSQL SQLジェネレーターテスト追加
**要件**: REQ-10  
**ファイル**: `tests/postgres_sql_generator_test.rs`  
**優先度**: High  
**推定時間**: 30分

**作業内容**:
- [ ] 新規型のマッピングテスト（9件）:
  - `test_map_column_type_decimal`
  - `test_map_column_type_float`
  - `test_map_column_type_double`
  - `test_map_column_type_char`
  - `test_map_column_type_date`
  - `test_map_column_type_time`
  - `test_map_column_type_blob`
  - `test_map_column_type_uuid`
  - `test_map_column_type_jsonb`

**受け入れ条件**:
- [ ] すべてのテストがパス

---

### Task 4.3: MySQL SQLジェネレーターテスト追加
**要件**: REQ-10  
**ファイル**: `tests/mysql_sql_generator_test.rs`  
**優先度**: High  
**推定時間**: 30分

**作業内容**:
- [ ] 新規型のマッピングテスト（9件）
- [ ] UUIDのCHAR(36)マッピング確認
- [ ] JSONBのJSONフォールバック確認

**受け入れ条件**:
- [ ] すべてのテストがパス

---

### Task 4.4: SQLite SQLジェネレーターテスト追加
**要件**: REQ-10  
**ファイル**: `tests/sqlite_sql_generator_test.rs`  
**優先度**: High  
**推定時間**: 30分

**作業内容**:
- [ ] 新規型のマッピングテスト（9件）
- [ ] DECIMALのTEXTマッピング確認
- [ ] FLOAT/DOUBLEの両方がREALになることを確認

**受け入れ条件**:
- [ ] すべてのテストがパス

---

### Task 4.5: バリデーターテスト追加
**要件**: REQ-9, REQ-10  
**ファイル**: `tests/schema_validator_test.rs`  
**優先度**: High  
**推定時間**: 30分

**作業内容**:
- [ ] DECIMALバリデーションテスト:
  - `test_decimal_scale_exceeds_precision`
  - `test_decimal_precision_exceeds_mysql_limit`
  - `test_decimal_valid`
- [ ] CHARバリデーションテスト:
  - `test_char_length_zero`
  - `test_char_length_exceeds_limit`
  - `test_char_valid`
- [ ] 警告テスト:
  - `test_sqlite_decimal_warning`
  - `test_jsonb_fallback_warning`

**受け入れ条件**:
- [ ] すべてのテストがパス

---

### Task 4.6: 既存テストの実行確認
**要件**: REQ-10  
**ファイル**: 全テストファイル  
**優先度**: Critical  
**推定時間**: 15分

**作業内容**:
- [ ] `cargo test` を実行して全テストがパスすることを確認
- [ ] 既存のYAMLスキーマファイル（`example/schema/users.yaml`）のパース確認
- [ ] リグレッションがないことを確認

**受け入れ条件**:
- [ ] 既存テストがすべてパス
- [ ] 新規テストがすべてパス

---

## Phase 5: ドキュメント更新

### Task 5.1: README.md のデータ型一覧更新
**要件**: N/A（ドキュメント）  
**ファイル**: `README.md`  
**優先度**: Medium  
**推定時間**: 30分

**作業内容**:
- [ ] サポートデータ型の一覧表を更新
- [ ] 新規データ型のYAML記述例を追加
- [ ] 各型の方言別マッピング表を追加

**受け入れ条件**:
- [ ] ドキュメントが最新の状態

---

### Task 5.2: サンプルスキーマファイル作成
**要件**: N/A（サンプル）  
**ファイル**: `example/schema/products.yaml`（新規）  
**優先度**: Low  
**推定時間**: 30分

**作業内容**:
- [ ] 新規データ型を使用したサンプルスキーマファイル作成
- [ ] 全9種類の新規データ型の使用例を含める

**受け入れ条件**:
- [ ] サンプルファイルが正常にパースできる

---

## Phase 6: 統合・検証

### Task 6.1: 全体統合テスト
**要件**: REQ-10  
**ファイル**: N/A  
**優先度**: Critical  
**推定時間**: 30分

**作業内容**:
- [ ] `cargo build --release` が成功することを確認
- [ ] `cargo test` がすべてパスすることを確認
- [ ] `cargo clippy` で警告がないことを確認
- [ ] 新規スキーマファイルでマイグレーション生成テスト

**受け入れ条件**:
- [ ] ビルド成功
- [ ] 全テストパス
- [ ] Clippy警告なし

---

### Task 6.2: 最終レビュー・マージ準備
**要件**: すべて  
**ファイル**: N/A  
**優先度**: Critical  
**推定時間**: 30分

**作業内容**:
- [ ] コード変更の最終レビュー
- [ ] 要件トレーサビリティの確認
- [ ] CHANGELOG.md の更新
- [ ] マージ準備完了

**受け入れ条件**:
- [ ] すべての要件が満たされている
- [ ] ドキュメントが更新されている
- [ ] リリース準備完了

---

## タスク依存関係

```
Phase 1 (コアモデル)
  ├── Task 1.1 ──┬──→ Task 1.2 ──→ Task 1.3
  │              │
  │              ▼
Phase 2 (SQLジェネレーター)
  │   ├── Task 2.1 (PostgreSQL)
  │   ├── Task 2.2 (MySQL)
  │   └── Task 2.3 (SQLite)
  │              │
  │              ▼
Phase 3 (バリデーション)
  │   ├── Task 3.1 ──→ Task 3.2
  │              │
  │              ▼
Phase 4 (テスト)
  │   ├── Task 4.1 ~ 4.5 (並列実行可能)
  │   └── Task 4.6 (全テスト実行)
  │              │
  │              ▼
Phase 5 (ドキュメント)
  │   ├── Task 5.1
  │   └── Task 5.2
  │              │
  │              ▼
Phase 6 (統合・検証)
      ├── Task 6.1 ──→ Task 6.2
```

---

## 進捗トラッキング

| Task ID | タスク名 | 状態 | 完了日 |
|---------|----------|------|--------|
| 1.1 | ColumnType enum 拡張 | ⬜ Not Started | - |
| 1.2 | to_sql_type() 拡張 | ⬜ Not Started | - |
| 1.3 | ColumnType テスト | ⬜ Not Started | - |
| 2.1 | PostgreSQL ジェネレーター | ⬜ Not Started | - |
| 2.2 | MySQL ジェネレーター | ⬜ Not Started | - |
| 2.3 | SQLite ジェネレーター | ⬜ Not Started | - |
| 3.1 | 型固有バリデーション | ⬜ Not Started | - |
| 3.2 | 方言固有警告 | ⬜ Not Started | - |
| 4.1 | スキーマモデルテスト | ⬜ Not Started | - |
| 4.2 | PostgreSQL テスト | ⬜ Not Started | - |
| 4.3 | MySQL テスト | ⬜ Not Started | - |
| 4.4 | SQLite テスト | ⬜ Not Started | - |
| 4.5 | バリデーターテスト | ⬜ Not Started | - |
| 4.6 | 既存テスト確認 | ⬜ Not Started | - |
| 5.1 | README 更新 | ⬜ Not Started | - |
| 5.2 | サンプルスキーマ | ⬜ Not Started | - |
| 6.1 | 統合テスト | ⬜ Not Started | - |
| 6.2 | 最終レビュー | ⬜ Not Started | - |
