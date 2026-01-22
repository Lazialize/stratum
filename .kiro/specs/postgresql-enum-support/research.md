# Research & Design Decisions

---
**Purpose**: Capture discovery findings, architectural investigations, and rationale that inform the technical design.

**Usage**:
- Log research activities and outcomes during the discovery phase.
- Document design decision trade-offs that are too detailed for `design.md`.
- Provide references and evidence for future audits or reuse.
---

## Summary
- **Feature**: `postgresql-enum-support`
- **Discovery Scope**: Extension
- **Key Findings**:
  - 既存の`DialectSpecific`は検証とマイグレーション対象外で、PostgreSQL ENUMの作成・変更に必要なDDLが生成されていない。
  - PostgreSQLのSQL生成は`ENUM(values...)`をカラム型として出力するため、PostgreSQLの型定義手順と整合しない。
  - マイグレーション生成はテーブル中心で、型（ENUM）の作成・変更・削除を順序制御する仕組みがない。

## Research Log

### コードベースの拡張点
- **Context**: 既存の型定義・検証・差分・SQL生成の拡張位置を特定する必要があった。
- **Sources Consulted**: `src/core/schema.rs`, `src/services/schema_validator.rs`, `src/services/schema_diff_detector.rs`, `src/services/migration_generator.rs`, `src/adapters/sql_generator/postgres.rs`, `src/cli/commands/export.rs`
- **Findings**:
  - スキーマ表現は`ColumnType::DialectSpecific`で方言固有型を表現している。
  - バリデータは`DialectSpecific`を検証スキップしている。
  - PostgreSQL SQL生成は`ENUM`をカラム型として出力するが、型作成DDLがない。
  - マイグレーション生成はテーブル追加/削除/カラム追加に限定される。
- **Implications**: ENUMを一級データとしてスキーマに定義し、差分・DDL順序・エクスポートまで横断的に拡張する必要がある。

### PostgreSQL ENUMの変更特性
- **Context**: 変更時のDDL生成戦略を決定する必要があった。
- **Sources Consulted**: 既存コードベースと一般的なPostgreSQLの制約知識（外部調査は未実施）
- **Findings**:
  - ENUMの追加は安全に拡張できるが、削除や並び替えは制約が強い。
  - 変更には型の再作成とカラム型の移行が必要になるケースがある。
- **Implications**: 変更種別（追加/再作成）を判定し、危険な変更には明示的なマイグレーション手順とリスク表示が必要。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| DialectSpecific継続 | ENUMを方言固有型のまま扱う | 既存構造に最小変更 | 検証・差分・DDL順序が扱えない | 既存の問題を解消できない |
| Enumを一級定義 | スキーマにENUM定義を追加し参照 | 検証と差分、DDL順序の統一が可能 | 影響範囲が広い | 要件2-4に整合 |

## Design Decisions

### Decision: ENUMをスキーマの一級定義として追加
- **Context**: 検証・差分・DDLの全経路でENUMを扱う必要がある。
- **Alternatives Considered**:
  1. DialectSpecificに値配列を保持し続ける
  2. SchemaにENUM定義コレクションを追加し、カラムは参照名で紐付ける
- **Selected Approach**: SchemaにENUM定義を追加し、ColumnTypeはENUM参照を表現する。
- **Rationale**: 参照整合性と変更検知を明確化でき、DDLの順序制御が容易になる。
- **Trade-offs**: 既存の方言固有型パスとの二重化を整理する必要がある。
- **Follow-up**: 既存のDialectSpecificの利用範囲を明文化し、互換性ルールをテストで担保する。

### Decision: ENUM変更のDDL戦略
- **Context**: 追加・削除・並び替えを含む変更に対して安全なマイグレーションを提供する必要がある。
- **Alternatives Considered**:
  1. すべて再作成（型作成→カラム型移行→旧型削除）
  2. 追加はALTER TYPE、削除/並び替えは再作成
- **Selected Approach**: 追加は`ALTER TYPE`、削除/並び替えは再作成を基本とする。
- **Rationale**: 変更の安全性と運用コストのバランスが取れる。
- **Trade-offs**: 再作成時はカラム移行と依存関係順序の調整が必要。
- **Follow-up**: 変更判定の境界条件（値順序維持）をテストで検証する。

### Decision: エクスポート時のENUM取り込み
- **Context**: 既存DBのENUMをスキーマに取り込む要件がある。
- **Alternatives Considered**:
  1. カラム型のみをDialectSpecificとして出力
  2. ENUM定義をスキーマ上部に抽出し、カラムから参照
- **Selected Approach**: ENUM定義を抽出し、カラムは参照名で表現する。
- **Rationale**: 再読込で同等のENUMを再構築でき、差分検出が安定する。
- **Trade-offs**: 既存のエクスポート形式からの変更に伴う互換性調整が必要。
- **Follow-up**: 既存スキーマに対する後方互換の扱いを決める。

## Risks & Mitigations
- ENUM削除や並び替えが含まれる変更でロールバックが難しくなる — 再作成手順の明示とテスト強化
- 既存DialectSpecific利用との二重運用で混乱が起きる — ENUMは専用定義に限定するルールを明文化
- 依存関係順序でDDL失敗が起きる — 型作成/削除の前後順序をマイグレーション生成で制御

## References
- コードベース内の既存実装（上記参照）
