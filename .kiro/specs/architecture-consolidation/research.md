# Research Log

## Summary

本リサーチでは、architecture-consolidation リファクタリングの設計フェーズに必要な調査を実施した。主な調査対象は以下の通り:

1. 方言依存型マッピングの現状と重複箇所
2. マイグレーション生成パイプラインの構造
3. export コマンドの責務分布
4. バリデーションロジックの構成
5. SQL組み立てパターンの安全性
6. DTO変換の整合性

調査の結果、Option C（ハイブリッドアプローチ）が最適であることを確認した。

## Research Log

### Topic 1: 方言依存型マッピングの重複分析

**調査内容:** 各方言の型マッピングロジックの分散状況を特定

**発見事項:**

| ファイル | 関数/メソッド | 行数 | 役割 |
|---------|-------------|------|------|
| [sqlite.rs](src/adapters/sql_generator/sqlite.rs#L46-L93) | `map_column_type` | ~50行 | ColumnType → SQL型文字列 |
| [postgres.rs](src/adapters/sql_generator/postgres.rs) | `map_column_type` | ~50行 | ColumnType → SQL型文字列 |
| [mysql.rs](src/adapters/sql_generator/mysql.rs) | `map_column_type` | ~50行 | ColumnType → SQL型文字列 |
| [export.rs](src/cli/commands/export.rs#L309-L453) | `parse_sqlite_type`, `parse_postgres_type`, `parse_mysql_type` | ~150行 | SQL型文字列 → ColumnType（逆変換） |

**影響:**
- 新しい型を追加する際、4箇所以上の修正が必要
- 双方向変換のロジックが分散しており、ラウンドトリップ整合性の保証が困難

**推奨:**
- `src/adapters/type_mapping.rs` に共通化
- trait `TypeMapper` で方言固有のフックを提供

### Topic 2: マイグレーション生成の二重経路

**調査内容:** `generate_up_sql` と `generate_up_sql_with_schemas` の差分分析

**発見事項:**
- [migration_generator.rs:89-165](src/services/migration_generator.rs#L89-L165) - `generate_up_sql`: スキーマ参照なし
- [migration_generator.rs:378-505](src/services/migration_generator.rs#L378-L505) - `generate_up_sql_with_schemas`: 型変更検証付き

重複コード:
```
- SqlGenerator の取得（lines 93-97 と 414-418）
- enum ステートメント生成（lines 99-101 と 420-422）
- テーブルソート（lines 103-104 と 424-425）
- CREATE TABLE / INDEX 生成（lines 107-128 と 427-450）
```

**影響:**
- バグ修正を2箇所で行う必要がある
- 機能追加時に整合性維持が困難

**推奨:**
- 共通パイプライン + フック方式への統合
- `generate_up_sql` は `generate_up_sql_with_schemas` のシンプルラッパーに

### Topic 3: export コマンドの責務集中

**調査内容:** [export.rs](src/cli/commands/export.rs) の責務分析

**発見事項:**
現在の `ExportCommandHandler` が担う責務:
1. **DB接続・設定読み込み** (lines 56-78)
2. **スキーマ情報抽出** (lines 112-160) - INFORMATION_SCHEMA/PRAGMA クエリ
3. **型パース** (lines 309-453) - 各方言固有のパースロジック
4. **YAMLシリアライズ** (lines 89-93) - `SchemaSerializerService` 呼び出し
5. **出力処理** (lines 96-109) - ファイル/標準出力

**影響:**
- 単体テストが困難（DB接続が必須）
- 型パースロジックが他で再利用不可

**推奨:**
- **adapters層:** `DatabaseIntrospector` trait + 方言実装
- **services層:** `SchemaConversionService`（型変換）
- **cli層:** `ExportCommandHandler`（出力のみ）

### Topic 4: バリデーション単一関数の巨大化

**調査内容:** [schema_validator.rs](src/services/schema_validator.rs) の構造分析

**発見事項:**
`validate_internal` 関数（lines 47-273）が以下を全て処理:
- ENUM検証 (lines 50-89)
- テーブル存在確認 (lines 91-94)
- カラム型検証 (lines 108-135)
- プライマリキー検証 (lines 137-149)
- インデックス参照検証 (lines 151-172)
- 制約参照検証 (lines 174-269)

**影響:**
- 226行の単一関数（保守困難）
- 特定カテゴリのみのテストが困難

**推奨:**
- カテゴリ別に分割:
  - `validate_enums()`
  - `validate_column_types()`
  - `validate_constraints()`
  - `validate_references()`
  - `generate_warnings()`

### Topic 5: SQL文字列組み立ての安全性

**調査内容:** [database_migrator.rs](src/adapters/database_migrator.rs) のSQL組み立てパターン

**発見事項:**

危険なパターン（lines 96-103, 143-147）:
```rust
fn generate_record_migration_sql(&self, ...) -> String {
    format!(
        "INSERT INTO {} (version, name, checksum, dialect, applied_at) VALUES ('{}', '{}', '{}', '{}', datetime('now'))",
        self.migration_table_name, version, name, checksum, dialect
    )
}
```

**リスク:**
- `name` パラメータにユーザー入力が含まれる可能性
- SQLインジェクションの潜在的リスク

**調査: sqlx::query! の利用可否**

AnyPool での `sqlx::query!` マクロ:
- **結論:** コンパイル時検証は単一方言のみサポート
- AnyPool使用時は `sqlx::query()` + bind を使用する必要がある
- CI環境では各方言のDBコンテナを用意してマクロ検証可能

**推奨:**
```rust
sqlx::query(&format!(
    "INSERT INTO {} (version, name, checksum, dialect, applied_at) VALUES (?, ?, ?, ?, datetime('now'))",
    self.migration_table_name
))
.bind(version)
.bind(name)
.bind(checksum)
.bind(dialect)
```

### Topic 6: DTO変換の分散

**調査内容:** Schema ↔ YAML の変換ロジック分析

**発見事項:**

| 方向 | ファイル | 主要メソッド |
|------|---------|-------------|
| YAML → Schema | [schema_parser.rs](src/services/schema_parser.rs) | `parse_schema_file`, `convert_dto_to_schema` |
| Schema → YAML | [schema_serializer.rs](src/services/schema_serializer.rs) | `serialize_to_string`, `convert_schema_to_dto` |

ラウンドトリップテスト:
- [schema_serializer.rs:439-493](src/services/schema_serializer.rs#L439-L493) で実装済み

**リスク:**
- 片側のみの変更でラウンドトリップが壊れる可能性
- 新フィールド追加時に両方の変更が必要

**推奨:**
- `src/services/dto_converter.rs` に双方向変換を集約
- 単一の `DtoConverter` trait:
  ```rust
  trait DtoConverter<T, D> {
      fn to_dto(&self, model: &T) -> D;
      fn from_dto(&self, dto: &D) -> Result<T>;
  }
  ```

## Architecture Decisions

### ADR-1: 型マッピング共通化の配置層

**決定:** adapters 層に配置

**理由:**
- 型マッピングはDB方言に依存する技術的関心事
- services 層は DB 非依存であるべき
- SqlGenerator と同じ層に配置することで依存方向が一貫

### ADR-2: マイグレーションパイプラインの統合方式

**決定:** 共通パイプライン + フック方式

**パイプライン構成:**
1. `prepare()` - SqlGenerator 取得、事前検証
2. `generate_enum_statements()` - ENUM処理（PostgreSQLフック）
3. `generate_table_statements()` - CREATE/ALTER TABLE
4. `generate_constraint_statements()` - 制約追加
5. `finalize()` - SQL結合、後処理

**理由:**
- 各ステップでの拡張ポイントが明確
- 方言固有ロジックをフックで注入可能
- 既存出力との互換性維持が容易

### ADR-3: バリデーション分割粒度

**決定:** カテゴリ別関数 + 統合エントリポイント

```rust
impl SchemaValidatorService {
    pub fn validate(&self, schema: &Schema) -> ValidationResult {
        let mut result = ValidationResult::new();
        result.merge(self.validate_enums(schema));
        result.merge(self.validate_column_types(schema));
        result.merge(self.validate_constraints(schema));
        result.merge(self.validate_references(schema));
        result
    }

    fn validate_enums(&self, schema: &Schema) -> ValidationResult { ... }
    fn validate_column_types(&self, schema: &Schema) -> ValidationResult { ... }
    // ...
}
```

**理由:**
- 各検証カテゴリの独立テストが可能
- 外部APIは変更なし（後方互換性維持）
- 将来的な並列化も可能

## Risks & Mitigations

| リスク | 影響度 | 緩和策 |
|--------|-------|--------|
| ラウンドトリップ整合性の破壊 | High | プロパティベーステストの追加、CI必須化 |
| マイグレーションSQL出力の変更 | High | ゴールデンテスト追加（既存出力との比較） |
| パフォーマンス劣化 | Low | ベンチマーク追加、変更前後の比較 |
| テスト不足による回帰 | Medium | カバレッジ目標設定（80%以上） |

## Open Questions

1. **SQLite再作成時のフォールバック値:** 共通化の範囲をどこまでにするか
   - 現状: `sqlite_table_recreator.rs` に閉じ込め
   - 選択肢: 共通化 vs 方言固有のまま維持

2. **エラーメッセージの国際化:** 将来的な多言語対応の必要性
   - 現状: ハードコードされた英語メッセージ
   - 影響: バリデーション分割時に検討可能
