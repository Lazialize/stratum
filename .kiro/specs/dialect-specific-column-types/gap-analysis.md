# Gap Analysis: dialect-specific-column-types

**作成日**: 2026-01-22T09:29:14Z
**仕様ID**: dialect-specific-column-types
**分析対象**: 既存コードベースと要件の実装ギャップ

---

## 1. 現状調査 (Current State Investigation)

### 1.1 関連資産

#### 主要コンポーネント

**コアドメイン** ([src/core/schema.rs](src/core/schema.rs))
- `ColumnType` enum: 共通データ型を15種類定義（`#[serde(tag = "kind")]`でタグ付き）
- `to_sql_type(&Dialect) -> String`: 方言ごとのSQL型文字列へ変換するメソッド
- パターン: 共通抽象化層 → 各方言へのマッピング

**スキーマパーサー** ([src/services/schema_parser.rs](src/services/schema_parser.rs))
- `serde-saphyr` を使用したYAMLデシリアライゼーション（パニックフリー）
- `parse_schema_file()`: YAML → `Schema` オブジェクトへの自動変換
- パターン: Serdeによる宣言的デシリアライゼーション

**スキーマバリデーター** ([src/services/schema_validator.rs](src/services/schema_validator.rs))
- `validate_column_type()`: 型パラメータの妥当性検証（DECIMAL, CHARのみ）
- `generate_dialect_warnings()`: 方言固有の警告生成（フォールバック型への警告）
- パターン: ハードコードされたバリデーションルール

**SQLジェネレーター** ([src/adapters/sql_generator/](src/adapters/sql_generator/))
- `SqlGenerator` トレイト: PostgreSQL/MySQL/SQLite用の3実装
- `map_column_type()`: `ColumnType` → 方言固有SQL型文字列
- パターン: 共通型を方言固有SQL文字列に変換

**エラー処理** ([src/core/error.rs](src/core/error.rs))
- `ValidationError`: Syntax, Reference, Constraint の3種類
- `ErrorLocation`: table/column/line番号による位置情報
- `ValidationWarning`: DialectSpecific, PrecisionLoss, Compatibility

#### 依存関係

- **serde/serde-saphyr**: YAML/JSONシリアライゼーション（現在利用中）
- **JSON Schema検証**: 未使用（新規導入が必要）

### 1.2 既存の慣習とパターン

**型システムアーキテクチャ**
- 単一の `ColumnType` enum で全方言を抽象化
- `to_sql_type()` メソッドで実行時に方言別変換
- Serdeの `#[serde(tag = "kind")]` による Tagged Union

**バリデーション戦略**
- ハードコードされたバリデーションロジック（`validate_column_type()`内）
- 方言固有の制約はコード内に直接記述（例: DECIMAL precision <= 65）

**エラーメッセージパターン**
- `ValidationError` のmessageフィールドに詳細情報を含める
- `suggestion` フィールドで修正案を提示
- `ErrorLocation` で発生位置を特定

### 1.3 統合ポイント

- YAMLスキーマファイル → `SchemaParserService` → `ColumnType` enum
- `SchemaValidatorService` による検証
- `SqlGenerator` 実装による方言別SQL生成

---

## 2. 要件実現性分析 (Requirements Feasibility)

### 2.1 技術的要求事項

#### Requirement 1: 方言固有カラム型の定義

**必要な機能**:
- 方言固有の `kind` 値を受け入れる拡張された `ColumnType` enum
- 方言ごとに異なる型定義を許可する仕組み（例: `type_postgresql`, `type_mysql`, `type_sqlite`）
- 方言固有パラメータのシリアライズ/デシリアライズ

**現状のギャップ**:
- ❌ 方言固有バリアントが未定義（例: PostgreSQLの `SERIAL`, `INT2`, MySQLの `TINYINT`, `ENUM`）
- ❌ 複数方言対応の型定義構造が未実装（`type_{dialect}` パターン）
- ✅ Serdeによる自動シリアライゼーションは既存パターンで対応可能

#### Requirement 2: JSON Schemaによる型検証

**必要な機能**:
- 各方言用のJSON Schemaファイル（`postgres-types.schema.json`, etc.）
- JSON Schema検証ライブラリの統合
- 方言別の型名・パラメータ検証ロジック

**現状のギャップ**:
- ❌ JSON Schema検証機能が未実装
- ❌ 方言別のスキーマ定義ファイルが存在しない
- **Research Needed**: JSON Schema検証ライブラリの選定と統合方法

#### Requirement 3: 後方互換性の維持

**必要な機能**:
- 既存の共通型（`INTEGER`, `VARCHAR`, etc.）の継続サポート
- 共通型と方言固有型の混在スキーマのサポート

**現状のギャップ**:
- ✅ 既存の共通型定義は維持可能
- ⚠️ 共通型と方言固有型の共存戦略が必要（Option A vs Option B で検討）

#### Requirement 4: SQL生成ロジックの最適化

**必要な機能**:
- 方言固有 `kind` をそのままSQL DDL文に出力
- 方言ごとに最適化されたup/down SQL生成

**現状のギャップ**:
- ✅ `SqlGenerator` トレイトの既存構造は流用可能
- ⚠️ 方言固有型を受け取った場合のSQL生成ロジック追加が必要

#### Requirement 5: エラーメッセージの改善

**必要な機能**:
- 不正な型名に対する詳細なエラーメッセージ（利用可能な型のリスト含む）
- ファイルパスと行番号の表示
- 複数エラーの一括収集と表示

**現状のギャップ**:
- ✅ `ErrorLocation` による位置情報表示は既存
- ⚠️ YAML行番号の取得方法が課題（`serde-saphyr` の制約）
- ✅ 複数エラーの収集は `ValidationResult` で実装済み

#### Requirement 6: ドキュメントとサンプルの提供

**必要な機能**:
- 方言別型リファレンスドキュメント
- サンプルスキーマファイル（`postgres_advanced.yaml`, etc.）
- マイグレーションガイド

**現状のギャップ**:
- ❌ 方言固有型のドキュメントが未作成
- ✅ `example/schema/` ディレクトリの既存構造は流用可能

### 2.2 制約と未知事項

**アーキテクチャ上の制約**:
- 既存の共通型システムとの共存が必要
- Serdeによるデシリアライゼーションの制約（Tagged Unionパターン）

**未知事項（Research Needed）**:
1. JSON Schema検証ライブラリ（`jsonschema` crate）の統合方法と性能影響
2. YAML行番号の取得方法（`serde-saphyr` の制約下での実現可能性）
3. 複数方言対応型定義の具体的なYAML構造設計

---

## 3. 実装アプローチオプション

### Option A: 方言固有バリアント拡張（Extend Existing ColumnType）

**概要**: 既存の `ColumnType` enum に方言固有バリアントを追加

**変更対象ファイル**:
- [src/core/schema.rs](src/core/schema.rs): `ColumnType` enum拡張
- [src/adapters/sql_generator/*.rs](src/adapters/sql_generator/): 新規バリアントへの対応
- [src/services/schema_validator.rs](src/services/schema_validator.rs): JSON Schema検証ロジック追加

**拡張内容**:
```rust
pub enum ColumnType {
    // 既存の共通型（後方互換性維持）
    INTEGER { precision: Option<u32> },
    VARCHAR { length: u32 },
    // ...

    // PostgreSQL固有型
    #[serde(rename = "SERIAL")]
    PostgresSerial,
    #[serde(rename = "INT2")]
    PostgresInt2,
    #[serde(rename = "VARBIT")]
    PostgresVarbit { length: Option<u32> },

    // MySQL固有型
    #[serde(rename = "TINYINT")]
    MysqlTinyint { unsigned: Option<bool> },
    #[serde(rename = "ENUM")]
    MysqlEnum { values: Vec<String> },

    // SQLite固有型（特殊なケースは少ない）
}
```

**互換性評価**:
- ✅ 既存の共通型は変更なし（後方互換性維持）
- ✅ Serdeの Tagged Union パターンを継続利用
- ⚠️ 方言固有型の数が多い場合、enum が肥大化

**複雑性と保守性**:
- ⚠️ PostgreSQL: 30+ 型, MySQL: 25+ 型 → enum が非常に大きくなる
- ⚠️ 各SQL生成実装で全バリアントへのマッチ対応が必要
- ✅ 単一の型システムで管理できる

**Trade-offs**:
- ✅ 既存パターンの延長で実装可能
- ✅ 型安全性が高い（コンパイル時チェック）
- ❌ enum肥大化によるコード可読性低下
- ❌ 新しい方言型追加時の影響範囲が広い

---

### Option B: 方言別型定義構造（Create New Dialect-Specific Type System）

**概要**: `ColumnType` を共通型と方言固有型の2層構造に分離

**新規作成コンポーネント**:
- [src/core/dialect_types.rs](src/core/dialect_types.rs): 方言固有型定義
- [src/services/type_resolver.rs](src/services/type_resolver.rs): 型解決サービス
- [src/core/validation/json_schema_validator.rs](src/core/validation/json_schema_validator.rs): JSON Schema検証

**構造設計**:
```rust
// src/core/schema.rs
pub struct Column {
    pub name: String,
    #[serde(flatten)]
    pub column_type: ColumnTypeDefinition,
    // ...
}

// src/core/dialect_types.rs
pub enum ColumnTypeDefinition {
    /// 共通型（後方互換性維持）
    Common(ColumnType),
    /// 方言固有型
    DialectSpecific {
        postgres: Option<PostgresType>,
        mysql: Option<MysqlType>,
        sqlite: Option<SqliteType>,
    },
}

pub struct PostgresType {
    pub kind: String, // "SERIAL", "INT2", etc.
    pub params: serde_json::Value, // 型パラメータ
}
```

**統合ポイント**:
- `SchemaParserService`: `ColumnTypeDefinition` のデシリアライゼーション
- `TypeResolverService`: 実行時に対象方言の型を解決
- `SqlGenerator`: 解決済み型からSQL生成

**責任分界**:
- `ColumnTypeDefinition`: 型定義の保持（共通型 or 方言固有型）
- `TypeResolverService`: 方言選択と型解決
- `JsonSchemaValidator`: JSON Schemaによる型検証

**Trade-offs**:
- ✅ 方言型の追加が容易（JSON Schemaファイル更新のみ）
- ✅ 型定義と検証ロジックの分離
- ✅ 拡張性が高い
- ❌ 新規ファイル/モジュールの追加が必要
- ❌ 型安全性が低下（実行時検証に依存）
- ❌ 複雑な型解決ロジックが必要

---

### Option C: ハイブリッドアプローチ（Hybrid Approach）

**概要**: Option A（共通型維持）+ Option B（方言固有型を追加構造で対応）

**段階的実装戦略**:

**Phase 1: 基盤整備**
- JSON Schema検証基盤の導入（`jsonschema` crate）
- 方言別スキーマファイル作成（`postgres-types.schema.json`, etc.）
- `ColumnType` に `DialectSpecific` バリアント追加

**Phase 2: 方言固有型サポート**
```rust
pub enum ColumnType {
    // 既存の共通型（後方互換性維持）
    INTEGER { precision: Option<u32> },
    // ...

    // 新規: 方言固有型エントリーポイント
    DialectSpecific {
        dialect: Dialect,
        kind: String,
        params: serde_json::Value,
    },
}
```

**Phase 3: リファクタリング（オプション）**
- 使用頻度の高い方言固有型を専用バリアントに昇格（例: `PostgresSerial`）

**リスク軽減策**:
- フェーズ1で検証基盤を確立してから方言固有型を追加
- 既存テストの継続動作を保証
- マイグレーションガイド提供

**Trade-offs**:
- ✅ 段階的移行でリスク低減
- ✅ 初期フェーズで最小限の変更
- ✅ 後方互換性を完全維持
- ❌ 最終的な型システムの設計が複雑化する可能性
- ⚠️ Phase 2以降の実装方針の早期決定が必要

---

## 4. 実装複雑性とリスク評価

### 4.1 工数見積もり

**Option A: 方言固有バリアント拡張**
- サイズ: **XL (2+ weeks)**
- 根拠: PostgreSQL/MySQL/SQLiteの全型をバリアントとして定義（50+ variants）、SQL生成ロジックの全面的な拡張が必要

**Option B: 方言別型定義構造**
- サイズ: **L (1-2 weeks)**
- 根拠: 新規モジュール作成、JSON Schema検証統合、型解決ロジック実装

**Option C: ハイブリッドアプローチ**
- Phase 1: **M (3-7 days)** - JSON Schema基盤導入
- Phase 2: **M (3-7 days)** - `DialectSpecific` バリアント実装
- 合計: **L (1-2 weeks)**
- 根拠: 段階的実装により各フェーズが明確、既存パターンの活用が可能

### 4.2 リスク評価

**Option A: High Risk**
- 理由: enum肥大化による保守性低下、全SQL生成実装への影響範囲の広さ
- 軽減策: 型のグループ化、マクロによるボイラープレート削減

**Option B: Medium Risk**
- 理由: 新規アーキテクチャパターンの導入、実行時型検証への依存
- 軽減策: JSON Schema検証の徹底、統合テストの充実

**Option C: Medium Risk**
- 理由: 段階的移行による複雑性、Phase 2以降の設計判断が必要
- 軽減策: Phase 1完了後にPhase 2の設計レビュー実施

---

## 5. 推奨事項 (Recommendations)

### 5.1 推奨アプローチ

**Option C: ハイブリッドアプローチ** を推奨

**理由**:
1. **段階的リスク管理**: Phase 1でJSON Schema検証基盤を確立し、Phase 2で方言固有型を追加
2. **後方互換性の完全保証**: 既存の共通型システムを維持しつつ拡張
3. **拡張性**: `DialectSpecific` バリアント + JSON Schemaで新しい方言型の追加が容易
4. **実装コスト**: Option Aより少ない変更で実現可能

### 5.2 設計フェーズで決定すべき事項

1. **複数方言対応のYAML構造**
   - `type_postgresql` / `type_mysql` / `type_sqlite` フィールドの採用可否
   - デフォルト方言の選択ロジック

2. **JSON Schemaファイルの管理方法**
   - バイナリへの埋め込み vs 外部ファイル
   - スキーマバージョニング戦略

3. **型解決の実行タイミング**
   - パース時 vs SQL生成時
   - パフォーマンスへの影響

4. **エラーメッセージの詳細度**
   - 利用可能な型リストの生成方法（JSON Schemaから自動抽出 vs 手動管理）
   - YAML行番号取得の実装可能性

### 5.3 設計フェーズで実施すべきリサーチ

1. **JSON Schema検証ライブラリの技術調査**
   - `jsonschema` crate (0.26+) の統合方法
   - パフォーマンスベンチマーク（大規模スキーマファイルでの検証時間）
   - カスタムエラーメッセージの実装方法

2. **YAML行番号取得の実現可能性**
   - `serde-saphyr` のAPI調査
   - 代替パーサー（`serde_yaml` v0.9+）の検討

3. **方言型定義の網羅性調査**
   - PostgreSQL 17.x, MySQL 8.x, SQLite 3.x の全型リスト作成
   - よく使われる型の優先順位付け

---

## 6. 要件とコンポーネントのマッピング

| 要件 | 既存コンポーネント | ギャップ | アプローチ |
|------|-------------------|---------|-----------|
| **Req 1.1**: 方言固有`kind`受け入れ | `ColumnType` enum | ❌ Missing | Option C: `DialectSpecific` バリアント追加 |
| **Req 1.2**: 方言専用バリアント | `ColumnType` enum | ❌ Missing | Option C: `DialectSpecific` + JSON Schema |
| **Req 1.3**: 複数方言対応構造 | `Column` struct | ❌ Missing | 設計フェーズで構造決定 |
| **Req 1.4**: シリアライゼーション | Serde (`serde-saphyr`) | ✅ Reusable | 既存パターン流用 |
| **Req 2.1**: JSON Schemaファイル保持 | - | ❌ Missing | 新規作成: `resources/schemas/` |
| **Req 2.2**: JSON Schema検証 | - | ❌ Missing | `jsonschema` crate統合 |
| **Req 2.3**: 不正型検出エラー | `ValidationError` | ⚠️ Constraint | エラーメッセージ拡張 |
| **Req 2.4**: パラメータ違反検出 | `validate_column_type()` | ⚠️ Constraint | JSON Schema検証に移行 |
| **Req 2.5**: 拡張性（新規型追加） | - | ❌ Missing | JSON Schemaファイル更新のみで対応 |
| **Req 3.1**: 共通型の継続サポート | `ColumnType` enum | ✅ Reusable | 既存バリアント維持 |
| **Req 3.2**: 混在スキーマのサポート | `SchemaParserService` | ⚠️ Constraint | 型解決ロジック追加 |
| **Req 3.3**: デフォルト共通型解釈 | `ColumnType` enum | ✅ Reusable | 既存動作維持 |
| **Req 4.1**: 方言固有型のSQL出力 | `SqlGenerator` trait | ⚠️ Constraint | `DialectSpecific` 対応追加 |
| **Req 4.2**: 方言別up/down SQL | `MigrationGenerator` | ✅ Reusable | 既存ロジック流用 |
| **Req 4.3**: 型変換なしでSQL出力 | `map_column_type()` | ⚠️ Constraint | `DialectSpecific` 分岐追加 |
| **Req 4.4**: 未サポート型の警告 | `generate_dialect_warnings()` | ⚠️ Constraint | 検証ロジック拡張 |
| **Req 5.1**: 詳細エラーメッセージ | `ValidationError` | ⚠️ Constraint | JSON Schema由来のエラー変換 |
| **Req 5.2**: パラメータエラー詳細 | `ValidationError` | ⚠️ Constraint | JSON Schema検証結果の変換 |
| **Req 5.3**: 適切な終了コード | CLI commands | ✅ Reusable | 既存エラーハンドリング流用 |
| **Req 5.4**: 複数エラー一括表示 | `ValidationResult` | ✅ Reusable | 既存実装流用 |
| **Req 6.1**: 型リファレンス | - | ❌ Missing | 新規ドキュメント作成 |
| **Req 6.2**: サンプルスキーマ | `example/schema/` | ⚠️ Constraint | 新規ファイル追加 |
| **Req 6.3**: マイグレーションガイド | - | ❌ Missing | 新規ドキュメント作成 |
| **Req 6.4**: トラブルシューティング | - | ❌ Missing | 新規ドキュメント作成 |

**凡例**:
- ✅ **Reusable**: 既存コンポーネントを流用可能
- ⚠️ **Constraint**: 既存コンポーネントの拡張が必要
- ❌ **Missing**: 新規作成が必要

---

## 7. 参考資料

### JSON Schema検証ライブラリ調査結果

**推奨ライブラリ**: `jsonschema` crate (v0.26+)

**選定理由**:
- Rust製で最も成熟したJSON Schema検証ライブラリ
- serde_json との完全統合
- 非同期/同期両対応
- Draft 4, 6, 7, 2019-09, 2020-12 サポート

**基本的な使用例**:
```rust
use serde_json::json;
let schema = json!({"type": "string", "maxLength": 5});
let instance = json!("foo");
let validator = jsonschema::validator_for(&schema)?;
assert!(validator.is_valid(&instance));
```

**参考リンク**:
- [jsonschema - Rust](https://docs.rs/jsonschema/latest/jsonschema/)
- [jsonschema - crates.io: Rust Package Registry](https://crates.io/crates/jsonschema)
- [GitHub - Stranger6667/jsonschema](https://github.com/Stranger6667/jsonschema)

---

## 8. まとめ

**実装ギャップの概要**:
- 方言固有型のサポートは新規機能であり、既存の共通型システムとの共存が課題
- JSON Schema検証基盤の導入が必要（新規依存関係）
- 後方互換性維持が最重要制約

**推奨実装戦略**:
- **Option C: ハイブリッドアプローチ** による段階的実装
- Phase 1でJSON Schema検証基盤を確立
- Phase 2で `DialectSpecific` バリアントを追加

**次フェーズで決定すべき設計事項**:
1. 複数方言対応のYAML構造設計
2. JSON Schemaファイル管理戦略
3. 型解決の実行タイミング
4. エラーメッセージの詳細度

**技術的リスク**: Medium
**実装工数**: L (1-2 weeks)
