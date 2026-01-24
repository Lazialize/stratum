# Research & Design Decisions

## Summary
- **Feature**: column-rename
- **Discovery Scope**: Extension（既存システムの拡張）
- **Key Findings**:
  - 既存の`Column`構造体に`renamed_from`フィールドを追加することで、YAMLパースを自然にサポート可能
  - MySQL 8.0+では`RENAME COLUMN`構文が使えるが、互換性のため`CHANGE COLUMN`を使用
  - SQLite 3.25.0+で`ALTER TABLE RENAME COLUMN`がネイティブサポート

## Research Log

### 方言別カラムリネームSQL構文

- **Context**: 各データベース方言でカラムリネームをサポートするSQL構文を調査
- **Sources Consulted**:
  - [MySQL 8.0 Reference Manual - ALTER TABLE](https://dev.mysql.com/doc/refman/8.0/en/alter-table.html)
  - [SQLite ALTER TABLE Documentation](https://www.sqlite.org/lang_altertable.html)
- **Findings**:

| 方言 | 構文 | 備考 |
|------|------|------|
| PostgreSQL | `ALTER TABLE t RENAME COLUMN old TO new` | 全バージョン対応 |
| MySQL | `ALTER TABLE t CHANGE old new column_definition` | 全バージョン対応、完全なカラム定義が必要 |
| MySQL 8.0+ | `ALTER TABLE t RENAME COLUMN old TO new` | 型定義不要だが後方互換性のためCHANGEを推奨 |
| SQLite 3.25+ | `ALTER TABLE t RENAME COLUMN old TO new` | 3.25.0以降でサポート |
| SQLite <3.25 | テーブル再作成が必要 | 非推奨、実装対象外 |

- **Implications**:
  - MySQLでは`CHANGE COLUMN`を使用し、完全なカラム定義（型、NULL制約、デフォルト値、AUTO_INCREMENT）を出力する必要がある
  - SQLiteでは3.25.0以降を前提として`RENAME COLUMN`を使用可能
  - リネームと同時に型変更がある場合、操作を分離して順序制御が必要

### 既存コードベースの構造分析

- **Context**: カラムリネーム機能追加に必要な変更箇所を特定
- **Sources Consulted**: ソースコード分析
- **Findings**:
  - `Column`構造体（[src/core/schema.rs:149-165](src/core/schema.rs#L149-L165)）: `renamed_from`フィールドなし
  - `ColumnDiff`構造体（[src/core/schema_diff.rs:332-345](src/core/schema_diff.rs#L332-L345)）: リネーム表現なし
  - `ColumnChange`列挙型（[src/core/schema_diff.rs:437-459](src/core/schema_diff.rs#L437-L459)）: `NameChanged`バリアントなし
  - `SchemaDiffDetector`（[src/services/schema_diff_detector.rs](src/services/schema_diff_detector.rs)）: カラム名ベースで差分検出、リネームは削除+追加として扱われる
  - `SqlGenerator`トレイト（[src/adapters/sql_generator/mod.rs](src/adapters/sql_generator/mod.rs)）: `generate_rename_column`メソッドなし

- **Implications**:
  - 複数レイヤ（モデル→差分検出→SQL生成→CLI）に渡る変更が必要
  - 既存パターンに沿った拡張が可能（ColumnChangeにバリアント追加、SqlGeneratorにメソッド追加）

### リネームと型変更の同時実行順序

- **Context**: カラムリネームと同時に型変更が行われた場合の安全な実行順序を検討
- **Sources Consulted**: 各DBMSのドキュメント、実装経験
- **Findings**:
  - **Up方向（適用）**: リネーム → 型変更の順序が安全
    - リネーム後のカラム名で型変更を実行
    - 理由: 型変更SQLは新しいカラム名を参照する必要がある
  - **Down方向（ロールバック）**: 型変更（逆）→ リネーム（逆）の順序
    - 先に型を元に戻し、その後カラム名を元に戻す
  - **MySQL特殊ケース**: `CHANGE COLUMN`は名前と型を同時に変更可能
    - 単一のSQL文で両方の変更を適用できるため、順序問題を回避できる

- **Implications**:
  - PostgreSQL/SQLiteでは操作を分離して順序制御
  - MySQLでは`CHANGE COLUMN`で一括変更が可能（効率的）

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A: 既存コンポーネント拡張 | Column/ColumnDiff/SqlGeneratorに直接フィールド/メソッド追加 | 既存パターンに沿う、変更箇所が明確 | 広範囲の修正、テスト更新必要 | **推奨** |
| B: 新規サービス作成 | RenameDetector/RenameGeneratorを独立サービスに | 責務分離が明確 | 既存差分検出との統合が複雑 | 過剰設計のリスク |
| C: ハイブリッド | Columnにrenamed_from、差分検出に専用フェーズ追加 | 段階的拡張可能 | 実装フェーズが複数に分かれる | Aに近い |

**選択**: Option A（既存コンポーネント拡張）
- 理由: Stratumの既存アーキテクチャ（Column→ColumnDiff→SqlGenerator）に自然に統合可能

## Design Decisions

### Decision: `renamed_from`属性の導入
- **Context**: カラムリネームをスキーマ定義で表現する方法
- **Alternatives Considered**:
  1. コメント方式（sqldef風）: `# rename: old_name`
  2. 属性方式: `renamed_from: old_name`
  3. 別ファイル管理: リネームマッピングを別YAMLで管理
- **Selected Approach**: 属性方式（`renamed_from`）
- **Rationale**:
  - 型安全で構造化されたデータとして扱える
  - serdeで自然にデシリアライズ可能
  - CI/CD自動化に適している
- **Trade-offs**:
  - マイグレーション適用後に属性の削除が必要（残骸管理）
  - スキーマファイルに一時的なヒントが混在
- **Follow-up**: 属性残存時の警告機能を実装（R4-5）

### Decision: 旧カラム不存在時の挙動
- **Context**: `renamed_from`で指定されたカラムが旧スキーマに存在しない場合の処理
- **Alternatives Considered**:
  1. エラーとして処理を停止
  2. 警告を出して属性を無視
  3. 警告なしで無視
- **Selected Approach**: 警告を出して属性を無視
- **Rationale**:
  - マイグレーション適用済みで属性が残っている場合に対応
  - 厳格すぎるエラーはユーザー体験を損なう
- **Trade-offs**: 属性の残骸が蓄積しやすい
- **Follow-up**: 属性削除推奨の警告を表示（R4-5）

### Decision: MySQL SQL生成方式
- **Context**: MySQLでのカラムリネームSQL生成方式
- **Alternatives Considered**:
  1. `RENAME COLUMN`（MySQL 8.0+専用）
  2. `CHANGE COLUMN`（全バージョン対応）
- **Selected Approach**: `CHANGE COLUMN`
- **Rationale**:
  - MySQL 5.7以前との後方互換性を維持
  - リネームと型変更を単一SQL文で実行可能
  - 既存の`generate_column_definition_for_modify`メソッドを再利用可能
- **Trade-offs**: 完全なカラム定義の出力が必要（複雑さ増加）

### Decision: 無効リネームの差分検出での処理（レビュー指摘対応）
- **Context**: `renamed_from`で指定された旧カラムが存在しない場合、Validator警告だけでなくDiff生成側での挙動を明確化
- **Alternatives Considered**:
  1. Validator → Diff の2段階処理（Validatorで無効マーキング、Diffで参照）
  2. SchemaDiffDetector内で検証込みで処理
- **Selected Approach**: SchemaDiffDetector内で検証込みで処理
- **Rationale**:
  - 単一コンポーネントで完結し、責務が明確
  - 既存の`detect_column_diff`メソッドの拡張として自然に実装可能
- **Behavior**:
  - 旧カラムが存在 → `renamed_columns`に追加
  - 旧カラムが不存在 → 警告を収集、通常の追加/変更として処理
    - 新カラム名が旧スキーマに存在 → `modified_columns`または変更なし
    - 新カラム名が旧スキーマに不存在 → `added_columns`

### Decision: RenamedColumnに旧カラム定義を保持（レビュー指摘対応）
- **Context**: MySQLのCHANGE COLUMNはDown方向で旧カラムの完全定義が必要だが、元の設計では新カラム定義のみ保持
- **Alternatives Considered**:
  1. `RenamedColumn`に`old_column: Column`を追加
  2. GeneratorにoldSchema参照を渡す
  3. `changes`から旧定義を再構成
- **Selected Approach**: `RenamedColumn`に`old_column: Column`を追加
- **Rationale**:
  - 明示的で、Generator側の実装がシンプル
  - 旧カラム定義の再構成ロジックが不要
  - DiffDetector側で旧カラム情報を直接保持できる
- **Trade-offs**: RenamedColumnのサイズが増加するが、メモリ影響は軽微

## Risks & Mitigations

- **リスク1**: MySQL `CHANGE COLUMN`でのカラム定義漏れ（属性欠落でデータ損失）
  - **軽減策**: 既存の`generate_column_definition_for_modify`メソッドを活用し、全属性（型、NULL、デフォルト、AUTO_INCREMENT）を出力
- **リスク2**: SQLite 3.25未満での`RENAME COLUMN`非対応
  - **軽減策**: SQLite 3.25.0以降を前提条件としてドキュメント化（2018年リリース以降は十分普及）
- **リスク3**: リネームと型変更の順序誤りによるマイグレーション失敗
  - **軽減策**: Up方向は「リネーム→型変更」、Down方向は「型変更→リネーム」の順序を強制

## References

- [MySQL 8.0 ALTER TABLE Reference](https://dev.mysql.com/doc/refman/8.0/en/alter-table.html) — CHANGE COLUMN構文の詳細
- [SQLite ALTER TABLE](https://www.sqlite.org/lang_altertable.html) — RENAME COLUMN対応バージョン情報
- [Atlas v0.22 Rename Detection](https://atlasgo.io/blog/2024/05/01/atlas-v-0-22) — 他ツールのリネーム検出アプローチ参考
