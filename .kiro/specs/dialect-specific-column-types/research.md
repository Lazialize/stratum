# Research & Design Decisions: dialect-specific-column-types

---
**作成日**: 2026-01-22T09:54:21Z
**仕様ID**: dialect-specific-column-types
**フェーズ**: 設計

---

## Summary

- **Feature**: `dialect-specific-column-types`
- **Discovery Scope**: Extension（既存の共通型システムへのシンプルな拡張）
- **Key Findings**:
  - **シンプルアプローチを採用**: `DialectSpecific` バリアント + データベース側での検証
  - **JSON Schema検証は不要**: Stratum内部では検証せず、IDE補完用リソースとして提供
  - **検証の責務分離**: データベースエンジンの正確なエラーメッセージを活用
  - Serdeの Tagged Union (`#[serde(tag = "kind")]`) を維持しつつ拡張

## Research Log

### アプローチの決定: JSON Schema検証の必要性

- **Context**: 当初、方言固有型の検証をJSON Schemaで実装する計画だったが、ユーザーとの対話で方針を再考
- **Sources Consulted**: ユーザーからのフィードバック、既存の設計原則（「データベースエンジン側の処理に委ねる」）
- **Findings**:
  - **JSON Schema検証の目的**:
    - 型名の妥当性チェック（`SERIAL`がPostgreSQLで有効か）
    - パラメータの検証（`ENUM`に`values`が指定されているか）
  - **代替案の検討**:
    1. **JSON Schema検証あり**: エラーを早期検出、`jsonschema` crate依存
    2. **検証なし（シンプル）**: データベースに委譲、依存関係なし
    3. **軽量な検証**: 警告のみ、エラーは許可
  - **ユーザーの意図**: YAML入力時のIDE補完・バリデーション用にJSON Schemaを提供、Stratum内部では検証しない
- **Implications**:
  - **Stratum内部**: 検証なし、`DialectSpecific`バリアントをそのままSQL出力
  - **IDE補完**: YAML Schemaファイル（JSON Schema形式）をリソースとして提供
  - **エラー検出**: データベース実行時に型エラーを検出、エラーメッセージをそのまま伝達
  - **依存関係**: `jsonschema` crate不要、シンプルな実装

### YAML Schema（JSON Schema形式）の提供

- **Context**: IDE補完用のリソースとしてYAML Schemaを提供する方法
- **Sources Consulted**:
  - [JSON Schema Draft 2020-12](https://json-schema.org/draft/2020-12)
  - VSCode YAML拡張の設定方法
- **Findings**:
  - YAML Schemaは JSON Schema形式で定義
  - VSCodeの `yaml.schemas` 設定で自動補完を有効化
  - `oneOf` パターンで方言固有型の候補を定義
  - `description` フィールドで型の説明を提供
- **Implications**:
  - YAML Schemaファイルを `resources/schemas/` に配置
  - ユーザーはVSCode等のIDEでYAML編集時に自動補完を利用
  - Stratum内部ではYAML Schemaを使用しない（IDE専用）

### Serdeのenum表現パターン

- **Context**: 既存の `#[serde(tag = "kind")]` パターンと新しい `DialectSpecific` バリアントの統合方法
- **Sources Consulted**:
  - [Enum representations · Serde](https://serde.rs/enum-representations.html)
  - [Container attributes · Serde](https://serde.rs/container-attrs.html)
- **Findings**:
  - **Internally tagged** (`#[serde(tag = "kind")]`): フィールド名で型を識別（現在の実装）
  - **Untagged** (`#[serde(untagged)]`): タグなし、順序でマッチング
  - `#[serde(flatten)]` on enum variantsはサポートされない
- **Implications**:
  - 既存の `#[serde(tag = "kind")]` パターンを維持
  - `DialectSpecific` バリアントは `#[serde(untagged)]` でタグなしデシリアライゼーション
  - `params: serde_json::Value` でパラメータを柔軟に保持

### 方言固有型のSQL生成

- **Context**: `DialectSpecific` バリアントの `kind` をどのようにSQL DDL文に出力するか
- **Sources Consulted**: 既存の `SqlGenerator` 実装パターン
- **Findings**:
  - 既存の共通型は `to_sql_type()` メソッドで方言別変換
  - `DialectSpecific` バリアントは `format_dialect_specific_type()` メソッドで柔軟にフォーマット
  - パラメータがある場合の処理例:
    - `ENUM(values)` → `ENUM('a', 'b', 'c')`
    - `VARBIT(length)` → `VARBIT(16)`
- **Implications**:
  - `SqlGenerator` 実装に `format_dialect_specific_type()` メソッドを追加
  - `kind` をそのままSQL出力（型変換なし）
  - データベースエラーはそのままユーザーに伝達

### PostgreSQL/MySQL/SQLite型の網羅性調査

- **Context**: 各方言でサポートすべき型の優先順位付け
- **Sources Consulted**:
  - PostgreSQL 17.x公式ドキュメント
  - MySQL 8.x公式ドキュメント
  - SQLite 3.x公式ドキュメント
- **Findings**:
  - **PostgreSQL**: 優先度高い型
    - `SERIAL`, `BIGSERIAL`, `SMALLSERIAL` (自動増分)
    - `INT2`, `INT4`, `INT8` (明示的な整数サイズ)
    - `VARBIT(n)` (可変長ビット列)
    - `INET`, `CIDR` (IPアドレス)
    - `ARRAY` (配列型)
  - **MySQL**: 優先度高い型
    - `TINYINT`, `MEDIUMINT` (小さい整数)
    - `ENUM(values)` (列挙型)
    - `SET(values)` (セット型)
    - `YEAR` (年型)
  - **SQLite**: 優先度高い型
    - `INTEGER PRIMARY KEY` (ROWID別名)
    - その他はほぼ共通型で対応可能
- **Implications**:
  - YAML Schemaファイルに頻度の高い型を優先的に定義
  - ユーザーフィードバックで追加型を検討

## Design Decisions

### Decision: シンプルアプローチの採用（JSON Schema検証なし）

- **Context**: 方言固有型の検証をStratum内部で行うか、データベース側に委譲するか
- **Alternatives Considered**:
  1. JSON Schema検証あり: `jsonschema` crateで型名・パラメータを検証
  2. 検証なし（シンプル）: データベースに委譲、依存関係なし
  3. 軽量な検証: 警告のみ、エラーは許可
- **Selected Approach**: 検証なし（シンプルアプローチ）
  - Stratum内部では `DialectSpecific` バリアントの検証をスキップ
  - データベース実行時に型エラーを検出
  - データベースの正確なエラーメッセージをそのまま伝達
- **Rationale**:
  - **シンプルさ**: `jsonschema` crate不要、依存関係削減
  - **責務の明確化**: 型検証はデータベースの責務、Stratumは橋渡し役
  - **正確なエラーメッセージ**: データベースエンジンの詳細なエラーメッセージ（`HINT`含む）を活用
  - **要件の整合性**: 「データベースエンジン側の処理に委ね、Stratum側では型変換や内部展開を行わない」
- **Trade-offs**:
  - **Benefits**: シンプルな実装、依存関係なし、データベースの正確なエラーメッセージ
  - **Compromises**: マイグレーション生成前にエラーを検出できない（データベース実行時に検出）
- **Follow-up**: IDE補完でタイプミスを軽減（YAML Schema提供）

### Decision: YAML Schemaファイルの提供（IDE補完用）

- **Context**: ユーザーのYAML記述をサポートする方法
- **Alternatives Considered**:
  1. Stratum内部でJSON Schema検証
  2. IDE補完用のYAML Schemaファイル提供
  3. ドキュメントのみ提供（補完なし）
- **Selected Approach**: YAML Schemaファイル提供（IDE補完用）
  - JSON Schema形式でYAML Schemaを定義
  - `resources/schemas/` に配置
  - VSCode等のIDEで自動補完を有効化
- **Rationale**:
  - **DX向上**: YAML記述時の型候補とパラメータを自動表示
  - **タイプミス軽減**: IDE補完でエラーを未然に防ぐ
  - **Stratum実装のシンプル化**: IDE側で補完、Stratum内部では未使用
- **Trade-offs**:
  - **Benefits**: DX向上、タイプミス軽減、Stratum実装のシンプル化
  - **Compromises**: YAML Schemaの保守が必要、IDE設定が必要
- **Follow-up**: IDE設定ガイドの作成、YAML Schemaの継続更新

### Decision: `DialectSpecific` バリアントの構造

- **Context**: 方言固有型のデータ構造設計
- **Alternatives Considered**:
  1. `DialectSpecific { dialect: Dialect, kind: String, params: serde_json::Value }`
  2. `DialectSpecific { kind: String, params: HashMap<String, serde_json::Value> }`
  3. 方言ごとに個別のバリアント（`PostgresSerial`, `MysqlEnum`）
- **Selected Approach**: `DialectSpecific { kind: String, params: serde_json::Value }`
  - `dialect` フィールドは不要（実行時に方言を指定）
  - `kind`: 型名（例: "SERIAL", "ENUM"）
  - `params`: 型パラメータ（`serde_json::Value` で柔軟に保持）
- **Rationale**:
  - **シンプルさ**: 方言の指定は実行時（`stratum generate --dialect postgres`）
  - **柔軟性**: `serde_json::Value` で任意のパラメータ構造をサポート
  - **拡張性**: 新しい型の追加が容易
- **Trade-offs**:
  - **Benefits**: シンプル、柔軟、拡張性が高い
  - **Compromises**: 型安全性は実行時検証に依存（データベース側）
- **Follow-up**: なし

## Risks & Mitigations

- **Risk 1: タイプミスによる型エラーがデータベース実行時に検出される** — 提案された軽減策: IDE補完（YAML Schema）でタイプミスを未然に防ぐ、ドキュメントで型リファレンス提供
- **Risk 2: データベースエラーメッセージの可読性** — 提案された軽減策: データベースの詳細なエラーメッセージ（`HINT`含む）をそのまま伝達、Stratumは加工しない
- **Risk 3: YAML Schemaの保守コスト** — 提案された軽減策: 型追加時のチェックリスト化、リリースノートへの記載
- **Risk 4: 方言固有型の網羅性不足** — 提案された軽減策: ユーザーフィードバックで追加型を検討、YAML Schemaの継続更新

## References

- [JSON Schema Draft 2020-12](https://json-schema.org/draft/2020-12) — JSON Schemaの最新仕様
- [Enum representations · Serde](https://serde.rs/enum-representations.html) — Serdeのenum表現パターン
- PostgreSQL 17.x公式ドキュメント — 方言固有型のリファレンス
- MySQL 8.x公式ドキュメント — 方言固有型のリファレンス
- SQLite 3.x公式ドキュメント — 方言固有型のリファレンス
