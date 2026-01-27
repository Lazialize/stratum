# Implementation Plan

- [ ] 1. UNIQUE・CHECK制約名の生成ヘルパー実装
  - 既存のFK制約名生成ロジック（`fk_`プレフィックス）を汎用化し、UNIQUE制約名（`uq_`プレフィックス）とCHECK制約名（`ck_`プレフィックス）を生成する関数を追加する
  - `{prefix}_{table}_{columns}` 形式で名前を組み立て、63文字（`MAX_IDENTIFIER_LENGTH`）を超える場合はSHA-256ハッシュ付きで切り詰める
  - 同一入力に対して決定論的に同一の制約名を返すことを保証する
  - FK版のハッシュ切り詰めロジックを内部ヘルパーとして共通化し、FK・UNIQUE・CHECKの3種類で共有する
  - ユニットテスト: 正常系（単一カラム、複合カラム）、63文字超のハッシュ切り詰め、決定性検証
  - _Requirements: 1.2, 3.2_

- [ ] 2. PostgreSQL・MySQL方言の制約操作SQL生成
- [ ] 2.1 (P) PostgreSQL方言のUNIQUE・CHECK制約SQL生成
  - UNIQUE制約追加時に `ALTER TABLE ADD CONSTRAINT UNIQUE (columns)` を生成する（単一カラム・複合カラム両対応）
  - CHECK制約追加時に `ALTER TABLE ADD CONSTRAINT CHECK (expression)` を生成する
  - UNIQUE・CHECK制約削除時に `ALTER TABLE DROP CONSTRAINT IF EXISTS` を生成する
  - 制約名はタスク1で実装した生成関数を使用する
  - 識別子は全てダブルクォートで囲む
  - PRIMARY_KEYに対しては引き続き空文字列を返す
  - ユニットテスト: ADD/DROP × UNIQUE/CHECK × 単一/複合カラムの各パターン
  - _Requirements: 1.1, 1.3, 2.1, 3.1, 4.1_

- [ ] 2.2 (P) MySQL方言のUNIQUE・CHECK制約SQL生成
  - UNIQUE制約追加時に `ALTER TABLE ADD CONSTRAINT UNIQUE (columns)` を生成する（単一カラム・複合カラム両対応）
  - CHECK制約追加時に `ALTER TABLE ADD CONSTRAINT CHECK (expression)` を生成する
  - UNIQUE制約削除時に `ALTER TABLE DROP INDEX` を生成する（MySQLではUNIQUEはINDEXとして扱われる）
  - CHECK制約削除時に `ALTER TABLE DROP CHECK` を生成する（MySQL 8.0.16+構文）
  - 制約名はタスク1で実装した生成関数を使用する
  - 識別子は全てバッククォートで囲む
  - ユニットテスト: ADD/DROP × UNIQUE/CHECK の各パターン、特にDROP構文の制約種類別差異を検証
  - _Requirements: 1.1, 1.3, 2.2, 3.1, 4.2_

- [ ] 3. (P) スキーマバリデーションの拡充
  - CHECK制約の `check_expression` が空文字列（トリム後）の場合にバリデーションエラーを報告する
  - 同一テーブル内に同じカラム構成（ソート後比較）のUNIQUE制約が複数定義されている場合に警告を報告する
  - 既存のカラム参照検証（UNIQUE・CHECKの存在しないカラム参照チェック）はそのまま維持する
  - エラーメッセージはテーブル名・カラム名を含む日本語メッセージとする
  - ユニットテスト: 空expression検出、重複UNIQUE検出、正常ケースの非検出
  - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [ ] 4. マイグレーションパイプラインの制約処理統合
- [ ] 4.1 UP方向の制約追加・削除処理
  - `stage_constraint_statements` に `removed_constraints` のDROP処理を追加する
  - PostgreSQL・MySQL方言: 既存の `generate_add_constraint_for_existing_table` ループを UNIQUE・CHECK に対応させ、`removed_constraints` に対して `generate_drop_constraint_for_existing_table` を呼び出す
  - SQLite方言: テーブル単位で制約変更を集約し、同一テーブルにカラム型変更がある場合はステージ3で再作成済みとしてスキップする（`has_type_change` メソッドで判定）
  - SQLite方言: カラム型変更がない場合のみ `SqliteTableRecreator::generate_table_recreation_with_old_table` を呼び出し、1テーブルにつき1回の再作成を保証する
  - _Requirements: 1.1, 1.4, 2.1, 2.2, 2.3, 3.1, 3.3, 4.1, 4.2, 4.3, 5.1, 5.2_

- [ ] 4.2 DOWN方向の逆操作処理
  - `generate_down` に `removed_constraints` のADD処理を追加する（削除された制約をロールバック時に復元）
  - PostgreSQL・MySQL方言: `added_constraints` → `generate_drop_constraint_for_existing_table`（既存拡張）、`removed_constraints` → `generate_add_constraint_for_existing_table`（新規）
  - SQLite方言: DOWN方向では `old_schema` のテーブル定義を `new_table`、`new_schema` のテーブル定義を `old_table` としてテーブル再作成を実行する
  - SQLite方言: カラム型変更との重複再作成回避をDOWN方向でも同様に適用する
  - _Requirements: 2.4, 2.5, 4.4, 4.5, 5.1_

- [ ] 5. 統合テスト・エッジケース検証
- [ ] 5.1 3方言のパイプラインUP/DOWN統合テスト
  - UNIQUE制約追加のUP SQL生成を3方言（PostgreSQL, MySQL, SQLite）で検証する
  - CHECK制約追加のUP SQL生成を3方言で検証する
  - UNIQUE・CHECK制約削除のUP SQL生成を3方言で検証する
  - 各UP操作に対するDOWN SQLの逆操作が正しく生成されることを検証する
  - カラム追加 + UNIQUE制約追加が同一マイグレーションに含まれるケースを検証する
  - dry-runモードでのプレビュー表示が既存フローで動作することを確認する
  - _Requirements: 1.1, 1.4, 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.3, 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 5.3_

- [ ] 5.2 SQLiteエッジケース・混合変更テスト
  - 同一テーブルに複数の制約変更（UNIQUE追加 + CHECK追加）が同時に存在するケースで、テーブル再作成が1回に集約されることを検証する
  - SQLiteでカラム型変更 + 制約変更が同一テーブルに同時発生するケースで、ステージ5でスキップされることを検証する
  - SQLite DOWN方向のロールバックで `old_schema`/`new_schema` の入れ替えが正しく機能することを検証する
  - 長い制約名（63文字超）がハッシュ切り詰めされた場合でも、パイプライン全体が正常に動作することを検証する
  - _Requirements: 1.4, 2.3, 3.3, 4.3, 5.1, 5.2_

## 要件カバレッジ

| 要件ID | タスク |
|--------|--------|
| 1.1 | 2.1, 2.2, 4.1, 5.1 |
| 1.2 | 1 |
| 1.3 | 2.1, 2.2 |
| 1.4 | 4.1, 5.1, 5.2 |
| 2.1 | 2.1, 4.1, 5.1 |
| 2.2 | 2.2, 4.1, 5.1 |
| 2.3 | 4.1, 5.1, 5.2 |
| 2.4 | 4.2, 5.1 |
| 2.5 | 4.2, 5.1 |
| 3.1 | 2.1, 2.2, 4.1, 5.1 |
| 3.2 | 1 |
| 3.3 | 4.1, 5.1, 5.2 |
| 4.1 | 2.1, 4.1, 5.1 |
| 4.2 | 2.2, 4.1, 5.1 |
| 4.3 | 4.1, 5.1, 5.2 |
| 4.4 | 4.2, 5.1 |
| 4.5 | 4.2, 5.1 |
| 5.1 | 4.1, 4.2, 5.1, 5.2 |
| 5.2 | 4.1, 5.1, 5.2 |
| 5.3 | 5.1 |
| 6.1 | 3（既存実装維持） |
| 6.2 | 3（既存実装維持） |
| 6.3 | 3 |
| 6.4 | 3 |
