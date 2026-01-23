# Research & Design Decisions Template

---
**Purpose**: Capture discovery findings, architectural investigations, and rationale that inform the technical design.

**Usage**:
- Log research activities and outcomes during the discovery phase.
- Document design decision trade-offs that are too detailed for `design.md`.
- Provide references and evidence for future audits or reuse.
---

## Summary
- **Feature**: `app-rename-strata`
- **Discovery Scope**: Extension
- **Key Findings**:
  - CLI名とヘルプ文言は`src/cli.rs`に集中しており、名前変更の主導点になる
  - 設定ファイル名は`src/core/config.rs`の定数と`src/cli/commands/init.rs`で生成される
  - 配布物/ドキュメント/ビルドスクリプトに`stratum`表記が多数存在するため一括更新が必要

## Research Log

### 既存の名称参照の分布
- **Context**: 変更範囲と影響点を把握するため
- **Sources Consulted**: リポジトリ内`rg "stratum"`検索結果
- **Findings**:
  - CLIコマンド名は`src/cli.rs`の`#[command(name = "stratum")]`とヘルプ例に埋め込み
  - `.stratum.yaml`は`src/core/config.rs`の`DEFAULT_CONFIG_PATH`と`src/cli/commands/init.rs`で生成
  - Cargoのクレート名は`Cargo.toml`、バイナリ名はビルド成果物や`BUILDING.md`/`scripts/build-release.sh`で指定
  - README/CONTRIBUTING/ROADMAP/CHANGELOG/例示ドキュメントに`stratum`記載多数
- **Implications**:
  - CLI/設定/ビルド/ドキュメントの4領域に分けて設計と作業分担を行う必要がある

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 既存レイヤー拡張 | CLI/Services/Coreの既存構成に名称ポリシーを追加 | 変更箇所が明確、既存パターンに一致 | 改修範囲が広い | SteeringのClean Architectureに整合 |
| 新規独立モジュール | 命名だけの独立クレートを追加 | 将来的再利用 | 依存関係増大、現状過剰 | 今回は不要 |

## Design Decisions

### Decision: 命名ポリシーをCoreに集約
- **Context**: CLI/設定/ビルドで同じ名称定義を参照する必要がある
- **Alternatives Considered**:
  1. 各モジュールで定数を持つ
  2. Coreに集約し参照する
- **Selected Approach**: Coreに`NamingProfile`を定義し、CLIとServicesが参照する
- **Rationale**: 変更点の一元化で表記ゆれを防止
- **Trade-offs**: Coreへの参照が増える
- **Follow-up**: 既存テストの修正と新規テスト追加

## Risks & Mitigations
- 置換漏れで旧名称が残るリスク — 文字列検索とドキュメント点検をタスク化
- CLIの表示とバイナリ名が不一致になるリスク — ビルドスクリプトとCargo設定を同時更新

## References
- リポジトリ内検索結果（ローカル）
