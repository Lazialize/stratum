# Implementation Tasks

## Task 1: Coreドメインモデルの拡張

### Task 1.1: Column構造体にrenamed_fromフィールドを追加 (P)
**Requirements**: 1.1, 1.2, 1.3

- [x] `src/core/schema.rs`の`Column`構造体に`renamed_from: Option<String>`フィールドを追加
- [x] `#[serde(default, skip_serializing_if = "Option::is_none")]`アトリビュートを設定
- [x] `Column::new()`メソッドを更新（デフォルトで`renamed_from: None`）
- [x] 単体テスト：`renamed_from`フィールドのシリアライズ/デシリアライズ
- [x] 単体テスト：`renamed_from`が`None`の場合YAML出力から除外されることを確認

### Task 1.2: ColumnChange列挙型にRenamedバリアントを追加 (P)
**Requirements**: 2.2, 2.3

- [x] `src/core/schema_diff.rs`の`ColumnChange`列挙型に`Renamed { old_name: String, new_name: String }`バリアントを追加
- [x] 単体テスト：`Renamed`バリアントの生成と比較

### Task 1.3: RenamedColumn構造体を追加 (P)
**Requirements**: 2.1, 2.2

- [x] `src/core/schema_diff.rs`に`RenamedColumn`構造体を追加
  ```rust
  pub struct RenamedColumn {
      pub old_name: String,
      pub old_column: Column,  // MySQL Down方向で必要
      pub new_column: Column,
      pub changes: Vec<ColumnChange>,
  }
  ```
- [x] `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`を設定
- [x] 単体テスト：`RenamedColumn`の生成と比較

### Task 1.4: TableDiff構造体にrenamed_columnsフィールドを追加
**Requirements**: 2.1, 2.2, 2.4

- [x] `src/core/schema_diff.rs`の`TableDiff`構造体に`renamed_columns: Vec<RenamedColumn>`フィールドを追加
- [x] `#[serde(default, skip_serializing_if = "Vec::is_empty")]`アトリビュートを設定
- [x] `TableDiff::new()`または初期化コードを更新
- [x] 既存テストの修正（`TableDiff`初期化箇所）

## Task 2: スキーマ差分検出の拡張

### Task 2.1: SchemaDiffDetectorにリネーム検出ロジックを追加
**Requirements**: 2.1, 2.2, 2.3, 2.4

- [x] `src/services/schema_diff_detector.rs`の`detect_column_diff`メソッドを拡張
- [x] 新カラムに`renamed_from`がある場合のリネーム検出ロジックを実装
  - 旧テーブルに`renamed_from`で指定されたカラムが存在するか確認
  - 存在する場合：`renamed_columns`に追加、`removed_columns`から除外
  - 存在しない場合：警告を収集、通常のadded/modifiedとして処理
- [x] リネームと同時の型/NULL/default変更も`changes`ベクターに追加
- [x] 単体テスト：単純なリネーム検出
- [x] 単体テスト：リネーム+型変更の同時検出
- [x] 単体テスト：複数カラムのリネーム検出
- [x] 単体テスト：旧カラム不存在時の警告生成

### Task 2.2: detect_diff_with_warningsメソッドを追加
**Requirements**: 2.1, 4.1

- [x] `SchemaDiffDetector`に`detect_diff_with_warnings()`メソッドを追加
  ```rust
  pub fn detect_diff_with_warnings(
      &self,
      old_schema: &Schema,
      new_schema: &Schema,
  ) -> (SchemaDiff, Vec<ValidationWarning>);
  ```
- [x] 既存の`detect_diff`を内部で呼び出し、警告も返すよう拡張
- [x] 単体テスト：警告付き差分検出

## Task 3: スキーマ検証の拡張

### Task 3.1: ValidationWarning型の追加（存在しない場合）
**Requirements**: 4.1

- [x] `ValidationWarning`型が存在しない場合、`src/core/validation.rs`または適切な場所に追加
- [x] 警告の種類を区別できるよう設計（`OldColumnNotFound`, `ForeignKeyReference`など）

### Task 3.2: SchemaValidatorServiceにリネーム検証を追加
**Requirements**: 4.1, 4.2, 4.3, 4.4

- [x] `src/services/schema_validator.rs`に`validate_renames`メソッドを追加
- [x] 重複リネームのエラー検出（同じ`renamed_from`が複数カラムで指定）
- [x] 名前衝突のエラー検出（`renamed_from`がリネーム先以外の既存カラム名と一致）
- [x] FK参照カラムのリネーム警告を検出
- [x] 単体テスト：重複リネームエラー
- [x] 単体テスト：名前衝突エラー
- [x] 単体テスト：FK参照警告
- [x] 単体テスト：旧カラム不存在警告（detect_diff_with_warningsと整合確認）

## Task 4: SQL生成の拡張

### Task 4.1: SqlGeneratorトレイトにgenerate_rename_columnメソッドを追加
**Requirements**: 3.1, 3.2, 3.3, 3.4

- [x] `src/adapters/sql_generator/mod.rs`の`SqlGenerator`トレイトに以下を追加
  ```rust
  fn generate_rename_column(
      &self,
      table: &Table,
      renamed_column: &RenamedColumn,
      direction: MigrationDirection,
  ) -> Vec<String>;
  ```
- [x] デフォルト実装は空のベクターを返す

### Task 4.2: PostgresSqlGeneratorのリネームSQL生成を実装 (P)
**Requirements**: 3.1, 3.4

- [x] `src/adapters/sql_generator/postgres.rs`に`generate_rename_column`を実装
- [x] Up方向：`ALTER TABLE {table} RENAME COLUMN {old_name} TO {new_name};`
- [x] Down方向：`ALTER TABLE {table} RENAME COLUMN {new_name} TO {old_name};`
- [x] 単体テスト：Up方向のリネームSQL生成
- [x] 単体テスト：Down方向のリネームSQL生成

### Task 4.3: MysqlSqlGeneratorのリネームSQL生成を実装 (P)
**Requirements**: 3.2, 3.4

- [x] `src/adapters/sql_generator/mysql.rs`に`generate_rename_column`を実装
- [x] Up方向：`ALTER TABLE {table} CHANGE COLUMN {old_name} {new_name} {column_definition};`（完全なカラム定義が必要）
- [x] Down方向：`ALTER TABLE {table} CHANGE COLUMN {new_name} {old_name} {column_definition};`（old_columnの定義を使用）
- [x] 単体テスト：Up方向のリネームSQL生成
- [x] 単体テスト：Down方向のリネームSQL生成

### Task 4.4: SqliteSqlGeneratorのリネームSQL生成を実装 (P)
**Requirements**: 3.3, 3.4

- [x] `src/adapters/sql_generator/sqlite.rs`に`generate_rename_column`を実装
- [x] Up方向：`ALTER TABLE {table} RENAME COLUMN {old_name} TO {new_name};`
- [x] Down方向：`ALTER TABLE {table} RENAME COLUMN {new_name} TO {old_name};`
- [x] 注：SQLite 3.25.0+を前提
- [x] 単体テスト：Up方向のリネームSQL生成
- [x] 単体テスト：Down方向のリネームSQL生成

## Task 5: マイグレーションパイプラインの拡張

### Task 5.1: リネーム+型変更の実行順序制御を実装
**Requirements**: 3.5

- [x] マイグレーション生成ロジックで`renamed_columns`を処理
- [x] Up方向：リネーム → 型変更の順序でSQL生成
- [x] Down方向：型変更の逆 → リネームの逆の順序でSQL生成
- [x] `renamed_columns`と`modified_columns`の重複排除を確認
- [x] 統合テスト：リネーム+型変更のマイグレーション生成

### Task 5.2: マイグレーションファイル生成にリネームSQLを追加
**Requirements**: 3.1, 3.2, 3.3, 3.4

- [x] `up.sql`にリネームSQLを適切な順序で出力
- [x] `down.sql`に逆リネームSQLを適切な順序で出力
- [x] 統合テスト：生成されるマイグレーションファイルの内容確認

## Task 6: CLIとユーザーフィードバック

### Task 6.1: 警告統合とエラー表示の実装
**Requirements**: 5.1, 4.1, 4.5

- [x] CLI `generate`コマンドで`detect_diff_with_warnings`と`validate_renames`の結果をマージ
- [x] 警告は黄色、エラーは赤色で表示
- [x] エラーメッセージにテーブル名、旧カラム名、新カラム名を含める
- [x] `renamed_from`属性の削除推奨警告を表示
- [x] 統合テスト：警告/エラーメッセージの表示確認

### Task 6.2: dry-runモードでのリネームSQLプレビュー
**Requirements**: 5.2

- [x] dry-runモードでリネームSQLを含むマイグレーション内容を表示
- [x] 統合テスト：dry-run出力確認

### Task 6.3: データベースエラーハンドリング
**Requirements**: 5.3

- [x] リネーム操作失敗時のエラーメッセージを改善
- [x] 具体的なエラー原因（カラム不存在、権限不足等）を含める
- [x] 統合テスト：エラーハンドリング確認（モック使用可）

## Task 7: 統合テストとE2Eテスト

### Task 7.1: YAMLスキーマからの完全なリネームフローテスト (P)
**Requirements**: 全要件

- [x] テスト用YAMLスキーマを作成（リネーム含む）
- [x] パース → 差分検出 → 検証 → SQL生成の統合テスト
- [x] PostgreSQL/MySQL/SQLiteそれぞれでのSQL出力確認

### Task 7.2: testcontainersを使用したE2Eテスト
**Requirements**: 全要件

- [x] PostgreSQLコンテナでのリネームマイグレーション適用・ロールバックテスト
- [x] MySQLコンテナでのリネームマイグレーション適用・ロールバックテスト
- [x] SQLiteでのリネームマイグレーション適用・ロールバックテスト（コンテナ不要）
- [x] リネーム+型変更の同時処理E2Eテスト

## Task Summary

| Task | Description | Requirements | Parallel |
|------|-------------|--------------|----------|
| 1.1 | Column.renamed_fromフィールド追加 | 1.1, 1.2, 1.3 | (P) |
| 1.2 | ColumnChange::Renamed追加 | 2.2, 2.3 | (P) |
| 1.3 | RenamedColumn構造体追加 | 2.1, 2.2 | (P) |
| 1.4 | TableDiff.renamed_columns追加 | 2.1, 2.2, 2.4 | - |
| 2.1 | リネーム検出ロジック実装 | 2.1-2.4 | - |
| 2.2 | detect_diff_with_warnings実装 | 2.1, 4.1 | - |
| 3.1 | ValidationWarning型追加 | 4.1 | - |
| 3.2 | validate_renames実装 | 4.1-4.4 | - |
| 4.1 | SqlGenerator.generate_rename_column追加 | 3.1-3.4 | - |
| 4.2 | PostgreSQL リネームSQL生成 | 3.1, 3.4 | (P) |
| 4.3 | MySQL リネームSQL生成 | 3.2, 3.4 | (P) |
| 4.4 | SQLite リネームSQL生成 | 3.3, 3.4 | (P) |
| 5.1 | リネーム+型変更の順序制御 | 3.5 | - |
| 5.2 | マイグレーションファイル生成 | 3.1-3.4 | - |
| 6.1 | 警告統合とエラー表示 | 5.1, 4.1, 4.5 | - |
| 6.2 | dry-runプレビュー | 5.2 | - |
| 6.3 | DBエラーハンドリング | 5.3 | - |
| 7.1 | 統合テスト | 全要件 | (P) |
| 7.2 | E2Eテスト | 全要件 | - |

## Dependency Graph

```
Task 1.1 ─┬─> Task 1.4 ─> Task 2.1 ─> Task 2.2 ─┬─> Task 5.1 ─> Task 5.2
Task 1.2 ─┤                                      │
Task 1.3 ─┘                                      │
                                                 │
Task 3.1 ─> Task 3.2 ─────────────────────────────┤
                                                 │
Task 4.1 ─┬─> Task 4.2 ───────────────────────────┼─> Task 6.1 ─> Task 6.2 ─> Task 6.3
          ├─> Task 4.3 ───────────────────────────┤
          └─> Task 4.4 ───────────────────────────┘
                                                 │
                                                 └─> Task 7.1 ─> Task 7.2
```

## Notes

- **(P)** マークのタスクは他の(P)タスクと並列実行可能
- Task 1.1, 1.2, 1.3 は依存関係がないため並列実行可能
- Task 4.2, 4.3, 4.4 は各DB方言で独立しているため並列実行可能
- SQLite 3.25.0+の`RENAME COLUMN`サポートを前提としており、それ以前のバージョンはサポート対象外
- `old_column`フィールドは旧スキーマからそのままコピー（`renamed_from`が残存していても問題なし）
