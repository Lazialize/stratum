# Research & Design Decisions

---
**Purpose**: 破壊的変更安全ガードの設計判断と調査結果を記録

**Usage**: 設計フェーズでの決定事項と技術的根拠を文書化
---

## Summary
- **Feature**: `destructive-change-guard`
- **Discovery Scope**: Extension (既存システムの拡張)
- **Key Findings**:
  - `SchemaDiff` に破壊的変更データが既に含まれており、新規検出ロジックは不要
  - `.meta.yaml` の拡張でメタデータ永続化が可能
  - ~~`split_sql_statements` でSQL解析によるフォールバック検出が実装可能~~ → **レビュー後の決定: SQLフォールバックは実装しない**
- **Design Review Outcomes** (2026-01-26 ~ 2026-01-27):
  - SQL解析フォールバックを廃止し、古いマイグレーションは破壊的変更ありとして扱う
  - `enum_recreate_allowed` を廃止し、`--allow-destructive` に統合
  - 旧メタデータ時は警告バナーのみ表示（個別SQL強調なし）
  - `destructive_changes` フィールドは空オブジェクトでも必ず保存
  - `enum_recreate_allowed` は警告表示して無視
  - **3回目レビュー**: `apply --dry-run` で軽量キーワードハイライト、旧メタデータ時は最小限情報表示

## Research Log

### 既存の破壊的変更検出基盤
- **Context**: 要件1（破壊的変更の検出と分類）の実現可能性調査
- **Sources Consulted**:
  - [src/core/src/core/schema_diff.rs](../../../src/core/src/core/schema_diff.rs)
  - [src/db/src/services/schema_diff_detector.rs](../../../src/db/src/services/schema_diff_detector.rs)
  - [gap-analysis.md](./gap-analysis.md)
- **Findings**:
  - `SchemaDiff` 構造体が以下のフィールドを保持：
    - `removed_tables: Vec<String>` - テーブル削除
    - `TableDiff.removed_columns: Vec<String>` - カラム削除
    - `TableDiff.renamed_columns: Vec<RenamedColumn>` - カラムリネーム
    - `removed_enums: Vec<String>` - ENUM削除
    - `EnumDiff.change_kind: EnumChangeKind::Recreate` - ENUM再作成
  - `SchemaDiffDetector` サービスが既に差分を完全に抽出可能
  - 既存の `enum_recreate_allowed` フラグが ENUM再作成のガードとして機能
- **Implications**:
  - 新規の破壊的変更検出ロジックを実装する必要はなく、`SchemaDiff` を読み取るラッパーサービスで十分
  - ENUM以外の破壊的変更にも同様のガードメカニズムを拡張する

### メタデータフォーマット拡張戦略
- **Context**: 要件2.2（`apply` での破壊的変更拒否）の実現方法
- **Sources Consulted**:
  - [src/core/src/core/migration.rs](../../../src/core/src/core/migration.rs#L13-L37)
  - [src/db/src/services/migration_generator.rs](../../../src/db/src/services/migration_generator.rs#L130-L141)
- **Findings**:
  - 既存の `.meta.yaml` フォーマット:
    ```yaml
    version: <timestamp>
    description: <description>
    dialect: <dialect>
    checksum: <sha256>
    ```
  - YAML形式でフィールド追加が容易（serde-saphyr使用）
  - 既存マイグレーションには新フィールドが存在しないが、Option型でパース可能
- **Implications**:
  - `destructive_changes` フィールドを追加し、破壊的変更の種類と影響範囲を記録
  - 後方互換性を維持（古いメタデータはNoneとして扱う）

### SQL解析ユーティリティの活用
- **Context**: 既存マイグレーションへのフォールバック検出方法
- **Sources Consulted**:
  - [src/cli/src/cli/commands/mod.rs](../../../src/cli/src/cli/commands/mod.rs#L11-L99) (`split_sql_statements`)
  - [src/cli/src/cli/commands/apply.rs](../../../src/cli/src/cli/commands/apply.rs) (使用例)
- **Findings**:
  - `split_sql_statements` 関数が既に実装されており、以下をサポート：
    - 文字列リテラル（シングル/ダブルクォート）の正確な処理
    - ドルクォート（PostgreSQL）の処理
    - セミコロン区切りでのSQL文分割
  - `apply` コマンドで既に使用されている
- **Implications**:
  - SQL文を解析して `DROP TABLE`, `DROP COLUMN`, `RENAME COLUMN` などのキーワードを検出可能
  - 簡易パターンマッチングで破壊的変更を推定（完全な精度は不要、メタデータ優先）

### CLI引数拡張の既存パターン
- **Context**: 要件3（明示的な許可フラグ）の実装方法
- **Sources Consulted**:
  - [src/cli/src/cli.rs](../../../src/cli/src/cli.rs) (clap定義)
  - [gap-analysis.md](./gap-analysis.md#L86-L90)
- **Findings**:
  - clap 4.5 derive macros使用
  - 既存の `--dry-run` フラグがGenerateコマンドとApplyコマンドに存在
  - グローバルフラグ（`--no-color`, `--verbose`）とコマンド固有フラグが混在
- **Implications**:
  - `--allow-destructive` フラグをGenerateとApplyコマンドに追加（コマンド固有フラグ）
  - `dry_run` と同様のbool型フラグとして実装

### enum_recreate_allowedとの統合方針
- **Context**: 既存のENUMガードと新フラグの整合性
- **Sources Consulted**:
  - [src/db/src/services/migration_pipeline.rs](../../../src/db/src/services/migration_pipeline.rs) (ENUM再作成ガード実装)
  - [gap-analysis.md](./gap-analysis.md#L114-L116)
- **Findings**:
  - `enum_recreate_allowed` は `SchemaDiff` の内部フラグ（デフォルト false）
  - ENUM再作成/削除時にエラーを返す仕組み
  - ユーザー向けのCLIフラグは存在しない
- **Implications**:
  - `--allow-destructive` が指定された場合、`SchemaDiff.enum_recreate_allowed = true` を設定
  - ENUMだけでなく、すべての破壊的変更を統一的に扱う

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Option A: CLI内包 | CLIコマンド内で直接検出・拒否 | 最小限の変更、新規型不要 | 責務肥大化、SQL解析精度に依存 | 保守性が低い |
| Option B: サービス新設 | `DestructiveChangeDetector` 新設、メタデータ完全依存 | ロジック集中、高精度 | メタデータ互換性、既存マイグレーション対応困難 | **採用**（レビュー後） |
| Option C: ハイブリッド | サービス化 + SQL解析フォールバック | 新旧対応、段階移行可能 | 2系統のロジック保守 | ~~推奨~~（廃止） |

## Design Decisions

### Decision: Option C (ハイブリッドアプローチ) の採用

- **Context**: `generate` と `apply` で異なる情報源を持つため、統一的な検出方法が必要
- **Alternatives Considered**:
  1. Option A (CLI内包) — CLIが肥大化し、テストが困難
  2. Option B (サービス新設のみ) — 既存マイグレーションで動作しない
  3. Option C (ハイブリッド) — 互換性と設計のバランス
- **Selected Approach**:
  - `DestructiveChangeDetector` サービスを新設し、`SchemaDiff` から破壊的変更を抽出
  - `generate` 時に検出結果を `.meta.yaml` に保存
  - `apply` 時はメタデータ優先、なければSQL解析でフォールバック
- **Rationale**:
  - ギャップ分析で推奨されたOption Cと一致
  - 新規マイグレーションは高精度、既存マイグレーションも部分的にサポート可能
  - サービス層に責務を集約することで、CLI層の肥大化を防止
- **Trade-offs**:
  - **Benefits**: 互換性維持、段階的な移行、テスタビリティ
  - **Compromises**: 2系統のロジック（メタデータ vs SQL解析）を保守する必要
- **Follow-up**:
  - SQL解析の精度は完全である必要はない（警告的な位置づけ）
  - 将来的にはメタデータのみに一本化可能

### Decision: `.meta.yaml` へのフィールド追加

- **Context**: `apply` コマンドで破壊的変更情報を取得する必要がある
- **Alternatives Considered**:
  1. 別ファイル (`.destructive.yaml`) — ファイル数増加、管理複雑化
  2. `.meta.yaml` 拡張 — 既存フォーマットへの追加
- **Selected Approach**: `.meta.yaml` に `destructive_changes` フィールドを追加
  ```yaml
  destructive_changes:
    tables_dropped: ["users", "posts"]
    columns_dropped:
      - table: "products"
        columns: ["old_field"]
    columns_renamed:
      - table: "items"
        old_name: "name"
        new_name: "item_name"
    enums_dropped: ["status_enum"]
    enums_recreated: ["priority_enum"]
  ```
- **Rationale**:
  - 既存のメタデータファイルを再利用し、ファイル数を増やさない
  - Option型でパースすることで後方互換性を維持
- **Trade-offs**:
  - **Benefits**: 単一ファイル管理、後方互換性
  - **Compromises**: メタデータファイルのサイズが若干増加
- **Follow-up**: `DestructiveChangeReport` の serde シリアライゼーション実装

### ~~Decision: SQL解析の簡易パターンマッチング~~ (廃止)

> **Status**: 廃止（2026-01-26 設計レビュー後の決定）

- **Context**: 既存マイグレーションにはメタデータが存在しないため、SQL文から推定が必要
- **Original Approach**: 正規表現とキーワードマッチングによる簡易検出
- **Reason for Deprecation**:
  - SQL解析の精度問題（誤検出リスク）
  - 2系統のロジック保守コスト
  - 安全性を優先し、古いマイグレーションは破壊的変更ありとして扱う方が明確

### Decision: 古いマイグレーションの扱い（レビュー後決定）

- **Context**: `.meta.yaml` に `destructive_changes` フィールドがない古いマイグレーションの扱い
- **Alternatives Considered**:
  1. SQL解析でフォールバック検出 — 精度問題、保守コスト
  2. 破壊的変更ありとして扱う — 安全性優先、明確な動作
  3. 非破壊的変更として扱う — 危険、安全性軽視
- **Selected Approach**: **破壊的変更ありとして扱う**
  - `destructive_changes` フィールドがない場合は `--allow-destructive` が必須
  - エラーメッセージで「Legacy migration format detected」と表示
- **Rationale**:
  - 安全性を最優先（Design Principle 4: 安全性優先）
  - 明確な動作（SQL解析の曖昧さを排除）
  - 単一のロジックで保守性向上
- **Trade-offs**:
  - **Benefits**: 安全性、明確な動作、保守性
  - **Compromises**: 古いマイグレーションの適用に追加フラグが必要
- **User Decision**: "破壊的変更とし、以前のフォーマットをサポートしない"

### Decision: enum_recreate_allowed の廃止（レビュー後決定）

- **Context**: 既存の `enum_recreate_allowed` フラグと新しい `--allow-destructive` フラグの関係
- **Alternatives Considered**:
  1. 両方のフラグを維持 — 動作が複雑、ユーザー混乱
  2. `--allow-destructive` が `enum_recreate_allowed = true` を設定 — 暗黙的な動作
  3. `enum_recreate_allowed` を廃止し統合 — 明確、一元管理
- **Selected Approach**: **`enum_recreate_allowed` を廃止し `--allow-destructive` に統合**
  - `MigrationPipeline` 内部で `allow_destructive` フラグを参照するよう変更
  - すべての破壊的変更（テーブル削除、カラム削除、リネーム、ENUM削除、ENUM再作成）を単一フラグで制御
- **Rationale**:
  - 破壊的変更の許可を一元管理
  - ユーザーにとって明確なインターフェース
  - 内部APIの簡素化
- **Trade-offs**:
  - **Benefits**: 一元管理、明確なAPI、保守性
  - **Compromises**: 内部APIの変更が必要（`MigrationPipeline`）
- **User Decision**: "enum_recreate_allowedを廃止"

### Decision: 旧メタデータ時のdry-run表示（2回目レビュー後決定）

- **Context**: 旧メタデータ（`destructive_changes` フィールドなし）時の `apply --dry-run` での表示方法
- **Alternatives Considered**:
  1. 警告バナーのみ — 個別SQL強調なし
  2. SQL全体を警告色で表示 — 個別特定は不可
- **Selected Approach**: **警告バナーのみ（個別SQL強調なし）**
  - 「Legacy migration format detected - treating as destructive」警告バナーを表示
  - 個別の破壊的SQL文の色付き強調表示は行わない
- **Rationale**:
  - 破壊的SQLの特定にはメタデータが必要だが、旧フォーマットには存在しない
  - 誤った情報（誤検出）を表示するより、「不明」を明示する方が安全
  - 要件4.4の例外として明記
- **Trade-offs**:
  - **Benefits**: 誤解を招かない、明確な動作
  - **Compromises**: 旧マイグレーションでは詳細な強調表示ができない
- **User Decision**: "警告バナーのみ（推奨）"

### Decision: destructive_changesの必須保存（2回目レビュー後決定）

- **Context**: `generate` 時の `destructive_changes` フィールドの保存方針
- **Alternatives Considered**:
  1. 空オブジェクトを必ず保存 — 新旧判別が明確
  2. 破壊的変更なしの場合は省略 — 別フラグで判別が必要
- **Selected Approach**: **空オブジェクトを必ず保存**
  - 破壊的変更なしの場合: `destructive_changes: {}`
  - 破壊的変更ありの場合: 該当するフィールドに値を設定
  - フィールドの有無で新旧フォーマットを判別
- **Rationale**:
  - 判定ロジックが単純（フィールドの有無のみ）
  - `apply` 側での誤判定リスクを排除
  - 明示的な「破壊的変更なし」の表現
- **Trade-offs**:
  - **Benefits**: 明確な判別、単純なロジック、誤判定回避
  - **Compromises**: 全マイグレーションで若干のファイルサイズ増加（数十バイト）
- **User Decision**: "空オブジェクトを必ず保存（推奨）"

### Decision: enum_recreate_allowed互換性方針（2回目レビュー後決定）

- **Context**: 既存設定やスキーマに `enum_recreate_allowed` フィールドがある場合の扱い
- **Alternatives Considered**:
  1. 警告＋無視 — 値を読まず、警告のみ表示
  2. 自動移行 — true の場合は暗黙的に許可
  3. エラーで拒否 — 手動削除を要求
- **Selected Approach**: **警告＋無視**
  - 読み取り時に警告メッセージを表示: "Warning: 'enum_recreate_allowed' is deprecated. Use '--allow-destructive' instead."
  - フィールドの値は無視（`--allow-destructive` フラグのみで動作を制御）
- **Rationale**:
  - 既存ワークフローを壊さない（エラーにならない）
  - ユーザーに移行を促す（警告表示）
  - 動作が予測可能（フラグのみで制御）
- **Trade-offs**:
  - **Benefits**: 互換性維持、明確な動作、移行促進
  - **Compromises**: 古い設定が残っていても動作するため、移行が遅れる可能性
- **User Decision**: "警告＋無視（推奨）"

### Decision: apply --dry-runの軽量キーワードハイライト（3回目レビュー後決定）

- **Context**: `apply --dry-run` 時に破壊的SQLを視覚的に強調表示する方法
- **Alternatives Considered**:
  1. 軽量キーワードハイライト — 正規表現で検出、精度は保証しない
  2. 完全なSQLパーサ導入 — 高精度だが実装コスト大
  3. 色付けなし — メタデータ情報のみで表示
- **Selected Approach**: **軽量キーワードハイライト**
  - `up.sql` 内の破壊的キーワード（DROP, RENAME, ALTER）を正規表現で検出
  - マッチした行を赤色で強調表示（`colored::Color::Red`）
  - 精度は保証しない（視覚的なヒントとしての位置づけ）
  - 対象パターン: `(?i)\b(DROP\s+(TABLE|COLUMN|TYPE|INDEX|CONSTRAINT)|ALTER\s+.*\s+(DROP|RENAME)|RENAME\s+(TABLE|COLUMN))\b`
- **Rationale**:
  - 低コストで視覚的なフィードバックを提供
  - 誤検出があっても、メタデータが正の情報源（false positive は許容）
  - 旧フォーマットではキーワードハイライトは行わない（メタデータがないため）
- **Trade-offs**:
  - **Benefits**: 低実装コスト、視覚的なヒント、ユーザー体験向上
  - **Compromises**: 誤検出・検出漏れの可能性（あくまでヒント）
- **User Decision**: "軽量キーワードハイライト（推奨）"

### Decision: 旧メタデータ時の最小限情報表示（3回目レビュー後決定）

- **Context**: 旧メタデータ（`destructive_changes` フィールドなし）時のエラー詳細表示
- **Alternatives Considered**:
  1. 最小限の情報のみ — バージョン、警告バナー、フラグ案内
  2. SQL解析で推定表示 — 誤情報リスク
  3. 詳細表示なし — ユーザーに不親切
- **Selected Approach**: **最小限の情報のみ**
  - 表示項目:
    1. マイグレーションバージョン（ファイル名から取得）
    2. 「Legacy migration format detected」警告バナー
    3. `--allow-destructive` 使用の案内
  - 省略項目: 変更種別の詳細（Tables to be dropped: ... 等）
  - 要件2.3, 5.2, 5.4の例外として明記
- **Rationale**:
  - 不正確な情報を表示するより、「不明」を明示する方が安全
  - 最低限の操作ガイダンスは提供
  - 誤解を招くリスクを回避
- **Trade-offs**:
  - **Benefits**: 誤情報回避、明確な動作、安全性
  - **Compromises**: 旧マイグレーションでは詳細な変更内容が分からない
- **User Decision**: "最小限の情報のみ（推奨）"

## Risks & Mitigations

- ~~**Risk 1: SQL解析の誤検出**~~ → **解消**: SQL解析フォールバックを廃止
- **Risk 2: メタデータフォーマット変更の互換性** — 古いバージョンのStrataが新フォーマットを読めない
  - **Mitigation**: Option型でパース、古いメタデータもサポート、バージョン互換性テスト
- ~~**Risk 3: enum_recreate_allowedとの動作不整合**~~ → **解消**: `enum_recreate_allowed` を廃止
- **Risk 4: 古いマイグレーションの適用に追加フラグが必要** — ユーザーの既存ワークフローへの影響
  - **Mitigation**: 明確なエラーメッセージで `--allow-destructive` の使用を案内、ドキュメントで移行ガイドを提供

## References
- [Gap Analysis Report](./gap-analysis.md) — 実装ギャップと推奨アプローチ
- [SchemaDiff Documentation](../../../src/core/src/core/schema_diff.rs) — 差分モデル定義
- [MigrationGenerator Service](../../../src/db/src/services/migration_generator.rs) — マイグレーション生成ロジック
- [Clap Documentation](https://docs.rs/clap/4.5/) — CLI引数パーサ
- [Serde YAML Documentation](https://docs.rs/serde-saphyr/0.0.16/) — YAMLシリアライゼーション

---

generated_at: 2026-01-25T15:52:23Z
updated_at: 2026-01-27T09:00:00Z
