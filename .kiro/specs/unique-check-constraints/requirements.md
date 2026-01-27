# 要件ドキュメント

## はじめに

本仕様は、Strata CLIツールにおける **UNIQUE制約** および **CHECK制約** の既存テーブルへの追加・削除に対するマイグレーションサポートを定義する。現在、スキーマモデル・YAML定義・差分検出・CREATE TABLE時の生成は実装済みだが、既存テーブルに対する制約変更のマイグレーション生成が未対応である。本仕様はこのギャップを埋め、全対応方言（PostgreSQL, MySQL, SQLite）で一貫した制約マイグレーションを実現することを目的とする。

## 要件

### 要件 1: UNIQUE制約の追加マイグレーション生成

**目的:** 開発者として、既存テーブルにUNIQUE制約を追加するマイグレーションを自動生成したい。これにより、スキーマYAMLの変更だけで制約変更を安全に適用できるようになる。

#### 受け入れ基準

1. When スキーマYAMLの既存テーブルに新しいUNIQUE制約が追加された場合, the Strata shall 対応する`ALTER TABLE ... ADD CONSTRAINT ... UNIQUE (...)` SQLをup.sqlに生成する（PostgreSQL・MySQL方言）
2. When UNIQUE制約の追加マイグレーションが生成される場合, the Strata shall 制約名を`uq_{テーブル名}_{カラム名}`の命名規則で自動生成する
3. When UNIQUE制約が複数カラムにまたがる場合, the Strata shall 複合UNIQUE制約として単一のALTER TABLE文を生成する
4. When SQLite方言でUNIQUE制約が追加される場合, the Strata shall テーブル再作成パターンを用いてマイグレーションを生成する

### 要件 2: UNIQUE制約の削除マイグレーション生成

**目的:** 開発者として、既存テーブルからUNIQUE制約を削除するマイグレーションを自動生成したい。これにより、不要になった制約を安全に除去できるようになる。

#### 受け入れ基準

1. When スキーマYAMLの既存テーブルからUNIQUE制約が削除された場合, the Strata shall 対応する`ALTER TABLE ... DROP CONSTRAINT ...` SQLをup.sqlに生成する（PostgreSQL方言）
2. When MySQL方言でUNIQUE制約が削除される場合, the Strata shall `ALTER TABLE ... DROP INDEX ...` SQLを生成する
3. When SQLite方言でUNIQUE制約が削除される場合, the Strata shall テーブル再作成パターンを用いてマイグレーションを生成する
4. When UNIQUE制約の追加マイグレーションが生成される場合, the Strata shall 対応するdown.sqlにロールバック用の削除SQLを生成する
5. When UNIQUE制約の削除マイグレーションが生成される場合, the Strata shall 対応するdown.sqlにロールバック用の追加SQLを生成する

### 要件 3: CHECK制約の追加マイグレーション生成

**目的:** 開発者として、既存テーブルにCHECK制約を追加するマイグレーションを自動生成したい。これにより、データ整合性ルールをスキーマレベルで適用できるようになる。

#### 受け入れ基準

1. When スキーマYAMLの既存テーブルに新しいCHECK制約が追加された場合, the Strata shall 対応する`ALTER TABLE ... ADD CONSTRAINT ... CHECK (...)` SQLをup.sqlに生成する（PostgreSQL・MySQL方言）
2. When CHECK制約の追加マイグレーションが生成される場合, the Strata shall 制約名を`ck_{テーブル名}_{カラム名}`の命名規則で自動生成する
3. When SQLite方言でCHECK制約が追加される場合, the Strata shall テーブル再作成パターンを用いてマイグレーションを生成する

### 要件 4: CHECK制約の削除マイグレーション生成

**目的:** 開発者として、既存テーブルからCHECK制約を削除するマイグレーションを自動生成したい。これにより、不要になったデータ整合性ルールを安全に除去できるようになる。

#### 受け入れ基準

1. When スキーマYAMLの既存テーブルからCHECK制約が削除された場合, the Strata shall 対応する`ALTER TABLE ... DROP CONSTRAINT ...` SQLをup.sqlに生成する（PostgreSQL方言）
2. When MySQL方言でCHECK制約が削除される場合, the Strata shall `ALTER TABLE ... DROP CHECK ...` SQLを生成する
3. When SQLite方言でCHECK制約が削除される場合, the Strata shall テーブル再作成パターンを用いてマイグレーションを生成する
4. When CHECK制約の追加マイグレーションが生成される場合, the Strata shall 対応するdown.sqlにロールバック用の削除SQLを生成する
5. When CHECK制約の削除マイグレーションが生成される場合, the Strata shall 対応するdown.sqlにロールバック用の追加SQLを生成する

### 要件 5: マイグレーションパイプラインへの統合

**目的:** 開発者として、UNIQUE・CHECK制約の変更が既存のマイグレーション生成パイプラインに統合されていることを期待する。これにより、他のスキーマ変更と同時に一貫した方法で処理される。

#### 受け入れ基準

1. When マイグレーションが生成される場合, the Strata shall UNIQUE・CHECK制約の追加・削除を他のスキーマ変更（カラム追加・インデックス変更等）と同一のマイグレーションファイルに含める
2. The Strata shall UNIQUE・CHECK制約の変更を適切なステージ順序（テーブル変更後、クリーンアップ前）で実行する
3. While dry-runモードが有効な場合, the Strata shall UNIQUE・CHECK制約のマイグレーションSQLをプレビュー表示し、実際のDB適用は行わない

### 要件 6: スキーマバリデーションの拡充

**目的:** 開発者として、UNIQUE・CHECK制約の定義ミスを早期に検出したい。これにより、不正なマイグレーションの生成を防止できる。

#### 受け入れ基準

1. When UNIQUE制約が存在しないカラムを参照している場合, the Strata shall バリデーションエラーを報告する
2. When CHECK制約が存在しないカラムを参照している場合, the Strata shall バリデーションエラーを報告する
3. When CHECK制約のcheck_expressionが空文字列の場合, the Strata shall バリデーションエラーを報告する
4. If 同一テーブルに同じカラム構成の重複するUNIQUE制約が定義された場合, the Strata shall 警告を報告する
