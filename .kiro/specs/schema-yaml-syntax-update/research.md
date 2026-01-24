# Research & Design Decisions

## Summary
- **Feature**: schema-yaml-syntax-update
- **Discovery Scope**: Extension（既存システムの拡張）
- **Key Findings**:
  - 現在の`Table`構造体は`name`フィールドを持ち、YAML直接デシリアライズで使用
  - `serde`の`#[serde(default)]`属性でオプショナルフィールドを実現可能
  - DTO（Data Transfer Object）パターンでYAML構造と内部モデルを分離する設計が最適

## Research Log

### serdeによるオプショナルフィールドの実装方法
- **Context**: `indexes`と`constraints`フィールドをオプショナル化するための実装方法調査
- **Sources Consulted**: serde公式ドキュメント、Rustコミュニティのベストプラクティス
- **Findings**:
  - `#[serde(default)]`属性で未定義フィールドをデフォルト値で初期化可能
  - `Vec<T>`のデフォルトは空ベクター`Vec::new()`
  - `Option<T>`のデフォルトは`None`
- **Implications**: 既存の`Table`構造体に`#[serde(default)]`を追加するだけで対応可能

### HashMapキーからの値抽出
- **Context**: YAMLの`tables`キー名をテーブル名として使用する方法
- **Sources Consulted**: serde-saphyr、serde公式ドキュメント
- **Findings**:
  - `HashMap<String, TableDto>`でデシリアライズ後、変換処理でキー名を`Table.name`に設定
  - カスタムデシリアライザより、DTO→内部モデル変換の方がシンプル
- **Implications**: DTOパターンを採用し、`parse_schema_file`メソッド内で変換処理を実装

### 主キーの独立フィールド化
- **Context**: `primary_key`フィールドを`constraints`から分離する設計
- **Sources Consulted**: 既存コードベース分析
- **Findings**:
  - 現在は`Constraint::PRIMARY_KEY`として制約リストに含まれる
  - 内部モデルは変更せず、YAML DTOでのみ`primary_key`フィールドを定義
  - パース時に`primary_key`を`Constraint::PRIMARY_KEY`に変換
- **Implications**: 内部モデルへの影響を最小化し、SQL Generatorの変更不要

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| DTOパターン | YAML用DTO構造体を定義し、内部モデルに変換 | 関心分離、内部モデル変更不要、テスト容易 | 変換ロジックの追加コスト | **採用** |
| 直接serde属性追加 | 既存構造体にserde属性を追加 | 変更最小 | YAML構造が内部モデルに強く結合 | 不採用 |
| カスタムDeserializer | serde Visitorパターンで独自実装 | 完全な制御 | 複雑、保守性低下 | 不採用 |

## Design Decisions

### Decision: DTOパターンの採用
- **Context**: YAML構文変更を内部モデルに影響なく実装する必要性
- **Alternatives Considered**:
  1. 既存`Table`構造体に直接serde属性追加 — YAML構造と内部モデルが密結合
  2. カスタムDeserializer — 実装複雑、保守性低下
- **Selected Approach**: YAML専用のDTO構造体（`SchemaDto`, `TableDto`）を定義し、パース後に内部モデルに変換
- **Rationale**:
  - 内部モデル（`Schema`, `Table`, `Constraint`）は変更不要
  - SQL Generator、Validator、Diff Detectorへの影響なし
  - YAML構文の将来変更にも柔軟に対応可能
- **Trade-offs**: 変換ロジックの追加コストが発生するが、保守性向上のメリットが上回る
- **Follow-up**: パフォーマンステストで変換オーバーヘッドを確認

### Decision: constraintsからPRIMARY_KEYを除外
- **Context**: `primary_key`フィールドの独立化に伴う`constraints`の扱い
- **Alternatives Considered**:
  1. `constraints`内にPRIMARY_KEYを残す（新旧両対応）
  2. `constraints`からPRIMARY_KEYを完全除外
- **Selected Approach**: YAMLの`constraints`フィールドからは`PRIMARY_KEY`タイプを除外し、`primary_key`フィールドのみで定義
- **Rationale**: ユーザーがまだいないため後方互換性不要、シンプルな構文を優先
- **Trade-offs**: 既存構文との互換性なし
- **Follow-up**: 例示ファイル（example/schema/）の更新

## Risks & Mitigations
- **リスク1**: 変換ロジックのバグ — 単体テストで網羅的にカバー
- **リスク2**: パフォーマンス低下 — DTO変換は軽量なため実質的影響なし
- **リスク3**: 例示ファイルの不整合 — 実装完了後に全例示ファイルを新構文に更新

## References
- [serde Field Attributes](https://serde.rs/field-attrs.html) — `#[serde(default)]`の使用方法
- 既存コード: `src/core/schema.rs` — 現在のスキーマモデル定義
- 既存コード: `src/services/schema_parser.rs` — 現在のパーサー実装
