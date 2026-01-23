# Implementation Plan

## Tasks

- [ ] 1. 型カテゴリ分類と検証基盤の構築
- [ ] 1.1 (P) 型カテゴリ列挙型とカテゴリ判定機能を実装する
  - ColumnTypeから型カテゴリ（Numeric, String, DateTime, Binary, Json, Boolean, Uuid, Other）を分類する機能を追加
  - カテゴリ間の変換が警告対象かエラー対象かを判定するメソッドを実装
  - 型互換性マトリクスに基づいた変換ルールを定義
  - _Requirements: 3.1, 4.1, 4.2, 4.3_

- [ ] 1.2 (P) マイグレーション方向を表す列挙型を追加する
  - up/down方向を区別するためのMigrationDirection列挙型を定義
  - 各SqlGenerator実装で方向に応じた適切なSQL生成ができるよう準備
  - _Requirements: 2.1, 2.2_

- [ ] 2. 型変更検証サービスの実装
- [ ] 2.1 型変更の互換性検証機能を実装する
  - TypeChangeValidatorサービスを新規作成
  - カラム差分リストを受け取り、各型変更の互換性を検証
  - 型カテゴリベースで警告（データ損失リスク）とエラー（互換性なし）を判定
  - 検証結果をValidationResultとして集約して返却
  - _Requirements: 3.2, 4.1, 4.3_

- [ ] 2.2 精度損失の検証機能を実装する
  - VARCHAR/DECIMAL等のサイズ縮小を検出する機能を追加
  - サイズ縮小時に警告メッセージを生成
  - テーブル名・カラム名を含む位置情報付きの警告を出力
  - _Requirements: 4.2_

- [ ] 3. SqlGeneratorトレイトの拡張
- [ ] 3.1 ALTER COLUMN TYPE生成APIをトレイトに追加する
  - SqlGeneratorトレイトにgenerate_alter_column_typeメソッドを追加
  - テーブル定義、カラム差分、マイグレーション方向を引数として受け取る
  - デフォルト実装で空のVecを返し、既存実装への影響を最小化
  - _Requirements: 2.1, 2.2_

- [ ] 4. PostgreSQL型変更SQL生成の実装
- [ ] 4.1 (P) PostgreSQL用のALTER COLUMN TYPE文を生成する
  - PostgresSqlGeneratorにgenerate_alter_column_typeを実装
  - ALTER TABLE ... ALTER COLUMN ... TYPE ... 構文でSQL生成
  - 型カテゴリベースでUSING句の要否を判定（String→Numeric等は必要）
  - up/down両方向のSQL生成に対応
  - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.3_

- [ ] 5. MySQL型変更SQL生成の実装
- [ ] 5.1 (P) MySQL用のMODIFY COLUMN文を生成する
  - MysqlSqlGeneratorにgenerate_alter_column_typeを実装
  - ALTER TABLE ... MODIFY COLUMN ... 構文でSQL生成
  - テーブル定義から対象カラムの完全な定義（NULL制約、DEFAULT値等）を取得して含める
  - up/down両方向のSQL生成に対応
  - _Requirements: 2.1, 2.2, 2.4, 3.1, 3.3_

- [ ] 6. SQLiteテーブル再作成の実装
- [ ] 6.1 SQLiteテーブル再作成サービスを実装する
  - SqliteTableRecreatorを新規作成
  - 外部キー制約の一時無効化（PRAGMA foreign_keys=off）を含む
  - トランザクション内での新テーブル作成、データコピー、旧テーブル削除、リネームを実装
  - インデックスと制約の再作成、外部キー整合性チェックを含む
  - _Requirements: 2.5_

- [ ] 6.2 列交差ベースのデータコピーを実装する
  - old_schemaとnew_schemaの共通カラムを特定する機能を追加
  - 明示的なカラムリストでINSERT INTO ... SELECT文を生成
  - 追加列はDEFAULT値またはNULLで自動補完（NOT NULL + DEFAULTなしは事前エラー検出）
  - 削除列はSELECTリストから除外
  - _Requirements: 2.5_

- [ ] 6.3 SQLite用のgenerate_alter_column_typeを実装する
  - SqliteSqlGeneratorにgenerate_alter_column_typeを追加
  - SqliteTableRecreatorへの委譲を実装
  - up/down両方向でテーブル再作成SQLを生成
  - _Requirements: 2.1, 2.2, 2.5_

- [ ] 7. MigrationGeneratorの拡張
- [ ] 7.1 旧/新スキーマ注入によるSQL生成を実装する
  - generate_up_sql/generate_down_sqlのシグネチャを拡張してold_schema/new_schemaを受け取る
  - modified_columnsの各カラムについて型変更があればgenerate_alter_column_typeを呼び出し
  - up方向ではnew_schema、down方向ではold_schemaからテーブル定義を取得
  - _Requirements: 2.1, 2.2_

- [ ] 7.2 型変更検証とSQL生成の統合を実装する
  - SQL生成前にTypeChangeValidatorで型変更を検証
  - エラーがあれば早期リターンしてマイグレーション生成を中止
  - 警告のみの場合はValidationResultとともにSQLを返却
  - _Requirements: 3.2, 4.1, 4.2, 4.3_

- [ ] 8. CLI dry-runモードの実装
- [ ] 8.1 generateコマンドにdry-runフラグを追加する
  - --dry-runオプションをclapで定義
  - dry-run時は生成されるSQLをコンソールに表示
  - ファイル作成を抑止する分岐を実装
  - _Requirements: 5.1, 5.2_

- [ ] 8.2 型変更プレビューと警告/エラー表示を実装する
  - 型変更を old_type → new_type 形式で表示
  - 警告は黄色、エラーは赤色で色付き出力
  - 位置情報（テーブル名、カラム名）をシアンで表示
  - 修正提案を緑色で表示
  - サマリー（警告数、エラー数）を太字で出力
  - エラー時は終了コード1で中止、警告のみは終了コード0で続行
  - _Requirements: 5.3, 4.1, 4.2, 4.3_

- [ ] 9. 統合テストの実装
- [ ] 9.1 (P) PostgreSQL型変更の統合テストを実装する
  - 型変更マイグレーション生成と実行のE2Eテスト
  - up適用→down適用のロールバック検証
  - USING句が必要なケースと不要なケースの両方をテスト
  - _Requirements: 2.3, 3.1_

- [ ] 9.2 (P) MySQL型変更の統合テストを実装する
  - MODIFY COLUMN文の生成と実行のE2Eテスト
  - NULL制約とDEFAULT値の保持を検証
  - up適用→down適用のロールバック検証
  - _Requirements: 2.4, 3.1_

- [ ] 9.3 (P) SQLiteテーブル再作成の統合テストを実装する
  - テーブル再作成パターンの生成と実行のE2Eテスト
  - データコピーの正確性を検証
  - インデックスと制約の再作成を検証
  - _Requirements: 2.5_

- [ ] 9.4 型変更検証のテストを実装する
  - 警告対象（データ損失リスク）の型変更テスト
  - エラー対象（互換性なし）の型変更テスト
  - 精度損失警告のテスト
  - _Requirements: 4.1, 4.2, 4.3_

## Requirements Coverage

| Requirement | Tasks |
|-------------|-------|
| 1.1 | （既存実装で対応済み - SchemaDiffDetector） |
| 1.2 | （既存実装で対応済み - ColumnDiff.old_column/new_column） |
| 1.3 | （既存実装で対応済み - TableDiff.modified_columns） |
| 1.4 | （既存実装で対応済み - ColumnChange::TypeChanged） |
| 2.1 | 1.2, 3.1, 4.1, 5.1, 6.3, 7.1 |
| 2.2 | 1.2, 3.1, 4.1, 5.1, 6.3, 7.1 |
| 2.3 | 4.1, 9.1 |
| 2.4 | 5.1, 9.2 |
| 2.5 | 6.1, 6.2, 6.3, 9.3 |
| 3.1 | 1.1, 4.1, 5.1, 9.1, 9.2 |
| 3.2 | 2.1, 7.2 |
| 3.3 | 4.1, 5.1 |
| 4.1 | 1.1, 2.1, 7.2, 8.2, 9.4 |
| 4.2 | 1.1, 2.2, 7.2, 8.2, 9.4 |
| 4.3 | 1.1, 2.1, 7.2, 8.2, 9.4 |
| 5.1 | 8.1 |
| 5.2 | 8.1 |
| 5.3 | 8.2 |
