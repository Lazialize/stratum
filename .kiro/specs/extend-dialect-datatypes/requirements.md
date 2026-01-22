# Requirements Document

## Project Description (Input)
各データベース方言のサポートデータタイプを拡張する。

## Background Analysis

### 現状のサポート型
現在 `ColumnType` enum で定義されているデータ型は以下の6種類のみ：

| 型名 | PostgreSQL | MySQL | SQLite |
|------|------------|-------|--------|
| INTEGER | SMALLINT/INTEGER/BIGINT | SMALLINT/INT/BIGINT | INTEGER |
| VARCHAR | VARCHAR(n) | VARCHAR(n) | TEXT |
| TEXT | TEXT | TEXT | TEXT |
| BOOLEAN | BOOLEAN | BOOLEAN (TINYINT(1)) | INTEGER |
| TIMESTAMP | TIMESTAMP [WITH TIME ZONE] | TIMESTAMP | TEXT |
| JSON | JSON | JSON | TEXT |

### 不足している主要なデータ型
各データベースで広く使用されているが未サポートの型：

**数値型**
- DECIMAL/NUMERIC（精度・スケール指定可能な固定小数点）
- FLOAT/REAL/DOUBLE（浮動小数点）

**文字列型**
- CHAR（固定長文字列）

**日付・時刻型**
- DATE（日付のみ）
- TIME（時刻のみ）

**バイナリ型**
- BLOB/BYTEA/BINARY（バイナリデータ）

**その他**
- UUID（PostgreSQL/MySQL 8.0+）
- JSONB（PostgreSQLの最適化JSON）

---

## Requirements

### REQ-1: DECIMAL/NUMERIC型のサポート
**優先度**: High  
**説明**: 精度（precision）とスケール（scale）を指定可能な固定小数点数型を追加する。金額計算等で必須となる型。

**受け入れ条件**:
- [ ] `ColumnType::DECIMAL { precision: u32, scale: u32 }` を追加
- [ ] PostgreSQL: `DECIMAL(p, s)` または `NUMERIC(p, s)` にマッピング
- [ ] MySQL: `DECIMAL(p, s)` にマッピング
- [ ] SQLite: `REAL` または `TEXT` にマッピング（精度保証のため）
- [ ] YAMLスキーマで `kind: DECIMAL`, `precision`, `scale` フィールドが使用可能

---

### REQ-2: FLOAT/DOUBLE型のサポート
**優先度**: High  
**説明**: 浮動小数点数型を追加する。科学計算や概算値の保存に使用。

**受け入れ条件**:
- [ ] `ColumnType::FLOAT` を追加（単精度）
- [ ] `ColumnType::DOUBLE` を追加（倍精度）
- [ ] PostgreSQL: `REAL` / `DOUBLE PRECISION` にマッピング
- [ ] MySQL: `FLOAT` / `DOUBLE` にマッピング
- [ ] SQLite: `REAL` にマッピング
- [ ] YAMLスキーマで `kind: FLOAT`, `kind: DOUBLE` が使用可能

---

### REQ-3: CHAR型のサポート
**優先度**: Medium  
**説明**: 固定長文字列型を追加する。国コード等の固定長データに使用。

**受け入れ条件**:
- [ ] `ColumnType::CHAR { length: u32 }` を追加
- [ ] PostgreSQL: `CHAR(n)` にマッピング
- [ ] MySQL: `CHAR(n)` にマッピング
- [ ] SQLite: `TEXT` にマッピング
- [ ] YAMLスキーマで `kind: CHAR`, `length` フィールドが使用可能

---

### REQ-4: DATE型のサポート
**優先度**: High  
**説明**: 日付のみ（時刻なし）を保存する型を追加する。

**受け入れ条件**:
- [ ] `ColumnType::DATE` を追加
- [ ] PostgreSQL: `DATE` にマッピング
- [ ] MySQL: `DATE` にマッピング
- [ ] SQLite: `TEXT`（ISO 8601形式）にマッピング
- [ ] YAMLスキーマで `kind: DATE` が使用可能

---

### REQ-5: TIME型のサポート
**優先度**: Medium  
**説明**: 時刻のみ（日付なし）を保存する型を追加する。

**受け入れ条件**:
- [ ] `ColumnType::TIME { with_time_zone: Option<bool> }` を追加
- [ ] PostgreSQL: `TIME` / `TIME WITH TIME ZONE` にマッピング
- [ ] MySQL: `TIME` にマッピング
- [ ] SQLite: `TEXT` にマッピング
- [ ] YAMLスキーマで `kind: TIME`, `with_time_zone` が使用可能

---

### REQ-6: BLOB/BYTEA型のサポート
**優先度**: Medium  
**説明**: バイナリラージオブジェクト型を追加する。画像やファイルデータの保存に使用。

**受け入れ条件**:
- [ ] `ColumnType::BLOB` を追加
- [ ] PostgreSQL: `BYTEA` にマッピング
- [ ] MySQL: `BLOB` にマッピング
- [ ] SQLite: `BLOB` にマッピング
- [ ] YAMLスキーマで `kind: BLOB` が使用可能

---

### REQ-7: UUID型のサポート
**優先度**: Medium  
**説明**: UUID（Universally Unique Identifier）型を追加する。分散システムでの一意識別子として使用。

**受け入れ条件**:
- [ ] `ColumnType::UUID` を追加
- [ ] PostgreSQL: `UUID` にマッピング
- [ ] MySQL: `CHAR(36)` または `BINARY(16)` にマッピング（MySQL 8.0未満の互換性）
- [ ] SQLite: `TEXT` にマッピング
- [ ] YAMLスキーマで `kind: UUID` が使用可能

---

### REQ-8: JSONB型のサポート（PostgreSQL専用）
**優先度**: Low  
**説明**: PostgreSQLの最適化バイナリJSON型を追加する。JSONより高速なクエリが可能。

**受け入れ条件**:
- [ ] `ColumnType::JSONB` を追加
- [ ] PostgreSQL: `JSONB` にマッピング
- [ ] MySQL: `JSON` にフォールバック（警告出力）
- [ ] SQLite: `TEXT` にフォールバック（警告出力）
- [ ] YAMLスキーマで `kind: JSONB` が使用可能

---

### REQ-9: スキーマバリデーションの拡張
**優先度**: High  
**説明**: 新規追加された型に対するバリデーションルールを追加する。

**受け入れ条件**:
- [ ] DECIMAL: precision >= scale の検証
- [ ] DECIMAL: precision <= 65（MySQL制限）, precision <= 1000（PostgreSQL制限）の検証
- [ ] CHAR: length <= 255 の検証
- [ ] 方言固有の警告（例：SQLiteでのDECIMAL精度喪失）

---

### REQ-10: 既存テストの互換性維持
**優先度**: Critical  
**説明**: 既存のテストがすべてパスすることを保証する。

**受け入れ条件**:
- [ ] 既存の6種類の型の動作に変更なし
- [ ] 既存のYAMLスキーマファイルが正常にパース可能
- [ ] 既存のマイグレーションファイルとの互換性維持

---

## Out of Scope
以下は本仕様の対象外とする：

- ARRAY型（PostgreSQL）
- ENUM型（カスタム列挙型）
- Geometry/Geography型（PostGIS等の空間データ型）
- XML型
- 方言固有の特殊型（例：MySQL YEAR, SET, MEDIUMTEXT等）
- カスタム型/ドメイン型

---

## Technical Constraints
- Rust 1.92+との互換性を維持
- serde Serialize/Deserializeの実装が必須
- 後方互換性のあるYAMLスキーマ形式

---

## Dependencies
- `src/core/schema.rs`: `ColumnType` enum の拡張
- `src/adapters/sql_generator/*.rs`: 各方言のマッピング実装
- `src/services/schema_validator.rs`: バリデーションルール追加
- `src/services/schema_parser.rs`: YAMLパース対応
