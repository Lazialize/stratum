# リサーチ & 設計決定ログ

## サマリー
- **機能**: `unique-check-constraints`
- **ディスカバリスコープ**: Extension（既存システムの拡張）
- **主要な発見**:
  - SQL Generatorの `generate_add/drop_constraint_for_existing_table` メソッドはFOREIGN KEYのみ対応。UNIQUE・CHECKアームの追加で対応可能
  - マイグレーションパイプラインは `removed_constraints` を**全く処理していない**（UP: DROP文未生成、DOWN: ADD文未生成）。`added_constraints` のみ処理済み
  - SQLiteはテーブル再作成パターンが実装済み（`SqliteTableRecreator`）。制約変更にも再利用可能だが、パイプラインからの呼び出しパス設計が必要

## リサーチログ

### パイプラインにおける制約削除処理の欠落
- **コンテキスト**: `removed_constraints` がdiff検出で生成されるが、パイプラインで消費されていない
- **ソース**: `src/db/src/services/migration_pipeline/mod.rs`（generate_up: L97-150, generate_down: L159-267）、`index_constraint_stages.rs`（stage_constraint_statements: L25-42）
- **発見**:
  - `stage_constraint_statements` は `added_constraints` のみ反復処理
  - `generate_down` も `added_constraints` の逆処理（DROP）のみ実行
  - `removed_constraints` は差分検出器（`constraint_comparator.rs:27`）で生成されるが、パイプラインのどのステージでも参照されない
- **影響**: UP マイグレーションでの制約削除SQL生成、DOWN マイグレーションでの削除された制約の復元SQL生成が必要。パイプラインの拡張が必須

### MySQL方言固有のDROP構文
- **コンテキスト**: MySQL は制約種類ごとに異なるDROP構文を要求する
- **ソース**: MySQL公式ドキュメント、既存コード（`mysql.rs:303-307`でFKは `DROP FOREIGN KEY` 使用）
- **発見**:
  - UNIQUE制約削除: `ALTER TABLE ... DROP INDEX constraint_name`
  - CHECK制約削除: `ALTER TABLE ... DROP CHECK constraint_name`（MySQL 8.0.16+）
  - FOREIGN KEY削除: `ALTER TABLE ... DROP FOREIGN KEY constraint_name`（実装済み）
  - PostgreSQLは全て `DROP CONSTRAINT` で統一
- **影響**: MySQL Generator では制約種類ごとに異なるDROP構文を生成する必要がある

### SQLiteテーブル再作成パターンの制約変更適用
- **コンテキスト**: SQLiteは `ALTER TABLE ADD/DROP CONSTRAINT` をサポートしない
- **ソース**: `sqlite_table_recreator.rs`、`sqlite.rs:144`
- **発見**:
  - `SqliteTableRecreator` は既に制約を含むCREATE TABLEを生成可能（L132-137）
  - `generate_table_recreation_with_old_table` メソッドで旧・新テーブル定義を受け取りテーブル再作成可能
  - 現在の呼び出しパス: カラム型変更時に `sqlite.rs:169` から呼び出し
  - 制約変更時も同様のパターンで呼び出し可能だが、**パイプラインから新旧テーブル定義を渡す仕組みが必要**
- **影響**: `SqlGenerator` トレイトに制約変更専用メソッド追加、またはパイプラインステージでSQLite分岐処理が必要

### 制約名生成関数の設計
- **コンテキスト**: UNIQUE用 `uq_` とCHECK用 `ck_` プレフィックスの命名関数が必要
- **ソース**: `mod.rs:37-66`（`generate_fk_constraint_name`）
- **発見**:
  - FK命名関数のパターン: `fk_{table}_{columns}_{ref_table}`、63文字超はSHA-256ハッシュ付き切り詰め
  - UNIQUE: `uq_{table}_{columns}` — 参照テーブルなし（FK と異なり、自テーブルのカラムのみ）
  - CHECK: `ck_{table}_{columns}` — 同上
  - ハッシュ切り詰めロジックは共通化可能
- **影響**: `generate_uq_constraint_name` と `generate_ck_constraint_name` を新規作成。ハッシュ切り詰めは内部ヘルパーとして共有

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク・制限 | 備考 |
|---|---|---|---|---|
| A: 既存拡張 | 各SQL Generatorのmatch式拡張 + パイプラインの制約削除処理追加 | パターン一貫性、最小変更 | match式の肥大化（軽微） | **推奨** |
| B: 新モジュール | 制約操作専用モジュール | 集約された制約ロジック | 方言差異により分離効果薄、過度な抽象化 | 非推奨 |
| C: ハイブリッド | PG/MySQL既存拡張 + SQLite専用ヘルパー | SQLite特殊処理の分離 | 計画やや複雑 | SQLite部分のみ検討価値あり |

## 設計決定

### 決定: 既存コンポーネント拡張アプローチの採用
- **コンテキスト**: UNIQUE・CHECK制約の追加/削除マイグレーション生成を実装する
- **検討した代替案**:
  1. Option A — 各SQL Generatorのmatch式にUNIQUE・CHECKアームを追加
  2. Option B — 制約操作専用モジュールを新設
  3. Option C — PostgreSQL/MySQL は拡張、SQLiteは専用ヘルパー
- **選択したアプローチ**: Option A（既存コンポーネント拡張）
- **理由**: FK制約の実装パターンがそのまま踏襲可能。パイプラインの変更も `stage_constraint_statements` への `removed_constraints` 処理追加のみ。新規ファイル不要
- **トレードオフ**: 各ジェネレーターのmatch式が若干拡大するが、一貫性と保守性で優位
- **フォローアップ**: SQLiteのテーブル再作成パイプライン呼び出しの詳細設計

### 決定: パイプラインでの制約削除処理の追加
- **コンテキスト**: `removed_constraints` がパイプラインで未処理
- **検討した代替案**:
  1. `stage_constraint_statements` に `removed_constraints` の DROP 処理を追加
  2. 新ステージ `stage_constraint_drop_statements` を新設
- **選択したアプローチ**: Option 1 — 既存ステージに統合
- **理由**: 制約の追加と削除は同一ステージで処理するのが自然。インデックスも `stage_index_statements` で一括処理のパターンに合致
- **トレードオフ**: ステージの責務がやや広がるが、制約操作という同一ドメイン内

### 決定: SQLite制約変更の呼び出しパス
- **コンテキスト**: SQLiteは ALTER TABLE ADD/DROP CONSTRAINT 非対応のためテーブル再作成が必要
- **選択したアプローチ**: `generate_add/drop_constraint_for_existing_table` メソッドでテーブル再作成SQLを `Vec<String>` として返すよう、トレイトのシグネチャを変更するか、SQLite専用の分岐でパイプラインからテーブル定義を渡す
- **理由**: 既存の `generate_alter_column_type` がカラム型変更で同様のパターンを実現済み
- **フォローアップ**: トレイトのシグネチャ変更の影響範囲を実装時に確認

## リスクと緩和策
- **リスク1**: SQLiteのテーブル再作成で既存データが失われる可能性 → **緩和**: トランザクション内で実行（既存パターン踏襲）
- **リスク2**: MySQL 8.0.16未満でCHECK制約が無視される → **緩和**: ドキュメント記載で対応（MySQL側の仕様制限）
- **リスク3**: パイプラインのシグネチャ変更が後方互換性に影響 → **緩和**: 既存のString返却パターンを維持し、SQLiteのみVec返却する設計

## リファレンス
- [PostgreSQL ALTER TABLE](https://www.postgresql.org/docs/current/sql-altertable.html) — ADD/DROP CONSTRAINT構文
- [MySQL ALTER TABLE](https://dev.mysql.com/doc/refman/8.0/en/alter-table.html) — DROP INDEX/DROP CHECK構文
- [SQLite ALTER TABLE](https://www.sqlite.org/lang_altertable.html) — 制限事項の公式ドキュメント
