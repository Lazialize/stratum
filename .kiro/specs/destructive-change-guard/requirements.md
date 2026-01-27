# 要件ドキュメント

## プロジェクト概要（入力）
破壊的変更の安全ガード（drop/rename/enum削除の明示フラグ + dry-run差分プレビュー）

## 導入

### 背景
Strataは、データベーススキーマをコードとして管理するIaCツールです。現在、`strata generate` と `strata apply` コマンドは、テーブル削除、カラム削除、カラムリネーム、ENUM削除などの破壊的なスキーマ変更を自動的に検出し、SQLを生成・実行します。しかし、これらの操作は誤って実行されると重大なデータ損失を引き起こす可能性があるため、明示的な確認なしに実行されるべきではありません。

### 目的
本仕様では、破壊的変更を検出し、ユーザーが明示的に許可しない限りマイグレーション生成と適用を拒否する安全ガード機能を実装します。既存のdry-runプレビュー機能を拡張し、破壊的変更を視覚的に明示することで、誤操作によるデータ損失を防ぎます。

### スコープ
- **対象**: `strata generate` および `strata apply` コマンド
- **破壊的変更の定義**: テーブル削除、カラム削除、カラムリネーム、ENUM削除、ENUM再作成
- **アプローチ**: デフォルト拒否 + 明示的な許可フラグ

---

## 要件

### 要件1: 破壊的変更の検出と分類

**目的:** システム開発者として、スキーマ差分から破壊的変更を自動検出し、その種類を分類したい。これにより、どのような破壊的操作が含まれるかを明確に把握できる。

#### 受入基準

1. When スキーマ差分検出が実行される場合、the Strataシステム shall 以下の5種類の破壊的変更を検出する
   - テーブル削除（`SchemaDiff.removed_tables`）
   - カラム削除（`TableDiff.removed_columns`）
   - カラムリネーム（`TableDiff.renamed_columns`）
   - ENUM削除（`SchemaDiff.removed_enums`）
   - ENUM再作成（`EnumChangeKind::Recreate`）

2. When 破壊的変更が検出される場合、the Strataシステム shall 各変更の種類ごとに影響範囲（テーブル名、カラム名、ENUM名）をリスト化する

3. When 複数の破壊的変更が含まれる場合、the Strataシステム shall すべての変更を漏れなく検出しリスト化する

4. The Strataシステム shall 破壊的変更の検出結果を `DestructiveChangeDetector` サービスとして実装する

5. The `DestructiveChangeDetector` shall スキーマ差分（`SchemaDiff`）を入力として受け取り、破壊的変更のリスト（`DestructiveChangeReport`）を返す

---

### 要件2: デフォルト拒否メカニズム

**目的:** システム運用者として、破壊的変更を含むマイグレーションがデフォルトで拒否されることを期待する。これにより、誤操作によるデータ損失を防止できる。

#### 受入基準

1. When `strata generate` が破壊的変更を検出し、かつ明示的な許可フラグが指定されていない場合、then the generateコマンド shall マイグレーション生成を中止し、エラーメッセージを返す

2. When `strata apply` が破壊的変更を含むマイグレーションを実行しようとし、かつ明示的な許可フラグが指定されていない場合、then the applyコマンド shall マイグレーション適用を中止し、エラーメッセージを返す

3. When 破壊的変更が検出されエラーが返される場合、the Strataシステム shall 検出された破壊的変更の種類と影響範囲を一覧表示する

4. When デフォルト拒否が発動する場合、the Strataシステム shall 適切な許可フラグ（`--allow-destructive`等）をエラーメッセージ内で提案する

5. When 破壊的変更が検出されない場合、the generateコマンド および applyコマンド shall 通常通りマイグレーション生成・適用を実行する

---

### 要件3: 明示的な許可フラグ

**目的:** システム管理者として、破壊的変更の実行を明示的に許可するフラグを使用したい。これにより、慎重に判断した上で破壊的変更を実行できる。

#### 受入基準

1. When `strata generate --allow-destructive` が実行される場合、the generateコマンド shall 破壊的変更を含むマイグレーションファイルを生成する

2. When `strata apply --allow-destructive` が実行される場合、the applyコマンド shall 破壊的変更を含むマイグレーションを適用する

3. When `--allow-destructive` フラグが指定される場合、the Strataシステム shall デフォルト拒否メカニズムをバイパスする

4. The generateコマンド および applyコマンド shall `--allow-destructive` フラグをCLI引数として受け付ける

5. When 破壊的変更が検出され、かつ `--allow-destructive` フラグが指定されている場合、the Strataシステム shall 警告メッセージを表示する（エラーではなく警告）

6. Where dry-runモードが有効な場合（`--dry-run`）、the Strataシステム shall 許可フラグの有無に関わらず破壊的変更をプレビュー表示する（実行はしない）

---

### 要件4: dry-run差分プレビューの拡張

**目的:** 開発者として、dry-runモードで破壊的変更を視覚的に明示したい。これにより、実行前にリスクを確認できる。

#### 受入基準

1. When `strata generate --dry-run` が破壊的変更を検出する場合、the generateコマンド shall プレビュー出力に「⚠ Destructive Changes Detected」セクションを追加する

2. When 破壊的変更が表示される場合、the Strataシステム shall 各変更の種類（DROP TABLE、DROP COLUMN、RENAME COLUMN等）を色付きテキスト（赤色）で強調表示する

3. When dry-runプレビューが表示される場合、the Strataシステム shall 破壊的変更の影響範囲（削除されるテーブル数、カラム数）を集計して表示する

4. When `strata apply --dry-run` が破壊的変更を含むマイグレーションを検出する場合、the applyコマンド shall 実行されるSQL文の中で破壊的なSQL（`DROP`, `RENAME COLUMN`等）を強調表示する

5. When dry-runプレビューが表示される場合、the Strataシステム shall 「To proceed, run with --allow-destructive flag」という指示を表示する

6. The dry-runプレビュー shall 既存のリネームプレビュー機能（Task 6.2）と統合し、一貫した表示形式を維持する

---

### 要件5: エラーメッセージと修正提案

**目的:** ユーザーとして、破壊的変更が拒否された際に、具体的な理由と次のアクションを知りたい。これにより、適切な対応を迅速に行える。

#### 受入基準

1. When 破壊的変更によりマイグレーション生成が拒否される場合、the generateコマンド shall 以下の情報を含むエラーメッセージを表示する
   - 検出された破壊的変更の種類と数
   - 影響を受けるテーブル/カラム/ENUMの一覧
   - 実行を許可するための正確なコマンド例（例: `strata generate --allow-destructive`）

2. When 破壊的変更によりマイグレーション適用が拒否される場合、the applyコマンド shall エラーメッセージに以下を含める
   - 適用しようとしているマイグレーションバージョン
   - 含まれる破壊的変更の詳細
   - `--dry-run` フラグでプレビュー確認する推奨手順

3. When エラーメッセージが表示される場合、the Strataシステム shall 色付きテキスト（赤色）を使用してエラーを視覚的に強調する

4. When 複数の破壊的変更が含まれる場合、the Strataシステム shall 変更種別ごとにグループ化して表示する（例: "Tables to be dropped: users, posts"）

5. The エラーメッセージ shall ユーザーに対して以下の選択肢を提示する
   - dry-runでプレビュー確認する（`--dry-run`）
   - 破壊的変更を許可して実行する（`--allow-destructive`）
   - スキーマ定義を見直す

---

## 非機能要件

### パフォーマンス
1. The 破壊的変更検出処理 shall スキーマ差分検出の総実行時間に対して10%以内のオーバーヘッドで完了する

### 後方互換性
1. The 新機能 shall 既存の `strata generate` および `strata apply` のCLI引数と競合しない
2. Where 既存のdry-runフラグが使用される場合、the Strataシステム shall 既存の動作を維持しつつ、破壊的変更のプレビューを追加表示する

### セキュリティ
1. The Strataシステム shall 破壊的変更の検出ロジックにおいて、SQLインジェクションやパストラバーサルの脆弱性を持たない

---

## 用語集

- **破壊的変更（Destructive Change）**: データベーススキーマの変更のうち、データ損失やアプリケーション互換性の喪失を引き起こす可能性のある操作。具体的には、テーブル削除、カラム削除、カラムリネーム、ENUM削除、ENUM再作成を指す。
- **デフォルト拒否（Deny by Default）**: 明示的な許可がない限り、危険な操作を実行しないセキュリティ原則。
- **明示的な許可フラグ（Explicit Allow Flag）**: ユーザーが意図的に危険な操作を許可するために指定するCLI引数（例: `--allow-destructive`）。

---

## 依存関係

- **既存機能**:
  - スキーマ差分検出（`SchemaDiffDetector`）
  - dry-runプレビュー表示（`GenerateCommand.execute_dry_run`, `ApplyCommand`）
  - リネームプレビュー（Task 6.2実装済み）

- **新規実装が必要なコンポーネント**:
  - `DestructiveChangeDetector` サービス（破壊的変更検出）
  - `DestructiveChangeReport` モデル（検出結果の表現）
  - CLI引数パーサーへの `--allow-destructive` 追加
  - エラーメッセージフォーマッター（破壊的変更用）

---

generated_at: 2026-01-25T15:41:41Z
