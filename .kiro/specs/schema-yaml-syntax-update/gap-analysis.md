# Gap Analysis: schema-yaml-syntax-update

**作成日**: 2026-01-24T00:00:00Z
**仕様ID**: schema-yaml-syntax-update
**分析対象**: 既存コードベースと要件の実装ギャップ

---

## Analysis Summary

- YAMLのテーブル名は `tables` キー名で管理する想定だが、現状は `Table.name` を必須とするSerde前提で冗長性が残っている
- 主キーは `constraints` の `PRIMARY_KEY` として扱われており、`primary_key` 独立フィールドへの移行が未対応
- `indexes` / `constraints` は必須フィールド扱いで、未定義時にデシリアライズエラーとなる可能性が高い
- シリアライザ（`serde_saphyr::to_string`）が `name` と `PRIMARY_KEY` を出力するため、要件の出力形式と乖離
- エラーメッセージの行番号・フィールドパス付与は現状のパーサー/バリデータに明示実装がなく要検討

## Document Status

- `.kiro/settings/rules/gap-analysis.md` の枠組みに基づき、関連資産・要件ギャップ・実装オプション・リスクを整理済み
- 要件は未承認（spec.json: requirements approved=false）だが、分析は継続

## Next Steps

- 設計フェーズで「互換性維持（旧フォーマット許容）」と「厳格移行」の方針を明確化
- `/prompts:kiro-spec-design schema-yaml-syntax-update` で設計文書を作成

---

## 1. 現状調査 (Current State Investigation)

### 1.1 関連資産

**スキーマモデル** (`src/core/schema.rs`)
- `Schema.tables: HashMap<String, Table>` と `Table.name` の二重管理
- `Table.indexes` / `Table.constraints` は `#[serde(default)]` がなく必須
- `Constraint::PRIMARY_KEY` を保持し、`Table.get_primary_key_columns()` は constraints を参照

**スキーマパーサー** (`src/services/schema_parser.rs`)
- `serde_saphyr::from_str` による自動デシリアライズ
- テーブル名のキー利用や `primary_key` 変換のロジックは存在しない

**スキーマバリデーター** (`src/services/schema_validator.rs`)
- PK存在チェックは `constraints` 依存
- 列存在チェックは `constraints` と `indexes` を前提
- `ErrorLocation` は line/column に対応可能だが parser からの情報連携がない

**シリアライゼーション**
- `serde_saphyr::to_string` が `Table.name` と `constraints` をそのまま出力
- `schema_checksum` は `table.name` と `constraints` を正規化対象に含む

**CLI/生成系**
- `generate` の `.schema_snapshot.yaml` 保存時も現行フォーマット
- `export` は `Table::new` + `constraints` で PRIMARY KEY を出力

**例示ファイル**
- `example/schema/*.yaml` は `name` と `constraints` / `indexes` 必須の現行構文

### 1.2 慣習とパターン

- YAMLはSerdeで直接 `Schema` へデシリアライズ（構文変換レイヤーなし）
- PK/制約/インデックスは `Table.constraints/indexes` に集約
- YAMLエラーは `anyhow` で返却し、ValidationErrorへの変換は実装なし

### 1.3 統合ポイント

- YAML → `SchemaParserService` → `SchemaValidatorService` → SQL Generator
- `schema_checksum` / `schema_diff` / `migration_generator` が `Table.constraints` に依存

---

## 2. 要件実現性分析 (Requirements Feasibility)

### Requirement 1: テーブル名としてYAMLキー名を使用

**必要な機能**
- `tables` キー名を `Table.name` にマッピング
- `name` フィールドを入力/出力から排除

**現状のギャップ**
- ❌ `Table.name` が必須で、キー名からの補完がない
- ❌ Serializer が `name` を出力する
- ⚠️ `schema_checksum`/`schema_snapshot`/例示ファイルの更新が必要

### Requirement 2: 主キー（primary_key）のconstraintsからの独立

**必要な機能**
- `primary_key` のパース → 内部 `Constraint::PRIMARY_KEY` への変換
- `constraints` 内 `PRIMARY_KEY` の廃止
- Serializer が `primary_key` を出力し、`constraints` から除外

**現状のギャップ**
- ❌ `Table` に `primary_key` フィールドが存在しない
- ❌ パーサー/ジェネレーター/バリデーターが `constraints` 前提
- ⚠️ 旧 `PRIMARY_KEY` の互換受け入れ方針が未定（移行戦略が必要）

### Requirement 3: indexesフィールドのオプショナル化

**必要な機能**
- `indexes` 未定義を空配列として扱う

**現状のギャップ**
- ❌ `Table.indexes` に `#[serde(default)]` がなく欠如で失敗する可能性

### Requirement 4: constraintsフィールドのオプショナル化

**必要な機能**
- `constraints` 未定義を空配列として扱う
- `primary_key` があれば制約として扱う

**現状のギャップ**
- ❌ `Table.constraints` に `#[serde(default)]` がなく欠如で失敗する可能性
- ❌ `primary_key` の扱いが未実装

### Requirement 5: エラーメッセージとバリデーション

**必要な機能**
- `primary_key` に未定義カラムがある場合の明確なメッセージ
- `columns` 欠落時の必須エラー
- 構文エラー時に行番号またはフィールドパスを含める

**現状のギャップ**
- ✅ `columns` 空はバリデータで検出可能
- ✅ 制約カラムの存在チェックは実装済み（メッセージにカラム名含む）
- ❌ `primary_key` 由来の検証が未実装
- ⚠️ `serde_saphyr` の行番号情報を `ValidationError` へ反映する実装がない

---

## 3. 実装アプローチの選択肢

### Option A: 既存コンポーネント拡張

- `SchemaParserService` に前処理/後処理を追加し、`tables` キー名で `Table.name` を補完
- `Table` に `primary_key` を追加し、serdeのカスタム(De)Serializeで `constraints` へ変換
- `Table.indexes/constraints` に `#[serde(default)]`

**トレードオフ**
- ✅ 既存構造に沿った最小変更
- ✅ 依存箇所の影響範囲を抑制
- ❌ Serdeカスタム実装が複雑化する可能性

### Option B: 新しいパース/モデル層の導入

- YAML用DTO（`SchemaYaml`/`TableYaml`）を新設
- DTO → ドメインモデルへの変換で `primary_key`/name 補完を集中管理

**トレードオフ**
- ✅ YAML仕様変更を隔離できる
- ✅ 互換フォーマット受入れの拡張が容易
- ❌ 追加の型・変換コードが必要

### Option C: ハイブリッド

- DTOでYAMLフォーマットを受け取り、内部は現行モデル継続
- Serializer側のみ DTO に変換して出力形式を制御

**トレードオフ**
- ✅ 既存サービスの影響を抑えつつ出力要件を満たせる
- ❌ 入力/出力の二重変換が増える

---

## 4. 実装規模とリスク

- **Effort**: M（3–7日）
  - 理由: Serde変更 + DTO/変換 + 既存テスト/例示/スナップショット更新が必要
- **Risk**: Medium
  - 理由: YAML互換性と `schema_snapshot` 形式変更の影響が読めない

---

## 5. Requirement-to-Asset Map

| 要件 | 既存コンポーネント | ギャップ | 備考 |
| --- | --- | --- | --- |
| R1 テーブル名キー化 | `Schema`/`Table`/`SchemaParserService` | Missing | `Table.name` 必須/serializer出力が阻害要因 |
| R2 primary_key独立 | `Constraint`/`SqlGenerator`/`SchemaValidator` | Missing | `constraints` 前提の全面見直しが必要 |
| R3 indexesオプショナル | `Table`/`SchemaParserService` | Missing | `#[serde(default)]` 追加で対応可能 |
| R4 constraintsオプショナル | `Table`/`SchemaParserService` | Missing | primary_key連携の方針決定が必要 |
| R5 エラーメッセージ | `SchemaValidator`/`ErrorLocation` | Unknown | `serde_saphyr` の行番号提供可否は要調査 |

---

## 6. Designフェーズへの引き継ぎ

**優先検討アプローチ**
- Option B/C を中心に、YAMLフォーマットとドメインモデルを分離する方針を検討

**Research Needed**
- `serde_saphyr` のエラー情報（行番号・フィールドパス）取得方法
- 旧フォーマット（`name` / `constraints` PRIMARY_KEY）互換性の扱い方針
- `schema_snapshot` の後方互換（旧スナップショット読み込み可否）
