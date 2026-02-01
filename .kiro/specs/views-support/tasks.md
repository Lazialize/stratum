# Implementation Plan

- [x] 1. View のスキーマ表現と入出力の拡張
- [x] 1.1 View ドメインモデルと Schema への統合
  - View に name/definition/depends_on/renamed_from を持たせる
  - Schema に views を追加し、テーブル/ENUM と同一のスキーマ単位で管理できるようにする
  - _Requirements: 1.1, 1.2, 1.3, 1.6_
- [x] 1.2 DTO/YAML 入出力の拡張
  - YAML の views 定義を読み書きできるようにする
  - definition が欠落した場合に検証エラーへつなげる
  - _Requirements: 1.1, 1.2, 1.3, 1.6_
- [x] 1.3 JSON Schema の更新
  - views セクションに definition/depends_on を追加する
  - _Requirements: 1.7_

- [x] 2. View 検証と依存関係の管理
- [x] 2.1 (P) 命名規則と衝突の検証
  - 既存の命名規則検証に View 名を追加する
  - テーブル名/他ビュー名との衝突をエラーにする
  - _Requirements: 1.4, 1.5_
- [x] 2.2 (P) depends_on 依存検証と循環検出
  - depends_on の参照先が tables/views に存在するか検証する
  - トポロジカルソートで循環を検出し、宣言順の安定ソートを採用する
  - depends_on 未指定時は SQL 参照解析を行わない
  - _Requirements: 2.1, 2.2, 2.3, 2.6_
- [x] 2.3 (P) definition の妥当性検証
  - 空文字/無効形式を検証エラーとして扱う
  - _Requirements: 2.5_

- [x] 3. View 差分検出の拡張
- [x] 3.1 (P) 追加/更新/削除/rename の差分抽出
  - View の追加/更新/削除/rename を差分として検出する
  - _Requirements: 3.1, 3.2, 3.3, 3.7_
- [x] 3.2 (P) definition 正規化比較
  - 空白/改行/連続スペースのみを正規化して差分判定する
  - _Requirements: 3.4_

- [x] 4. View マイグレーション生成と方言別 SQL
- [x] 4.1 マイグレーション生成ステージの追加
  - View 差分をマイグレーションに変換する
  - depends_on に基づいて適用順を整合させる
  - _Requirements: 3.5, 3.6_
- [x] 4.2 方言別の更新 SQL 生成
  - PostgreSQL/MySQL は CREATE OR REPLACE VIEW を使う
  - SQLite は DROP VIEW + CREATE VIEW を使う
  - _Requirements: 3.8, 3.9_
- [x] 4.3 ロールバック用の旧定義保持
  - View 更新時に down.sql へ旧定義を含める
  - _Requirements: 4.2, 4.5_

- [x] 5. 適用・破壊的変更・履歴の統合
- [x] 5.1 View の apply/rollback 連携
  - 生成済み SQL を適用し、履歴に記録する
  - _Requirements: 4.1, 4.4_
- [x] 5.2 破壊的変更レポートへの組み込み
  - View 削除/変更を破壊的変更として扱う
  - 既存ポリシーに従って警告/ブロックする
  - _Requirements: 2.4, 4.3, 6.3_

- [x] 6. View のエクスポート対応
- [x] 6.1 (P) DB から View 定義を取得
  - 方言ごとの取得実装を追加し Schema に含める
  - _Requirements: 5.1, 5.3_
- [x] 6.2 (P) 非対応要素の検出
  - マテリアライズドビュー等を未サポートとしてエラー/警告にする
  - _Requirements: 5.2, 6.4_

- [x] 7. 既存 CLI ワークフローとの整合
- [x] 7.1 CLI 出力とエラーフォーマットの整合
  - validate/generate/apply/rollback/status/export で View を同等に扱う
  - 既存のエラーレポート形式を維持する
  - _Requirements: 6.1, 6.2_

- [x] 8. テスト追加
- [x] 8.1 Unit: モデルとバリデーション
  - View のシリアライズ/デシリアライズ
  - 命名規則・衝突・depends_on・循環の検証
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 2.5, 2.6_
- [x] 8.2 Integration: 差分と SQL 生成
  - 差分検出（追加/更新/削除/rename）
  - 方言別 SQL と依存順序の適用
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.6, 3.7, 3.8, 3.9_
- [x] 8.3 E2E: ワークフロー統合
  - validate→generate→apply→rollback の一連フロー
  - export で View が YAML に含まれること
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.3, 6.1_
