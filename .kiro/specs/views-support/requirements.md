# Requirements Document

## Introduction
本仕様は、Strata における View（ビュー）サポートの要件を定義する。YAML での宣言、検証、差分検出、マイグレーション生成・適用、既存DBからのエクスポートまでを、既存のスキーマ管理ワークフローに統合する。

## Requirements

### Requirement 1: ビュー定義の宣言
**Objective:** As a スキーマ管理者, I want スキーマファイルでビューを宣言できる, so that ビューをコードとして管理できる

#### Acceptance Criteria
1. When スキーマ定義にビューが記載されている, the Strata system shall ビュー定義として読み取り可能にする
2. If ビュー定義に必須情報（名称とdefinitionのSQL）が欠けている, then the Strata system shall 検証エラーとして報告する
3. The Strata system shall ビュー定義のYAML形式として名称とdefinitionを必須キーとして要求する
4. While 既存の命名規則検証が有効である, the Strata system shall ビュー名が既存規則に合致することを検証する
5. If ビュー名が既存のテーブル名または他のビュー名と衝突する, then the Strata system shall 検証エラーとして報告する
6. The Strata system shall ビュー定義をテーブル定義と同一のスキーマバージョン管理単位に含める
7. The Strata system shall YAML検証用のJSON Schemaにビュー定義を含める

### Requirement 2: ビュー検証
**Objective:** As a スキーマ管理者, I want ビューの依存関係が検証される, so that 参照不整合を事前に検出できる

#### Acceptance Criteria
1. When ビュー定義にdepends_onが指定されている, the Strata system shall 依存先のテーブルまたはビューが同一スキーマ内に存在することを検証する
2. If depends_onで指定されたオブジェクトがスキーマ内に存在しない, then the Strata system shall 検証エラーとして報告する
3. If ビュー定義にdepends_onが指定されていない, then the Strata system shall SQLパースによる参照解析を行わない
4. While 既存の破壊的変更検出が有効である, the Strata system shall ビュー削除や定義変更が依存関係に影響する可能性を既存ポリシーの警告対象に含める
5. The Strata system shall ビュー定義が空文字または無効な形式である場合に検証エラーとして報告する
6. If ビュー依存関係に循環がある, then the Strata system shall 検証エラーとして報告する

### Requirement 3: ビューの差分検出とマイグレーション生成
**Objective:** As a スキーマ管理者, I want ビューの変更から差分とマイグレーションが生成される, so that 変更を安全に適用できる

#### Acceptance Criteria
1. When 新しいビューがスキーマに追加される, the Strata system shall ビュー作成の差分を検出する
2. When 既存ビューの定義が変更される, the Strata system shall ビュー更新の差分を検出する
3. When ビューがスキーマから削除される, the Strata system shall ビュー削除の差分を検出する
4. When ビュー定義の差分判定を行う, the Strata system shall SQL文字列の正規化ルールを適用して差分を判定する
5. The Strata system shall 既存のマイグレーション生成機能に準拠してビューの差分に対応するマイグレーションを生成する
6. While ビューが他のビューに依存している, the Strata system shall 依存関係に基づいてマイグレーションの適用順序を整合させる
7. When ビューのrenameが指定される, the Strata system shall 既存のリネーム追跡と同等の扱いで差分として検出する
8. When ビューの更新マイグレーションを生成する and DB方言がPostgreSQLまたはMySQLである, the Strata system shall CREATE OR REPLACE VIEW を用いる
9. When ビューの更新マイグレーションを生成する and DB方言がSQLiteである, the Strata system shall DROP VIEW と CREATE VIEW を用いる

### Requirement 4: ビューの適用とロールバック
**Objective:** As a スキーマ管理者, I want ビュー変更が安全に適用・ロールバックされる, so that 運用中のリスクを抑えられる

#### Acceptance Criteria
1. When 生成されたマイグレーションが適用される, the Strata system shall ビューの作成・更新・削除を反映する
2. When ロールバックが実行される, the Strata system shall ビュー変更を元の状態に戻す
3. If 既存の破壊的変更ブロック設定が有効である, then the Strata system shall ビュー削除を含むマイグレーションの適用を停止する
4. The Strata system shall ビューに関するマイグレーションの結果を適用履歴として記録する
5. The Strata system shall ビュー更新のロールバックに必要な前定義をマイグレーションに含める

### Requirement 5: ビューのエクスポート
**Objective:** As a スキーマ管理者, I want 既存DBのビューをスキーマに取り込める, so that 現行状態からコード管理へ移行できる

#### Acceptance Criteria
1. When 既存のエクスポート機能でDBからスキーマをエクスポートする, the Strata system shall ビュー定義をスキーマに含める
2. If DB 方言でサポートされないビュー定義が検出される, then the Strata system shall 互換性のない要素として警告またはエラーを報告する
3. The Strata system shall エクスポートされたビュー定義が後続の差分検出に利用できる形式で保存する

### Requirement 6: 既存機能との整合
**Objective:** As a スキーマ管理者, I want ビュー機能が既存のワークフローと一貫して動作する, so that 学習コストを増やさずに運用できる

#### Acceptance Criteria
1. The Strata system shall 既存のCLIワークフロー（validate/generate/apply/rollback/status/export）でビューを同等に扱う
2. While 既存の検証ルールが適用される, the Strata system shall ビュー定義に対して同じエラーレポート形式を使用する
3. When 既存の破壊的変更レポートが生成される, the Strata system shall ビュー変更を既存フォーマットで含める
4. If マテリアライズドビューが定義されている, then the Strata system shall 未サポートとしてエラーを報告する
