# Requirements Document

## Introduction

本ドキュメントでは、Stratumのスキーマ定義YAML構文の改善に関する要件を定義します。主な変更点として、テーブル名の冗長性排除、主キー定義の独立化、およびオプショナルフィールドの適切な処理があります。これらの変更により、スキーマ定義の簡潔性と直感性が向上し、ユーザー体験が改善されます。

## Requirements

### Requirement 1: テーブル名としてYAMLキー名を使用

**Objective:** As a バックエンドエンジニア, I want YAMLのキー名をテーブル名として使用したい, so that スキーマ定義が簡潔になり、テーブル名の重複記述を避けられる

#### 現在の構文
```yaml
tables:
  users:
    name: users  # 冗長な重複
    columns:
      ...
```

#### 新しい構文
```yaml
tables:
  users:  # このキー名がテーブル名として使用される
    columns:
      ...
```

#### Acceptance Criteria
1. When YAMLファイルがパースされる, the Schema Parser shall `tables`配下のキー名をテーブル名として使用する
2. The Schema Parser shall テーブル定義内の`name`フィールドを廃止する
3. When スキーマがシリアライズされる, the Schema Serializer shall `name`フィールドを出力しない

### Requirement 2: 主キー（primary_key）のconstraintsからの独立

**Objective:** As a バックエンドエンジニア, I want 主キーをconstraintsとは別のトップレベルフィールドとして定義したい, so that 主キーが他の制約と区別され、テーブル定義が直感的になる

#### 現在の構文
```yaml
tables:
  users:
    columns:
      - name: id
        ...
    constraints:
      - type: PRIMARY_KEY
        columns:
          - id
```

#### 新しい構文
```yaml
tables:
  users:
    columns:
      - name: id
        ...
    primary_key:
      - id
```

#### Acceptance Criteria
1. When テーブル定義に`primary_key`フィールドがある, the Schema Parser shall そのフィールドを主キーカラムのリストとして解釈する
2. When `primary_key`がパースされる, the Schema Parser shall 内部的にPRIMARY_KEY制約として変換する
3. The Schema Parser shall `constraints`内の`PRIMARY_KEY`タイプを廃止する
4. When SQLが生成される, the SQL Generator shall `primary_key`フィールドから正しいPRIMARY KEY制約を出力する
5. The Schema Serializer shall 主キーを`primary_key`フィールドとして出力する

### Requirement 3: indexesフィールドのオプショナル化

**Objective:** As a バックエンドエンジニア, I want indexesフィールドが未定義でもエラーにならないようにしたい, so that インデックスが不要なシンプルなテーブルを簡潔に定義できる

#### 期待する動作
```yaml
tables:
  simple_table:
    columns:
      - name: id
        ...
    primary_key:
      - id
    # indexes を省略可能
```

#### Acceptance Criteria
1. When `indexes`フィールドが省略されている, the Schema Parser shall エラーなくパースを完了する
2. When `indexes`フィールドが省略されている, the Schema Parser shall 空のインデックスリストとしてスキーマを構築する
3. When `indexes`フィールドが空配列として定義されている, the Schema Parser shall エラーなくパースを完了する
4. When `indexes`を持たないテーブルが処理される, the SQL Generator shall インデックス関連のSQLを出力しない
5. The Schema Validator shall `indexes`フィールドの有無に関わらずバリデーションを成功させる

### Requirement 4: constraintsフィールドのオプショナル化

**Objective:** As a バックエンドエンジニア, I want constraintsフィールドが未定義でもエラーにならないようにしたい, so that 制約が不要なシンプルなテーブルを簡潔に定義できる

#### 期待する動作
```yaml
tables:
  log_table:
    columns:
      - name: id
        ...
      - name: message
        ...
    primary_key:
      - id
    # constraints を省略可能（UNIQUE, FOREIGN_KEY等が不要な場合）
```

#### Acceptance Criteria
1. When `constraints`フィールドが省略されている, the Schema Parser shall エラーなくパースを完了する
2. When `constraints`フィールドが省略されている, the Schema Parser shall 空の制約リストとしてスキーマを構築する
3. When `constraints`フィールドが空配列として定義されている, the Schema Parser shall エラーなくパースを完了する
4. When 制約を持たないテーブルが処理される, the SQL Generator shall 制約関連のSQL（PRIMARY KEY以外）を出力しない
5. The Schema Validator shall `constraints`フィールドの有無に関わらずバリデーションを成功させる
6. When `primary_key`フィールドが定義されている, the Schema Parser shall `constraints`が空でも主キー制約を適用する

### Requirement 5: エラーメッセージとバリデーション

**Objective:** As a バックエンドエンジニア, I want 構文エラー時に明確なエラーメッセージを受け取りたい, so that スキーマ定義の問題を迅速に特定・修正できる

#### Acceptance Criteria
1. If `primary_key`に存在しないカラム名が指定されている, then the Schema Validator shall 該当カラム名を含むエラーメッセージを返す
2. If `columns`フィールドが省略されている, then the Schema Parser shall カラム定義が必須であることを示すエラーを返す
3. When 不正な構文が検出される, the Schema Parser shall 行番号またはフィールドパスを含むエラーメッセージを返す
