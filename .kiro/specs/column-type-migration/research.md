# Research & Design Decisions: column-type-migration

---
**Purpose**: カラム型変更機能の設計に向けた調査結果と設計判断の記録

**Discovery Scope**: Extension（既存システムの拡張）
---

## Summary
- **Feature**: `column-type-migration`
- **Discovery Scope**: Extension - 既存の差分検出・マイグレーション生成パイプラインの拡張
- **Key Findings**:
  - 型変更検出は既存の`SchemaDiffDetector`と`ColumnDiff`で実装済み
  - SQL生成は方言ごとに大きく異なり、SQLiteはテーブル再作成が必要
  - 既存の`ValidationWarning`インフラは拡張可能

## Research Log

### PostgreSQL ALTER COLUMN TYPE 構文
- **Context**: PostgreSQLでのカラム型変更SQL生成方法の調査
- **Sources Consulted**:
  - [PostgreSQL Documentation: ALTER TABLE](https://www.postgresql.org/docs/current/sql-altertable.html)
  - [W3Schools PostgreSQL ALTER COLUMN](https://www.w3schools.com/postgresql/postgresql_alter_column.php)
  - [Bytebase: How to ALTER COLUMN TYPE in Postgres](https://www.bytebase.com/reference/postgres/how-to/how-to-alter-column-type-postgres/)
- **Findings**:
  - 基本構文: `ALTER TABLE table_name ALTER COLUMN column_name TYPE new_data_type`
  - 互換性がない型変換には`USING`句が必要: `USING column_name::new_type`
  - 複数カラムを一度に変更可能（カンマ区切り）
  - デフォルト値がある場合、`USING`適用後に再設定が必要な場合がある
  - 大規模テーブルではテーブルロックが発生
- **Implications**:
  - `generate_alter_column_type`メソッドで`USING`句のオプショナル対応が必要
  - PostgresSqlGeneratorに`ALTER TABLE ... ALTER COLUMN ... TYPE ...`生成を追加

### MySQL MODIFY COLUMN 構文
- **Context**: MySQLでのカラム型変更SQL生成方法の調査
- **Sources Consulted**:
  - [MySQL 9.0 Reference: ALTER TABLE](https://dev.mysql.com/doc/refman/9.0/en/alter-table.html)
  - [DataCamp: MySQL ALTER TABLE](https://www.datacamp.com/doc/mysql/mysql-alter-table)
  - [Beekeeper Studio: MySQL Change Column Type](https://www.beekeeperstudio.io/blog/mysql-change-column-type)
- **Findings**:
  - 基本構文: `ALTER TABLE table_name MODIFY COLUMN column_name new_data_type`
  - `MODIFY COLUMN`は定義全体（NULL制約、DEFAULT等）を再指定する必要がある
  - `CHANGE COLUMN`はリネーム時に使用、型変更のみなら`MODIFY`が簡潔
  - 外部キー制約がある場合は関連テーブルにも影響
- **Implications**:
  - MysqlSqlGeneratorでは完全なカラム定義を含む`MODIFY COLUMN`文を生成
  - NULL制約とDEFAULT値を保持する必要がある

### SQLite テーブル再作成パターン
- **Context**: SQLiteでのカラム型変更の実現方法調査
- **Sources Consulted**:
  - [SQLite ALTER TABLE](https://www.sqlite.org/lang_altertable.html)
  - [SQLite Tutorial: ALTER TABLE Limitations](https://www.sqlitetutorial.net/sqlite-alter-table/)
  - [Simon Willison: Advanced ALTER TABLE](https://simonwillison.net/2020/Sep/23/sqlite-advanced-alter-table/)
- **Findings**:
  - SQLiteは`ALTER COLUMN TYPE`を**サポートしない**
  - 公式推奨パターン:
    1. `PRAGMA foreign_keys=off;`
    2. `BEGIN TRANSACTION;`
    3. 新テーブルを新しいスキーマで作成
    4. データをコピー: `INSERT INTO new_table SELECT * FROM old_table;`
    5. 旧テーブルを削除: `DROP TABLE old_table;`
    6. リネーム: `ALTER TABLE new_table RENAME TO old_table;`
    7. インデックス・トリガーを再作成
    8. `COMMIT;`
    9. `PRAGMA foreign_keys=on;` + `PRAGMA foreign_key_check;`
  - 大規模テーブルでは2倍のストレージが一時的に必要
- **Implications**:
  - SqliteSqlGeneratorでは複数文のトランザクションスクリプトを生成
  - インデックスと制約の再作成ロジックが必要
  - 複雑なため専用サービスへの分離を検討

### 型変更の互換性ルール
- **Context**: データ損失リスクの分類と警告ルールの定義
- **Sources Consulted**:
  - [QuestDB: ALTER TABLE COLUMN TYPE](https://questdb.com/docs/query/sql/alter-table-change-column-type/)
  - [Chat2DB: Change Column Data Types](https://chat2db.ai/resources/blog/change-column-data-types-in-mysql)
  - [PlanetScale: Backward Compatible Database Changes](https://planetscale.com/blog/backward-compatible-databases-changes)
- **Findings**:
  - **危険な変換**（データ損失の可能性）:
    - VARCHAR → INTEGER（非数値文字列でエラー）
    - TEXT → BOOLEAN（true/false以外でエラー）
    - サイズ縮小（VARCHAR(255) → VARCHAR(100)）
    - 精度低下（DECIMAL(10,2) → DECIMAL(5,2)）
  - **安全な変換**:
    - INTEGER → BIGINT（範囲拡大）
    - VARCHAR → TEXT（制限解除）
    - BOOLEAN → INTEGER（0/1へ自動変換）
  - **禁止すべき変換**:
    - 互換性のない型間（例: JSONB → INTEGER）
- **Implications**:
  - `TypeChangeValidator`サービスで互換性ルールを定義
  - `WarningKind::DataLoss`を`WarningKind`に追加
  - `ValidationError::TypeConversion`を新設

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A: 既存拡張 | MigrationGenerator + SqlGeneratorを拡張 | 影響範囲限定、既存パターン維持 | SQLite再作成でGenerator肥大化 | シンプルなケースに適合 |
| B: 新規サービス | ColumnTypeMigrationServiceを新設 | 責務分離、テスタビリティ向上 | 新インターフェース増加 | 過度な抽象化リスク |
| **C: ハイブリッド** | 基本はA、複雑部分を分離 | 既存活用 + 複雑部の隔離 | 境界設計の初期コスト | **推奨** |

## Design Decisions

### Decision: ハイブリッドアプローチの採用
- **Context**: 型変更SQL生成の責務配置
- **Alternatives Considered**:
  1. Option A - 既存`MigrationGenerator`への全機能統合
  2. Option B - 完全な新サービス`ColumnTypeMigrationService`
- **Selected Approach**: Option C - ハイブリッド
  - `SqlGenerator`トレイトに`generate_alter_column_type`メソッド追加
  - PostgreSQL/MySQLは各Generatorで実装
  - SQLiteテーブル再作成は`SqliteTableRecreator`として分離
  - 型変更検証は`TypeChangeValidator`として分離
- **Rationale**:
  - 既存の`MigrationGenerator`パイプラインを維持
  - SQLiteの複雑さを隔離してテスト容易性を確保
  - 型変更検証ルールを独立させて拡張性を確保
- **Trade-offs**:
  - (+) 既存コードへの影響最小化
  - (+) 複雑なロジックの分離
  - (-) 新ファイル追加（2-3ファイル）
- **Follow-up**: SQLiteテーブル再作成のトランザクション境界を実装時に検証

### Decision: SqlGeneratorトレイトへのALTER COLUMN API追加
- **Context**: 方言別SQL生成の統一インターフェース
- **Alternatives Considered**:
  1. 新トレイト`AlterColumnGenerator`を定義
  2. 既存`SqlGenerator`トレイトを拡張
- **Selected Approach**: 既存`SqlGenerator`トレイトに`generate_alter_column_type`を追加
- **Rationale**:
  - CREATE/ALTER操作は論理的に同一コンポーネントの責務
  - 方言別の統一的なディスパッチが可能
- **Trade-offs**: トレイトの肥大化（+1メソッド）

### Decision: 型変更互換性ルールの定義
- **Context**: 危険な型変更の検出と警告
- **Alternatives Considered**:
  1. ハードコードされた互換性マトリクス
  2. 設定ファイルベースのルール定義
  3. 型カテゴリベースのルール
- **Selected Approach**: 型カテゴリベース + ハードコードルール
  - 型を「数値」「文字列」「日時」「バイナリ」「JSON」「その他」にカテゴリ分け
  - カテゴリ間変換は警告、サイズ縮小は警告、互換性なしはエラー
- **Rationale**:
  - シンプルで理解しやすい
  - 将来的な設定ファイル化への拡張が可能
- **Trade-offs**:
  - (+) 実装がシンプル
  - (-) 細かい例外ケースの対応が必要な場合がある

### Decision: 旧/新Schemaの注入によるテーブル定義取得
- **Context**: SQLite再作成およびMySQL MODIFY COLUMNで完全なテーブル定義が必要
- **Alternatives Considered**:
  1. SchemaDiff拡張 - `TableDiff`に`old_table`/`new_table`を追加
  2. 旧/新Schemaの注入 - `MigrationGenerator`のAPIシグネチャ変更
  3. ColumnDiff拡張 - `old_column`/`new_column`のみ活用
- **Selected Approach**: Option B - 旧/新Schemaの注入
  - `MigrationGenerator::generate_up_sql(diff, old_schema, new_schema, dialect)`
  - `MigrationGenerator::generate_down_sql(diff, old_schema, new_schema, dialect)`
- **Rationale**:
  - 既存の`SchemaDiff`モデルを変更せずに済む
  - SQLite再作成時に完全なテーブル定義（全カラム、インデックス、制約）が取得可能
  - up/downで一貫した参照が可能（upは`new_schema`、downは`old_schema`を参照）
- **Trade-offs**:
  - (+) 既存モデルへの影響なし
  - (+) SQLiteの複雑な再作成に必要な全情報を取得可能
  - (-) APIシグネチャの変更が必要（呼び出し側の修正）

### Decision: 警告/エラーの出力フォーマット
- **Context**: 型変更検証結果のCLI表示方法
- **Alternatives Considered**:
  1. シンプルなテキスト出力
  2. 構造化された色付き出力
  3. JSON形式出力（CI/CD向け）
- **Selected Approach**: 構造化された色付き出力
  - 警告: Yellow (`⚠ Warning:`)
  - エラー: Red (`✗ Error:`)
  - 位置情報: Cyan (`(table: ..., column: ...)`)
  - 修正提案: Green (`Suggestion:`)
- **Rationale**:
  - 既存の`colored`クレートを活用
  - ユーザーが問題箇所を素早く特定可能
  - 出力順序（警告→エラー→サマリー）で重要度を明確化
- **Trade-offs**:
  - (+) ユーザビリティ向上
  - (-) 実装の複雑さ微増
- **Follow-up**: 将来的に`--format=json`オプションでJSON出力を検討

### Decision: SQLite「型変更＋他変更」の処理方針
- **Context**: SQLiteで型変更と他の変更（NULL変更、DEFAULT変更等）が同時に発生した場合の処理
- **Alternatives Considered**:
  1. 再作成に一本化 - 型変更があれば他変更も含めてテーブル再作成
  2. 型変更のみ再作成 - 型変更はテーブル再作成、他変更は別処理
  3. 変更種別で判断 - SQLiteでサポートされない変更のみ再作成
- **Selected Approach**: Option A - 再作成に一本化
  - 型変更を含む`TableDiff`のすべての変更を一度のテーブル再作成で処理
  - 型変更がない`TableDiff`（カラム追加のみ等）は既存の`ALTER TABLE ADD COLUMN`を使用
- **Rationale**:
  - SQLiteの`ALTER TABLE`は制限が多い（型変更、NULL変更、DEFAULT変更などほぼ再作成が必要）
  - 同一テーブルに複数変更がある場合、一度の再作成で済む方が効率的
  - 実装の複雑さを抑えられる
- **Trade-offs**:
  - (+) 実装シンプル、一度の再作成で完結
  - (+) 同一テーブルへの複数操作を最適化
  - (-) 型変更なしでも再作成が必要な変更があると別途対応が必要（将来課題）

### Decision: PostgreSQL USING句の自動生成ルール
- **Context**: PostgreSQLの`ALTER COLUMN TYPE`で`USING`句が必要なケースの判定
- **Alternatives Considered**:
  1. TypeCategoryベース自動生成 - カテゴリ間変換で自動的に`USING col::new_type`を付与
  2. ユーザー指定の変換式 - YAMLで`using_expression`を指定可能に
  3. ハイブリッド - デフォルトはTypeCategoryベース、必要時にYAMLでオーバーライド
- **Selected Approach**: Option A - TypeCategoryベース自動生成（初期実装）
  - String→Numeric: `USING col::INTEGER`
  - String→Boolean: `USING col::BOOLEAN`
  - String→Json: `USING col::JSONB`
  - Numeric→String, DateTime→String, Boolean→Numeric: 暗黙変換（USINGなし）
  - Same Category: 暗黙変換（USINGなし）
- **Rationale**:
  - 80%以上のケースは単純なキャスト（`::new_type`）で対応可能
  - 複雑な変換が必要なケースは稀で、その場合はraw SQLを書くユーザーが多い
  - 初期リリースはシンプルに保ち、ユーザーフィードバックを得てから拡張
- **Trade-offs**:
  - (+) 実装シンプル、多くのケースをカバー
  - (+) ユーザーが意識せずに正しいSQLが生成される
  - (-) 複雑な変換（JSON→特定フォーマット等）には対応不可
- **Follow-up**: 将来的にOption C（YAMLオーバーライド）への拡張を検討

### Decision: SQLiteデータコピー方式
- **Context**: SQLiteテーブル再作成時のデータコピー方法
- **Alternatives Considered**:
  1. `SELECT *` - 全カラム一括コピー
  2. 列交差ベースの明示的カラムリスト - old/new の共通カラムのみ指定
- **Selected Approach**: Option B - 列交差ベースの明示的カラムリスト
  - `INSERT INTO new_table (col1, col2) SELECT col1, col2 FROM old_table`
- **Rationale**:
  - カラム追加・削除と型変更が同時に発生した場合に対応可能
  - 追加列はDEFAULT値またはNULLで自動補完
  - 削除列は単純に除外
- **Trade-offs**:
  - (+) カラム変更に柔軟に対応
  - (+) 明示的で予測可能な動作
  - (-) `SELECT *` より若干複雑
- **追加列の扱い**:
  - DEFAULTあり → DEFAULT値が自動適用
  - DEFAULTなし（NULLable） → NULL挿入
  - NOT NULL かつ DEFAULTなし → 型変更と同時には不可（事前にエラー検出）

## Risks & Mitigations
- **SQLiteテーブル再作成の複雑さ** → 専用サービス`SqliteTableRecreator`で隔離、十分なテストケース
- **データ損失リスクの見落とし** → 保守的なルール設定（疑わしい場合は警告）
- **大規模テーブルでのパフォーマンス** → dry-runモードでの事前確認を推奨、ドキュメント化

## References
- [PostgreSQL ALTER TABLE Documentation](https://www.postgresql.org/docs/current/sql-altertable.html)
- [MySQL ALTER TABLE Statement](https://dev.mysql.com/doc/refman/9.0/en/alter-table.html)
- [SQLite ALTER TABLE](https://www.sqlite.org/lang_altertable.html)
- [PlanetScale: Backward Compatible Database Changes](https://planetscale.com/blog/backward-compatible-databases-changes)
