# Requirements Document

## Introduction

本仕様は、Stratumにおけるカラム型定義の改善を目的とします。現状、複数のデータベース方言に対応するため共通の`ColumnType` enumを用いて抽象化し、`to_sql_type()`メソッドで各方言向けSQL型に変換していますが、これを各データベース方言固有の型を直接指定できる仕組みに変更します。方言ごとに異なる`kind`値を許容し、JSON Schemaによる型検証で方言固有の型の整合性を担保します。

## Requirements

### Requirement 1: 方言固有カラム型の定義

**Objective:** スキーマ開発者として、データベース方言ごとに固有のカラム型を直接指定できるようにし、各方言の機能を最大限活用したい

#### Acceptance Criteria

1. When YAMLスキーマファイルにカラム型を定義する際, the Stratum schema parser shall 方言固有の`kind`値(例: PostgreSQLの`SERIAL`, MySQLの`TINYINT`, SQLiteの`INTEGER`)を受け入れる
2. The Stratum schema model shall 各方言専用の`ColumnType`バリアント(例: PostgreSQL用の`SERIAL`, `INT2`, `INT4`, `INT8`など)をサポートする
3. When 同一スキーマで複数方言をサポートする場合, the Stratum schema parser shall 方言ごとに異なるカラム型定義を許可する仕組み(例: `type_postgresql`, `type_mysql`, `type_sqlite`)を提供する
4. The Stratum schema serialization shall 方言固有の型パラメータ(例: PostgreSQLの`VARBIT(n)`, MySQLの`ENUM(values)`)をYAML/JSON形式で正確にシリアライズ・デシリアライズする

### Requirement 2: JSON Schemaによる型検証

**Objective:** スキーマ開発者として、方言に存在しない型が指定された場合に即座にエラーで検知し、不正なスキーマ定義を未然に防ぎたい

#### Acceptance Criteria

1. The Stratum schema validator shall 各データベース方言に対応したJSON Schemaファイル(例: `postgres-types.schema.json`, `mysql-types.schema.json`, `sqlite-types.schema.json`)を保持する
2. When スキーマパース時に方言固有の`kind`を検証する際, the Stratum schema validator shall 対応するJSON Schemaに基づいて型名とパラメータの妥当性を検証する
3. If 指定された方言に存在しない`kind`が検出された場合, then the Stratum schema validator shall エラーメッセージ(型名、方言名、利用可能な型のリストを含む)を出力し、パースを中断する
4. If 型パラメータが方言の仕様に違反している場合(例: PostgreSQLで`VARCHAR`の長さが上限を超える), then the Stratum schema validator shall パラメータ違反を示す詳細なエラーメッセージを出力する
5. The Stratum schema validator shall JSON Schemaの更新により新しい方言型を追加可能な拡張性を持つ

### Requirement 3: 後方互換性の維持

**Objective:** 既存ユーザーとして、現在の共通`ColumnType` enum定義を用いたスキーマファイルが引き続き動作することを保証したい

#### Acceptance Criteria

1. When 既存の共通型(例: `INTEGER`, `VARCHAR`, `TEXT`)がYAMLスキーマに記述されている場合, the Stratum schema parser shall 従来通りの変換ロジック(`to_sql_type()`相当)で各方言のSQL型にマッピングする
2. The Stratum migration generator shall 新しい方言固有型定義と従来の共通型定義の両方をサポートし、混在したスキーマファイルでも正しくマイグレーションSQLを生成する
3. If ユーザーが明示的に方言固有型を指定していない場合, then the Stratum schema parser shall デフォルトで共通型として解釈し、現行の動作を維持する

### Requirement 4: SQL生成ロジックの最適化

**Objective:** 開発者として、方言固有型が指定された場合に各データベースに最適化されたDDL文を生成したい

#### Acceptance Criteria

1. When 方言固有の`kind`が指定されている場合, the SQL generator shall 指定された型名をそのままSQL DDL文に出力する(例: PostgreSQLで`SERIAL`が指定されたら`CREATE TABLE ... id SERIAL`を生成し、型変換や展開は行わない)
2. When マイグレーションファイルを生成する際, the Stratum migration generator shall 方言ごとに最適化されたup/down SQLを出力する
3. The SQL generator shall 方言固有の`kind`を受け取った場合、データベースエンジン側の処理に委ね、Stratum側では型変換や内部展開を行わない
4. If 方言固有型が他の方言でサポートされていない場合, then the Stratum schema validator shall マイグレーション生成前に警告またはエラーを出力する

### Requirement 5: エラーメッセージの改善

**Objective:** スキーマ開発者として、型エラーが発生した際に原因を素早く特定し修正できるよう、明確で実用的なエラーメッセージを受け取りたい

#### Acceptance Criteria

1. If 方言に存在しない型が指定された場合, then the Stratum error handler shall エラーメッセージに以下の情報を含める: 不正な型名、対象の方言、対象ファイルのパスと行番号、利用可能な型のサンプルリスト
2. If 型パラメータが不足または不正な場合(例: `VARCHAR`に`length`が未指定), then the Stratum error handler shall 必要なパラメータ名とその形式を明示したエラーメッセージを出力する
3. The Stratum CLI shall 型検証エラー発生時に適切な終了コード(例: 1)を返し、CIパイプラインでの検出を可能にする
4. When 複数の型エラーが同時に存在する場合, the Stratum schema validator shall すべてのエラーを一度に収集して表示し、修正の手戻りを最小化する

### Requirement 6: ドキュメントとサンプルの提供

**Objective:** 新規ユーザーとして、方言固有型の定義方法を理解し、実際のプロジェクトで活用するための参考資料を入手したい

#### Acceptance Criteria

1. The Stratum documentation shall 各データベース方言でサポートされる型の一覧とYAMLでの記述例を含むリファレンスドキュメントを提供する
2. The Stratum example directory shall PostgreSQL, MySQL, SQLiteそれぞれの方言固有型を使用したサンプルスキーマファイル(例: `postgres_advanced.yaml`)を含む
3. The Stratum migration guide shall 既存の共通型定義から方言固有型への移行手順を説明するドキュメントを提供する
4. The Stratum error message documentation shall よくある型エラーとその解決方法をまとめたトラブルシューティングガイドを提供する
