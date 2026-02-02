# 研究と設計判断ログ

---
**目的**: ディスカバリ結果、設計判断の根拠、トレードオフを記録する。
---

## Summary
- **Feature**: check-command-wrapper
- **Discovery Scope**: Extension
- **Key Findings**:
  - 既存 `validate` は `schema_dir` 上書きに対応するが、`generate` は未対応であるため入力継承に制約がある。
  - `validate` は JSON モード失敗時に stdout へ直接出力する副作用があり、合成コマンドでは出力統合の設計が必要。
  - CLI 層は `CommandOutput` と `render_output` による Text/JSON 分離が確立しており、新コマンドも同一パターンに合わせる必要がある。

## Research Log

### 既存 CLI 拡張ポイント
- **Context**: `check` を CLI サブコマンドとして追加するための統合点を確認。
- **Sources Consulted**: `src/cli/src/cli.rs`, `src/cli/src/main.rs`, `src/cli/src/cli/commands/mod.rs`
- **Findings**:
  - サブコマンドは `Commands` 列挙に追加し、`main.rs` の dispatch に分岐を追加する必要がある。
  - 各コマンドは `commands/{name}.rs` で `CommandHandler` を実装し、`CommandOutput` を返す慣習がある。
- **Implications**: `check` も `CheckCommandHandler` と `CheckOutput` を用意し、既存の出力パターンに統合する。

### validate / generate の入力と出力
- **Context**: 要件 2 の「入力継承」と要件 3 の「出力統合」を実現するため。
- **Sources Consulted**: `src/cli/src/cli/commands/validate.rs`, `src/cli/src/cli/commands/generate/mod.rs`, `src/cli/src/cli/commands/generate/diff.rs`
- **Findings**:
  - `validate` は `schema_dir` 上書きをサポートするが、`generate` は config の schema_dir 固定。
  - `validate` は JSON 失敗時に stdout へ JSON を出力してから `Err` を返す。
  - `generate --dry-run` はファイルを書かずに SQL を表示し、破壊的変更の拒否は非 dry-run のみ。
- **Implications**: `check` は validate を直接呼ばずにサービス層で検証し、出力を制御する設計が必要。`generate` には schema_dir 上書きの拡張を検討する。

### 出力フォーマットと構造化 JSON
- **Context**: CI 用の `check` を想定した JSON 出力要件への対応。
- **Sources Consulted**: `src/cli/src/cli/commands/mod.rs`, `src/cli/src/cli/commands/validate.rs`, `src/cli/src/cli/commands/generate/mod.rs`
- **Findings**:
  - `CommandOutput` で Text/JSON を切替え、JSON では構造化出力を返すのが標準。
  - 既存の `ValidateOutput` と `GenerateOutput` は独立した JSON 構造を持つ。
- **Implications**: `check` の JSON は `validate` と `generate` の出力をネストした構造にし、要件 3 と 4 の明確性を確保する。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 既存ハンドラー合成 | `ValidateCommandHandler` → `GenerateCommandHandler` を直列呼び出し | 実装が最小 | validate の JSON 副作用で統一出力が難しい | 要件 3 に不利 |
| check 専用統合 | サービス層を直接使い、独自の出力を生成 | 出力制御が容易 | generate のロジック重複 | 保守コスト増 |
| ハイブリッド | validate をサービス層で再構築し、generate dry-run を再利用 | 出力制御と再利用のバランス | generate 入力の拡張が必要 | 推奨案 |

## Design Decisions

### Decision: check はハイブリッド構成を採用
- **Context**: 出力統合と既存ロジック再利用を両立する必要がある。
- **Alternatives Considered**:
  1. 既存ハンドラー合成
  2. check 専用統合
- **Selected Approach**: validate はサービス層（parser/validator）で実行し、generate は dry-run のみ既存ハンドラーを利用する。
- **Rationale**: validate の副作用を避けつつ generate の表示仕様を維持できる。
- **Trade-offs**: 一部ロジックの重複と generate 入力拡張が必要。
- **Follow-up**: generate 側の schema_dir 上書き導入時の影響範囲を実装時に検証。

### Decision: check JSON は validate と generate をネスト
- **Context**: CI が結果を機械判定できる構造が必要。
- **Alternatives Considered**:
  1. 文字列連結のみ
  2. 単一サマリーのみ
- **Selected Approach**: `CheckOutput` に `validate` / `generate` をネストし、サマリー項目を併記。
- **Rationale**: 要件 3 と 4 の「失敗理由の明確化」と「終了コード対応」を満たす。
- **Trade-offs**: 既存 JSON との互換はないが新コマンドなので許容。
- **Follow-up**: JSON スキーマを README に反映するかは設計レビューで判断。

## Risks & Mitigations
- validate の JSON 副作用による二重出力 — validate をサービス層で再利用し、CheckOutput に統合する。
- schema_dir 上書き未対応 — generate に `schema_dir` オプションを追加し、check から伝播する。
- 出力結合の曖昧さ — text では明確なセクション見出しと成功/失敗メッセージを設ける。

## References
- `src/cli/src/cli.rs`
- `src/cli/src/main.rs`
- `src/cli/src/cli/commands/validate.rs`
- `src/cli/src/cli/commands/generate/mod.rs`
- `src/cli/src/cli/commands/generate/diff.rs`
- `.kiro/steering/structure.md`
- `.kiro/steering/tech.md`
