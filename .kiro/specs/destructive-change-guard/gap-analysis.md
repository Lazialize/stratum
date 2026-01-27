# Gap Analysis: destructive-change-guard

**作成日**: 2026-01-25T15:48:08Z
**仕様ID**: destructive-change-guard
**分析対象**: 既存コードベースと要件の実装ギャップ
**注意**: requirements 未承認（分析は継続）

---

## Analysis Summary

- `SchemaDiff` と `SchemaDiffDetector` により破壊的変更の元データは揃っているが、専用の検出サービス/レポートが未実装
- `generate`/`apply` にはデフォルト拒否・許可フラグ・警告表示の仕組みが存在せず、要件2/3/5の実装ギャップが大きい
- `apply` はマイグレーションSQLしか見ないため、破壊的変更判定の情報源を設計段階で決める必要がある
- `MigrationPipeline` には ENUM 再作成のガードがあるが、他の破壊的変更には適用されていない

## Document Status

- gap-analysis.md のフレームワークに従って現状調査・要件実現性・オプション比較・マッピングを実施
- 追加リサーチは設計フェーズで実施する前提

## Next Steps

- このギャップ分析を踏まえ `/prompts:kiro-spec-design destructive-change-guard` を実行
- 許可フラグと `apply` 側の検出源（SQL解析/メタデータ/スナップショット）を設計で決定

---

## 1. 現状調査 (Current State Investigation)

### 1.1 関連資産

#### 主要コンポーネント

**スキーマ差分モデル** (`src/core/src/core/schema_diff.rs`)
- `SchemaDiff.removed_tables`, `TableDiff.removed_columns`, `TableDiff.renamed_columns`, `SchemaDiff.removed_enums`, `EnumChangeKind::Recreate` を保持
- 破壊的変更判定に必要な名称情報（テーブル名/カラム名/ENUM名）は既に保持可能

**差分検出** (`src/db/src/services/schema_diff_detector.rs`)
- テーブル削除・カラム削除・カラムリネーム・ENUM削除・ENUM再作成を差分として抽出可能
- リネームは `renamed_from` 属性から検出し `RenamedColumn` に集約

**マイグレーション生成パイプライン** (`src/db/src/services/migration_pipeline.rs`)
- `enum_recreate_allowed` が false の場合、ENUM再作成/削除をエラーで拒否（既存の安全ガード）
- `removed_tables` は DROP TABLE として生成
- `removed_columns` の DROP COLUMN は生成されていない（SQL Generator自体はDROP COLUMN対応）

**CLIコマンド**
- `generate` (`src/cli/src/cli/commands/generate.rs`): dry-runでSQL/型変更/リネームのプレビュー表示
- `apply` (`src/cli/src/cli/commands/apply.rs`): dry-runは既存の `up.sql` をそのまま表示
- `--allow-destructive` フラグは未定義（`src/cli/src/cli.rs`）

**SQL分割ユーティリティ** (`src/cli/src/cli/commands/mod.rs`)
- `split_sql_statements` が `apply`/`rollback` で利用され、SQL文単位の解析が可能

#### 既存の慣習とパターン

- 差分検出 → マイグレーション生成の流れが一貫 (`SchemaDiffDetector` → `MigrationGenerator` → `MigrationPipeline`)
- CLI側で出力整形を実装しており、共通のメッセージフォーマッタは存在しない
- 色付けは `colored` を直接利用（`no_color` フラグは未活用）

#### 統合ポイント

- `generate`: スナップショット + スキーマ読み込み → `SchemaDiffDetector` → `MigrationGenerator`
- `apply`: `migrations/` 内の `up.sql`/`.meta.yaml` のみを参照（`SchemaDiff` にはアクセスしない）

---

## 2. 要件実現性分析 (Requirements Feasibility)

### 2.1 技術的要求事項とギャップ

#### 要件1: 破壊的変更の検出と分類
- **既存資産**: `SchemaDiff` と `SchemaDiffDetector` が該当差分を保持
- **ギャップ**:
  - ❌ `DestructiveChangeDetector` サービスが存在しない
  - ❌ `DestructiveChangeReport` モデルが存在しない
  - ⚠️ `apply` 側で利用可能な差分情報がない

#### 要件2: デフォルト拒否メカニズム
- **既存資産**: ENUM再作成のみ `enum_recreate_allowed` で拒否可能
- **ギャップ**:
  - ❌ `generate` で破壊的変更の検出結果に基づく拒否ロジックがない
  - ❌ `apply` でマイグレーション実行前に検出・拒否する仕組みがない

#### 要件3: 明示的な許可フラグ
- **ギャップ**:
  - ❌ CLI引数 `--allow-destructive` が存在しない
  - ❌ `generate`/`apply` の許可フラグによる分岐がない
  - ⚠️ 既存の `enum_recreate_allowed` と新フラグの整合性方針が未定

#### 要件4: dry-run差分プレビューの拡張
- **既存資産**: `generate --dry-run` のリネーム/型変更プレビュー
- **ギャップ**:
  - ❌ 破壊的変更の専用セクション表示がない
  - ❌ `apply --dry-run` で破壊的SQLを強調表示できない
  - ❌ 「--allow-destructive で続行」ガイダンスの表示がない

#### 要件5: エラーメッセージと修正提案
- **既存資産**: `generate` がエラーを返す仕組みはあるが、破壊的変更向けの整形はない
- **ギャップ**:
  - ❌ 変更種別のグルーピング表示、具体例コマンドの提示がない
  - ❌ `apply` の拒否時にマイグレーションバージョンの明示がない
  - ❌ 色付けされた破壊的エラー表示の統一がない

### 2.2 非機能要件・制約

- **性能**: 既存の差分検出は実行済みのため、検出サービスは `SchemaDiff` の再利用で低オーバーヘッド化可能
- **後方互換性**: `apply` が差分情報を持たないため、拒否判定の情報源を変更すると互換性影響が大きい
- **セキュリティ**: 既存のSQL実行は `sqlx` を利用、破壊的検出はSQL解析のみで完結可能
- **制約**: `removed_columns` がUP SQLに反映されていないため、実際の破壊的挙動と差分結果が一致しない可能性

**Research Needed**:
1. `apply` 時の破壊的変更判定の情報源設計（SQL解析 vs メタデータ拡張 vs スナップショット再比較）
2. `DROP`/`RENAME` のSQL判定ルール（方言差・複合DDL対応）
3. `enum_recreate_allowed` と `--allow-destructive` の優先順位方針

---

## 3. 実装アプローチオプション

### Option A: 既存CLIに検出ロジックを内包（Extend Existing Components）

**概要**: `generate`/`apply` の中で直接 `SchemaDiff`/SQL解析を使って破壊的変更を検出

**変更対象**:
- `src/cli/src/cli.rs`: `--allow-destructive` 追加
- `src/cli/src/cli/commands/generate.rs`: diff走査による拒否・警告・プレビュー
- `src/cli/src/cli/commands/apply.rs`: SQL文解析による拒否・プレビュー

**Trade-offs**:
- ✅ 既存フローに最小限の追加で済む
- ✅ 新規型の導入が少ない
- ❌ 破壊的検出の責務がCLIに集中し肥大化しやすい
- ❌ `apply` のSQL解析精度に依存する

### Option B: 検出サービスとレポートを新設（Create New Components）

**概要**: `DestructiveChangeDetector` と `DestructiveChangeReport` をサービス層に追加し、`generate` と `apply` が共通利用

**新規コンポーネント**:
- `src/db/src/services/destructive_change_detector.rs`
- `src/core/src/core/destructive_change_report.rs`
- `.meta.yaml` への破壊的変更メタデータ拡張（apply側で参照）

**Trade-offs**:
- ✅ ロジックが集中し、再利用性が高い
- ✅ `apply` での判定精度をメタデータで保証可能
- ❌ メタデータ互換性の検討が必要
- ❌ 既存マイグレーションへの後方互換が課題

### Option C: ハイブリッド（Hybrid Approach）

**概要**: `generate` では `SchemaDiff` ベースの検出サービスを使い、`apply` はメタデータが無い場合にSQL解析でフォールバック

**組み合わせ戦略**:
- 生成時に `DestructiveChangeReport` を作成し `.meta.yaml` に保存
- 既存マイグレーションにはSQL解析で限定的に対応

**Trade-offs**:
- ✅ 新旧マイグレーションに段階対応できる
- ✅ 検出ロジックをサービス化しつつ、互換性を確保
- ❌ 2系統の判定ロジックを保守する必要がある

---

## 4. 実装複雑性とリスク評価

### 工数見積もり
- **Option A**: **M (3-7 days)**
  - 理由: CLI変更のみだがSQL解析の設計が必要
- **Option B**: **M〜L (3-10 days)**
  - 理由: 新規モデル/サービス + メタデータ拡張 + 互換性検討
- **Option C**: **M (3-7 days)**
  - 理由: 生成時サービス化 + applyのフォールバックで段階移行

### リスク評価
- **Option A**: Medium
  - 理由: SQL解析の曖昧さと責務肥大化
- **Option B**: Medium
  - 理由: メタデータ拡張による互換性リスク
- **Option C**: Medium-Low
  - 理由: 互換性と新規設計のバランスが取りやすい

---

## 5. 設計フェーズへの推奨事項 (Recommendations)

### 5.1 推奨アプローチ

**Option C (ハイブリッド)** を推奨
- 既存マイグレーションとの互換性を保ちつつ、検出サービスを導入できるため

### 5.2 設計フェーズで決定すべき事項

1. `DestructiveChangeReport` の保存先（`.meta.yaml` 拡張 or 別ファイル）
2. `apply` 側の判定優先順位（メタデータ優先か、SQL解析優先か）
3. `enum_recreate_allowed` と `--allow-destructive` の整合性
4. 破壊的変更の色付き表示の標準フォーマット

### 5.3 設計フェーズで実施すべきリサーチ

1. DDLの簡易解析で十分か、パーサ導入が必要か
2. 破壊的SQLの検出パターン（DROP TABLE/TYPE/COLUMN、RENAME COLUMN 等）
3. 既存 `ValidationWarning`/`ValidationError` の活用可否

---

## 6. 要件とコンポーネントのマッピング

| 要件 | 既存コンポーネント | ギャップ | 備考 |
|------|-------------------|---------|------|
| Req 1.1 破壊的変更の検出 | `SchemaDiffDetector` | ✅ Reusable | 差分情報は取得済み |
| Req 1.2 影響範囲のリスト化 | `SchemaDiff` | ⚠️ Constraint | 集約レポートが未実装 |
| Req 1.3 複数変更の検出 | `SchemaDiff` | ✅ Reusable | 複数変更を保持可能 |
| Req 1.4 検出サービス化 | - | ❌ Missing | `DestructiveChangeDetector` が必要 |
| Req 1.5 レポート返却 | - | ❌ Missing | `DestructiveChangeReport` が必要 |
| Req 2.1 generate拒否 | `GenerateCommandHandler` | ❌ Missing | 破壊的判定がない |
| Req 2.2 apply拒否 | `ApplyCommandHandler` | ❌ Missing | SQL/メタデータ判定が必要 |
| Req 2.3 一覧表示 | - | ❌ Missing | フォーマッタ未実装 |
| Req 2.4 許可フラグ案内 | - | ❌ Missing | メッセージテンプレート必要 |
| Req 3.1/3.2 許可フラグ | `cli.rs` | ❌ Missing | CLI引数追加が必要 |
| Req 3.5 警告表示 | `GenerateCommandHandler` | ❌ Missing | 破壊的警告セクションが必要 |
| Req 4.1/4.3 プレビュー強調 | `generate`/`apply` | ❌ Missing | 色付き表示の拡張が必要 |
| Req 5.1/5.2 エラーメッセージ | - | ❌ Missing | 詳細・提案付きの整形が必要 |
| NFR パフォーマンス | `SchemaDiff` 再利用 | ✅ Reusable | 追加オーバーヘッドは小さい |
| NFR 互換性 | `apply` の情報不足 | ⚠️ Constraint | 判定情報の設計が必須 |

**凡例**:
- ✅ **Reusable**: 既存コンポーネントを流用可能
- ⚠️ **Constraint**: 既存拡張が必要
- ❌ **Missing**: 新規作成が必要

---

## 7. まとめ

- 破壊的変更の基礎データは `SchemaDiff` に揃っているが、専用検出サービスとユーザー通知が欠落
- `apply` が差分情報を持たないため、検出情報の保存/解析戦略が設計の主要論点
- ENUM再作成だけは既存で拒否可能だが、他の破壊的変更は未対処

**技術的リスク**: Medium
**実装工数**: M
