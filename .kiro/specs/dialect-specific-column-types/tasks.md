# Implementation Tasks: dialect-specific-column-types

---
**作成日**: 2026-01-22T10:18:58Z
**仕様ID**: dialect-specific-column-types
**フェーズ**: タスク生成完了

---

## Task Breakdown

### Task 1: DialectSpecificバリアントの追加 ✅
**要件マッピング**: 1, 2, 4

**サブタスク**:
- [x] `src/core/schema.rs`の`ColumnType` enumに`DialectSpecific`バリアントを追加する
   - `#[serde(untagged)]`属性を使用して既存のtaggedパターンと共存させる
   - `kind: String`と`params: serde_json::Value`フィールドを持つ構造にする
   - 既存の共通型バリアント（INTEGER, VARCHAR等）はそのまま維持する

- [x] `DialectSpecific`バリアントのシリアライゼーション・デシリアライゼーションが正しく動作することを単体テストで確認する
   - 既存の共通型との混在パターンをテストする
   - パラメータなしの型（例: `SERIAL`）のデシリアライズをテストする
   - パラメータありの型（例: `ENUM`with`values`）のデシリアライズをテストする

---

### Task 2: SQL生成ロジックの拡張 ✅
**要件マッピング**: 3

**サブタスク**:
- [x] `SqlGenerator` traitに`format_dialect_specific_type`メソッドを追加する (P)
   - `kind: &str`と`params: &serde_json::Value`を引数に取る
   - デフォルト実装で`kind`をそのまま出力し、`params`が存在する場合は適切にフォーマットする

- [x] PostgreSQL用の`format_dialect_specific_type`実装を追加する (P)
   - `src/adapters/sql_generator/postgres.rs`に実装する
   - `SERIAL`, `BIGSERIAL`, `ARRAY`等の頻出型のフォーマットを処理する
   - パラメータがある場合（例: `VARBIT(n)`）の括弧付き出力を処理する

- [x] MySQL用の`format_dialect_specific_type`実装を追加する (P)
   - `src/adapters/sql_generator/mysql.rs`に実装する
   - `ENUM(values)`, `SET(values)`, `TINYINT`等の頻出型のフォーマットを処理する
   - 配列パラメータ（例: `ENUM(['a', 'b'])`）を適切に引用符付きで出力する

- [x] SQLite用の`format_dialect_specific_type`実装を追加する (P)
   - `src/adapters/sql_generator/sqlite.rs`に実装する
   - SQLiteの制約された型システムに対応する（ほとんどの方言固有型は共通型で十分）
   - 必要に応じて警告メッセージを出力する

- [x] `generate_create_table`メソッド内で`DialectSpecific`バリアントを処理するロジックを追加する
   - 既存の`match`式に`DialectSpecific`パターンを追加する
   - `format_dialect_specific_type`メソッドを呼び出す

---

### Task 3: YAML Schema（IDE補完用）の作成
**要件マッピング**: 1, 2, 6

**サブタスク**:
1. `resources/schemas/stratum-schema.json`ファイルを作成する
   - JSON Schema Draft 2020-12形式で定義する
   - `ColumnType`のすべてのバリアント（共通型 + DialectSpecific）を`oneOf`で記述する
   - 各方言固有型に`description`フィールドで説明を追加する

2. PostgreSQL方言固有型のスキーマ定義を追加する
   - `SERIAL`, `BIGSERIAL`, `SMALLSERIAL`（パラメータなし）
   - `INT2`, `INT4`, `INT8`（パラメータなし）
   - `VARBIT`（lengthパラメータあり）
   - `INET`, `CIDR`（パラメータなし）
   - `ARRAY`（要素型パラメータあり）

3. MySQL方言固有型のスキーマ定義を追加する
   - `TINYINT`, `MEDIUMINT`（パラメータなし）
   - `ENUM`（valuesパラメータ必須）
   - `SET`（valuesパラメータ必須）
   - `YEAR`（パラメータなし）

4. READMEにVSCode YAML拡張の設定方法を追記する
   - `.vscode/settings.json`の設定例を追加する
   - `yaml.schemas`マッピングの記述方法を説明する

---

### Task 4: データベース検証とエラー伝達
**要件マッピング**: 2, 5

**サブタスク**:
1. `SchemaValidator`サービスで`DialectSpecific`バリアントを検証スキップ対象とする
   - `src/services/schema_validator.rs`の検証ロジックを確認する
   - `DialectSpecific`バリアントに対しては何も検証しない（データベースに委譲）
   - 共通型の既存検証ロジックは維持する

2. データベース実行時のエラーメッセージを透過的に伝達するテストを追加する
   - PostgreSQLで無効な型名（例: `INVALID_TYPE`）を使用した場合のエラー伝達をテストする
   - MySQLで無効なENUMパラメータを使用した場合のエラー伝達をテストする
   - エラーメッセージに`HINT`等のデータベース固有情報が含まれることを確認する

---

### Task 5: サンプルスキーマとドキュメント
**要件マッピング**: 6

**サブタスク**:
1. `examples/`ディレクトリに方言固有型を使用したサンプルスキーマを追加する (P)
   - PostgreSQL用サンプル（`examples/postgres_specific_types.yml`）
   - MySQL用サンプル（`examples/mysql_specific_types.yml`）
   - SQLite用サンプル（`examples/sqlite_specific_types.yml`）

2. READMEに方言固有型の使用方法セクションを追加する
   - YAMLでの記述例を示す
   - 共通型と方言固有型の使い分けガイドラインを記述する
   - エラー検出のタイミング（データベース実行時）を明記する

3. サポートされる方言固有型のリファレンスドキュメントを作成する
   - PostgreSQL, MySQL, SQLiteそれぞれの頻出型をリストアップする
   - 各型のパラメータ構造を説明する
   - YAML Schema（JSON Schema）ファイルへのリンクを追加する

---

### Task 6: 統合テスト
**要件マッピング**: 1, 2, 3, 4, 5

**サブタスク**:
1. PostgreSQL方言固有型の統合テストを追加する
   - `SERIAL`型のテーブル作成とマイグレーション実行をテストする
   - `ARRAY`型のパラメータ処理をテストする
   - 既存の共通型との混在スキーマをテストする

2. MySQL方言固有型の統合テストを追加する
   - `ENUM`型のvaluesパラメータ処理をテストする
   - `TINYINT`型のテーブル作成をテストする
   - 既存の共通型との混在スキーマをテストする

3. 無効な方言固有型のエラーハンドリングをテストする
   - 存在しない型名のエラーメッセージ伝達をテストする
   - 不正なパラメータのエラーメッセージ伝達をテストする
   - エラーメッセージにデータベースの詳細情報が含まれることを確認する

---

## Parallelization Notes

- **Task 2のサブタスク1-4**: 各方言のSQL生成実装は独立しているため並列実装可能
- **Task 5のサブタスク1**: 各サンプルスキーマファイルは独立しているため並列作成可能

## Implementation Order Recommendation

1. **Phase 1**: Task 1（ColumnType拡張）→ コア機能の基盤
2. **Phase 2**: Task 2（SQL生成）→ 主要機能の実装
3. **Phase 3**: Task 4（検証スキップ）→ データベース委譲の確立
4. **Phase 4**: Task 6（統合テスト）→ 動作確認
5. **Phase 5**: Task 3, Task 5（ドキュメント・YAML Schema）→ ユーザー体験の向上

## Coverage Analysis

- **Requirement 1** (方言固有型の定義): Task 1, Task 3, Task 5, Task 6
- **Requirement 2** (YAML Schema検証): Task 3, Task 4, Task 6
- **Requirement 3** (SQL生成): Task 2, Task 6
- **Requirement 4** (型変換なし): Task 2, Task 6
- **Requirement 5** (エラーメッセージ): Task 4, Task 6
- **Requirement 6** (ドキュメント): Task 3, Task 5

すべての要件がタスクでカバーされています。
