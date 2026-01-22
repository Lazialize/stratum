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

### Task 3: YAML Schema（IDE補完用）の作成 ✅
**要件マッピング**: 1, 2, 6

**サブタスク**:
- [x] `resources/schemas/stratum-schema.json`ファイルを作成する
   - JSON Schema Draft 2020-12形式で定義する
   - `ColumnType`のすべてのバリアント（共通型 + DialectSpecific）を`oneOf`で記述する
   - 各方言固有型に`description`フィールドで説明を追加する

- [x] PostgreSQL方言固有型のスキーマ定義を追加する
   - `SERIAL`, `BIGSERIAL`, `SMALLSERIAL`（パラメータなし）
   - `INT2`, `INT4`, `INT8`（パラメータなし）
   - `VARBIT`（lengthパラメータあり）
   - `INET`, `CIDR`（パラメータなし）
   - `ARRAY`（要素型パラメータあり）

- [x] MySQL方言固有型のスキーマ定義を追加する
   - `TINYINT`, `MEDIUMINT`（パラメータなし）
   - `ENUM`（valuesパラメータ必須）
   - `SET`（valuesパラメータ必須）
   - `YEAR`（パラメータなし）

- [x] READMEにVSCode YAML拡張の設定方法を追記する
   - `.vscode/settings.json`の設定例を追加する
   - `yaml.schemas`マッピングの記述方法を説明する

---

### Task 4: データベース検証とエラー伝達 ✅
**要件マッピング**: 2, 5

**サブタスク**:
- [x] `SchemaValidator`サービスで`DialectSpecific`バリアントを検証スキップ対象とする
   - `src/services/schema_validator.rs`の検証ロジックを確認する
   - `DialectSpecific`バリアントに対しては何も検証しない（データベースに委譲）
   - 共通型の既存検証ロジックは維持する
   - 4つの新しいユニットテストを追加（`test_validate_dialect_specific_type_*`）

- [x] データベース実行時のエラーメッセージを透過的に伝達するテストを追加する
   - PostgreSQLで無効な型名（例: `SERIALS`）を使用した場合のエラー伝達をテストする
   - PostgreSQLで正しい方言固有型（`SERIAL`, `INET`）の動作を確認するテストを追加する
   - PostgreSQLで無効なパラメータを使用した場合のエラー伝達をテストする
   - 統合テストファイル `tests/dialect_specific_database_error_test.rs` を作成
   - Docker環境が必要なテストは `#[ignore]` 属性でマーク

---

### Task 5: サンプルスキーマとドキュメント ✅
**要件マッピング**: 6

**サブタスク**:
- [x] `example/`ディレクトリに方言固有型を使用したサンプルスキーマを追加する
   - PostgreSQL用サンプル（`example/postgres_specific_types.yml`）- SERIAL, INET, CIDR, VARBIT, INT2/4/8型の例
   - MySQL用サンプル（`example/mysql_specific_types.yml`）- ENUM, SET, TINYINT, MEDIUMINT, YEAR型の例
   - SQLite用サンプル（`example/sqlite_specific_types.yml`）- SQLiteベストプラクティスの例
   - 既存: `example/schema/dialect_specific_example.yaml` - PostgreSQLとMySQLの混在例

- [x] READMEに方言固有型の使用方法セクションを追加する
   - YAMLでの記述例を示す（PostgreSQL: SERIAL, INET, VARBIT / MySQL: ENUM, SET, TINYINT）
   - 共通型と方言固有型の使い分けガイドラインを記述する（"When to Use"セクション）
   - エラー検出のタイミング（データベース実行時）を明記する
   - サンプルファイルへのリンクを追加

- [x] サポートされる方言固有型のリファレンスドキュメントを作成する
   - `example/DIALECT_SPECIFIC_TYPES.md`にPostgreSQL, MySQL, SQLiteの型をリストアップ
   - 各型のパラメータ構造を説明（ENUM values, VARBIT length等）
   - YAML Schema（JSON Schema）ファイルへのリンクを追加
   - IDE設定へのリンクを追加

---

### Task 6: 統合テスト ✅
**要件マッピング**: 1, 2, 3, 4, 5

**サブタスク**:
- [x] PostgreSQL方言固有型の統合テストを追加する
   - `SERIAL`型のテーブル作成とマイグレーション実行をテストする
   - `INET`型を使用したテーブル作成とデータ挿入をテストする
   - `ARRAY`型のパラメータ処理をテストする
   - 既存の共通型との混在スキーマをテストする

- [x] MySQL方言固有型の統合テストを追加する
   - `ENUM`型のvaluesパラメータ処理をテストする
   - `TINYINT`型のテーブル作成をテストする
   - `SET`型を使用したテーブル作成とデータ挿入をテストする
   - 既存の共通型との混在スキーマをテストする

- [x] 無効な方言固有型のエラーハンドリングをテストする（Task 4で実装済み）
   - 存在しない型名のエラーメッセージ伝達をテストする (`tests/dialect_specific_database_error_test.rs`)
   - 不正なパラメータのエラーメッセージ伝達をテストする (`tests/dialect_specific_database_error_test.rs`)
   - エラーメッセージにデータベースの詳細情報が含まれることを確認する (`tests/dialect_specific_database_error_test.rs`)

**実装内容**:
- `tests/dialect_specific_integration_test.rs`ファイルを作成
- PostgreSQL統合テスト（4テスト）:
  - `test_postgres_serial_type_table_creation` - SERIAL型のテーブル作成とスキーマ検証
  - `test_postgres_inet_type_table_creation` - INET型の動作確認（データ挿入・取得）
  - `test_postgres_array_type_table_creation` - ARRAY型の動作確認（配列データ処理）
  - `test_postgres_mixed_common_and_dialect_specific_types` - SERIAL+VARCHAR+DECIMAL混在スキーマ
- MySQL統合テスト（4テスト）:
  - `test_mysql_enum_type_table_creation` - ENUM型の動作確認（値制約検証）
  - `test_mysql_tinyint_type_table_creation` - TINYINT UNSIGNED型の動作確認
  - `test_mysql_set_type_table_creation` - SET型の動作確認（複数値選択）
  - `test_mysql_mixed_common_and_dialect_specific_types` - INT+VARCHAR+ENUM+TINYINT+TEXT混在スキーマ
- すべてのテストに`#[ignore]`属性を付与（Docker環境が必要）
- エラーハンドリングテストはTask 4で実装済みの`tests/dialect_specific_database_error_test.rs`で対応

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
