# Gap Analysis: check-command-wrapper

**作成日**: 2026-02-02T22:10:00Z
**仕様ID**: check-command-wrapper
**分析対象**: 既存コードベースと要件の実装ギャップ
**注意**: requirements 未承認（分析は継続）

---

## Analysis Summary

- `validate` と `generate --dry-run` は既存実装があり、コマンド合成の薄いラッパー追加が主要ギャップ
- ただし `validate` と `generate` の入出力・設定の取り回しが異なるため、統一出力と入力継承の設計が必要
- `generate` はスキーマディレクトリの上書き入力を持たず、`check` 要件の「スキーマパス受け付け」に対する制約が存在
- 失敗時出力（特に JSON モード）で `validate` が標準出力/エラー出力を直接使うため、合成コマンドのエラー整形方針を決める必要がある

## Document Status

- gap-analysis.md のフレームワークに従って現状調査・要件実現性・オプション比較・マッピングを実施
- 追加リサーチは設計フェーズで実施する前提

## Next Steps

- このギャップ分析を踏まえ `/prompts:kiro-spec-design check-command-wrapper` を実行
- 入力継承（schema_dir の扱い）と出力フォーマット（特に JSON）の設計方針を決定

---

## 1. 現状調査 (Current State Investigation)

### 1.1 関連資産

#### CLI コマンド定義/ディスパッチ
- `src/cli/src/cli.rs`
  - 既存サブコマンド: `init/generate/apply/rollback/validate/status/export`
  - `check` は未定義
- `src/cli/src/main.rs`
  - `Commands` 列挙に基づく dispatch 実装
  - ここに `Check` 分岐は存在しない

#### validate 実装
- `src/cli/src/cli/commands/validate.rs`
  - `ValidateCommandHandler::execute` が検証と出力を担当
  - `schema_dir` は CLI 引数で上書き可能（`CommandContext::resolve_schema_dir`）
  - 失敗時は `Err` を返し、JSON モードでは stdout に JSON を出力してから `Err` を返す

#### generate --dry-run 実装
- `src/cli/src/cli/commands/generate/mod.rs`
  - `GenerateCommandHandler::execute` が diff/SQL 生成/出力を担当
  - `dry_run` フラグでファイル書き出しを回避
- `src/cli/src/cli/commands/dry_run_formatter.rs`
  - dry-run の出力整形を集中管理
- `src/cli/src/cli/commands/generate/diff.rs`
  - 破壊的変更検出は `DestructiveChangeDetector` を利用
  - dry-run は `--allow-destructive` が不要（非 dry-run のみ拒否）

#### 設定・入力
- `src/cli/src/cli/command_context.rs`
  - config 読み込み、schema_dir/migrations_dir 解決
- `src/core/src/core/config.rs`
  - 設定ファイル `.strata.yaml` の schema_dir/migrations_dir を保持
- `generate` には schema_dir を直接上書きする CLI 引数がない

#### 出力フォーマット
- `src/cli/src/cli/commands/mod.rs`
  - `CommandOutput` + `render_output` による Text/JSON 形式の統一
- `ValidateOutput` / `GenerateOutput` は個別の JSON 構造を持つ
- `check` 用の合成出力は未定義

#### テストパターン
- `src/cli/tests/cmd_validate_test.rs` に validate のコマンドテスト
- `src/cli/tests/unit_cli_parsing_test.rs` に CLI パーステスト
- `check` コマンド用のテストは存在しない

### 1.2 既存の慣習とパターン

- CLI サブコマンドごとに `commands/{command}.rs` を作成し、`{Command}Handler` を実装
- `CommandOutput` を実装する出力構造体で Text/JSON を切り替え
- JSON モードでの失敗は `ErrorOutput` で統一（ただし validate は例外的に自前出力）

### 1.3 統合ポイント

- `check` の追加は CLI 定義 (`cli.rs`) と dispatcher (`main.rs`) が主要統合点
- `validate` と `generate --dry-run` の実行順序と出力結合が設計上の中心
- `schema_dir` 上書きの取り扱いが `validate` と `generate` で非対称

---

## 2. 要件実現性分析 (Requirements Feasibility)

### 2.1 技術的要求事項とギャップ

#### 要件1: validate → generate --dry-run の順次実行
- **既存資産**: `ValidateCommandHandler::execute`, `GenerateCommandHandler::execute`
- **ギャップ**:
  - ❌ `check` コマンド自体が未実装
  - ⚠️ `validate` の失敗時出力が副作用を持ち、合成コマンドで二重出力/フォーマット不整合の可能性

#### 要件2: 入力と設定の継承
- **既存資産**:
  - `validate` は `schema_dir` 上書きに対応
  - `generate` は config の schema_dir/migrations_dir に固定
- **ギャップ**:
  - ❌ `check` に `schema_dir` を渡した場合、`generate` に反映できない
  - ⚠️ `generate` に schema_dir 上書きを追加するか、`check` が独自にスキーマ読み込みを行う必要

#### 要件3: 出力と結果表示
- **既存資産**:
  - `validate` は詳細な検証結果表示を持つ
  - `generate --dry-run` は SQL/差分の詳細表示を持つ
- **ギャップ**:
  - ❌ `check` 用の統一出力（Text/JSON）が未定義
  - ⚠️ 複数出力の連結形式や JSON スキーマ設計が必要

#### 要件4: 終了コードと失敗時挙動
- **既存資産**: `main.rs` が `Result` で終了コード 1 を返す
- **ギャップ**:
  - ⚠️ `validate` の失敗時に `check` がどの出力を残すか（validate 出力のみ/統一出力）設計が必要
  - ⚠️ `generate` 失敗時のメッセージ整形を `check` 側で統合する必要

#### 要件5: 非破壊性
- **既存資産**: `generate --dry-run` はファイル書き出しを行わない
- **ギャップ**:
  - ✅ 破壊的変更や DB 書き込みは発生しない
  - ⚠️ `check` が内部で `generate` を呼ぶ際に `dry_run` を強制する必要

### 2.2 非機能要件・制約

- **互換性**: 既存 CLI の出力契約（Text/JSON）との整合が必要
- **保守性**: `check` が `validate`/`generate` と重複ロジックを持つと将来的な差異リスクが増える
- **入力制約**: `generate` の schema_dir 上書き非対応が要件2の主要制約

**Research Needed**:
1. `check` の JSON 出力スキーマ（validate/generate の埋め込み vs 独自構造）
2. `schema_dir` 上書きを `generate` に追加する場合の影響範囲（CLI/API/テスト）
3. `validate` の失敗時出力副作用を `check` でどう扱うか（再利用 vs 再実装）

---

## 3. 実装アプローチオプション

### Option A: 既存ハンドラーの直接呼び出し（Extend Existing Components）

**概要**: `CheckCommandHandler` が `ValidateCommandHandler` → `GenerateCommandHandler(dry_run=true)` を順に呼び出し、出力を結合

**変更対象**:
- `src/cli/src/cli.rs`: `Commands::Check` 追加
- `src/cli/src/main.rs`: `Check` 分岐追加
- `src/cli/src/cli/commands/check.rs`: 新規ハンドラー

**Trade-offs**:
- ✅ 実装が最小、既存処理を再利用
- ✅ dry-run フォーマッタ等の既存出力を利用可能
- ❌ `validate` の失敗時出力副作用で JSON 結合が難しい
- ❌ schema_dir 上書きが `generate` に伝わらず、要件2とのズレ

### Option B: check 専用の統合ハンドラー実装（Create New Components）

**概要**: `check` が `SchemaParserService`/`SchemaValidatorService`/`SchemaDiffDetector`/`MigrationGenerator` を直接利用し、独自の出力構造を生成

**新規/拡張要素**:
- `CheckOutput` を定義し、validate/generate の結果を一体化
- schema_dir 上書きを `check` 内で処理

**Trade-offs**:
- ✅ 入力継承と出力統一を最適に設計可能
- ✅ JSON スキーマを明確化できる
- ❌ `generate` とロジック重複が増え、保守コストが上がる
- ❌ dry-run の出力整形との整合維持が必要

### Option C: ハイブリッド（Hybrid Approach）

**概要**: `validate` はサービス層を直接使用、`generate --dry-run` は既存ハンドラーを利用して出力を再利用

**組み合わせ戦略**:
- `check` 内で validate を再実装し、output を制御
- generate の dry-run 出力は既存 `GenerateCommandHandler` を利用

**Trade-offs**:
- ✅ validate の副作用問題を回避しつつ generate の既存出力を活用
- ✅ schema_dir 上書きを `check` が保持したまま generate へ渡せる設計に拡張可能
- ❌ 一部ロジックの重複が残る
- ❌ generate 側への schema_dir 対応追加が必要になる可能性

---

## 4. 実装複雑性とリスク評価

### 工数見積もり
- **Option A**: **S〜M (1-5 days)**
  - 理由: 既存ハンドラーの合成のみだが、出力統合の調整が必要
- **Option B**: **M (3-7 days)**
  - 理由: 統合ロジックと出力構造を新規設計する必要がある
- **Option C**: **M (3-7 days)**
  - 理由: validate の再実装 + generate の再利用調整が必要

### リスク評価
- **Option A**: Medium
  - 理由: JSON 出力の一貫性と schema_dir 上書き要件が未解決
- **Option B**: Medium
  - 理由: 重複ロジックによる将来的な整合性リスク
- **Option C**: Medium-Low
  - 理由: 重複を最小化しつつ出力制御も確保できる

---

## 5. 設計フェーズへの推奨事項 (Recommendations)

### 5.1 推奨アプローチ

**Option C (ハイブリッド)** を推奨
- validate の副作用を避けつつ generate の dry-run 出力を再利用できるため、要件2/3の両立が容易

### 5.2 設計フェーズで決定すべき事項

1. `check` の JSON 出力スキーマ（validate/generate のネスト構造か、単一サマリーか）
2. `schema_dir` 上書きを `generate` に追加するか、`check` 内で処理するか
3. `check` の text 出力でのセクション分割と成功メッセージの表現

### 5.3 設計フェーズで実施すべきリサーチ

1. `validate` の出力副作用を抑制する既存パターン（他コマンドでの例）
2. `generate` の dry-run 出力の再利用可否（中間構造の再利用 vs 文字列連結）

---

## 6. 要件とコンポーネントのマッピング

| 要件 | 既存コンポーネント | ギャップ | 備考 |
|------|-------------------|---------|------|
| Req 1.1 validate 実行 | `ValidateCommandHandler` | ✅ Reusable | 失敗時副作用は要考慮 |
| Req 1.2 validate 成功時に generate | `GenerateCommandHandler` | ✅ Reusable | dry_run 強制が必要 |
| Req 1.3 validate 失敗時に generate を実行しない | `main.rs`/`Result` | ⚠️ Constraint | `check` の制御ロジックが必要 |
| Req 1.4 既存意味論一致 | `validate`/`generate` 実装 | ⚠️ Constraint | 入力・出力の統合方針が必要 |
| Req 2.1/2.2 入力継承 | `ValidateCommand` / `GenerateCommand` | ❌ Missing | schema_dir 上書きが generate に無い |
| Req 2.3 主要入力経路 | `CommandContext` | ⚠️ Constraint | config/schema_dir の優先順位設計が必要 |
| Req 3.1/3.2 出力表示 | `ValidateOutput` / `GenerateOutput` | ❌ Missing | check 用統一出力が未実装 |
| Req 3.3/3.4 失敗理由表示 | `ErrorOutput` / validate 出力 | ⚠️ Constraint | JSON/TXT 統合が必要 |
| Req 4.1-4.3 終了コード | `main.rs` | ✅ Reusable | `check` の Err 伝播で対応可能 |
| Req 5.1/5.2 非破壊性 | `generate --dry-run` | ✅ Reusable | dry_run 強制を保証する必要 |

**凡例**:
- ✅ **Reusable**: 既存コンポーネントを流用可能
- ⚠️ **Constraint**: 既存拡張が必要
- ❌ **Missing**: 新規作成が必要

---

## 7. まとめ

- `validate` と `generate --dry-run` は既に存在するため、主なギャップは **CLI 合成と出力統合** に集中している
- `schema_dir` 上書きの非対称性が要件2の最大制約であり、設計フェーズで優先的に解決する必要がある
- JSON 出力の整合性と失敗時挙動は CI 利用の観点で重要であり、`check` 専用の出力設計が必要

**技術的リスク**: Medium
**実装工数**: M
