# Implementation Plan

- [x] 1. CLI 入力とコマンド導線を追加する
- [x] 1.1 check コマンドの入力仕様を定義し、既存の設定/入力経路を継承できるようにする
  - schema_dir を指定できる入力を追加する
  - 既存の config と format などのグローバル入力を同等に扱う
  - validate と generate に同一の入力が渡る前提を明文化する
  - _Requirements: 2.1, 2.2, 2.3_
- [x] 1.2 check 実行がハンドラーへ到達し、終了コードが返る導線を整える
  - 成功時と失敗時の終了コードの扱いを統一する
  - 失敗時はエラー出力が一貫するようにする
  - _Requirements: 1.1, 4.1, 4.2, 4.3_

- [x] 2. check 実行フローと出力統合を実装する
- [x] 2.1 validate 成功時のみ generate dry-run を実行する制御を実装する
  - validate の失敗時は generate を実行しない
  - 既存 validate/generate の意味論に合わせた結果を返す
  - dry-run でのみ実行され、ファイル生成が発生しないことを保証する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 5.1, 5.2_
- [x] 2.2 CheckOutput による Text/JSON の統一出力を実装する
  - JSON では validate と generate をネストし、summary で成否を示す
  - validate 失敗時は generate を null にして失敗箇所を明確化する
  - Text 出力はセクション分離と成功/失敗メッセージを明示する
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.1, 4.2, 4.3_

- [x] 3. generate dry-run の入力拡張を行う
- [x] 3.1 schema_dir 上書きが generate に反映されるようにする
  - 指定がない場合は既存の config の挙動を維持する
  - check から渡された入力が正しく反映される前提を整える
  - _Requirements: 2.2, 2.3, 1.4_

- [x] 4. テストで check の主要フローを担保する
- [x] 4.1 validate 成功/失敗分岐の動作を確認する
  - 成功時にのみ dry-run が実行されることを検証する
  - 失敗時に終了コードが失敗を示すことを検証する
  - _Requirements: 1.1, 1.2, 1.3, 4.1, 4.2_
- [x] 4.2 出力フォーマットの構造を確認する
  - JSON のネスト構造と summary の値を検証する
  - Text 出力の成功/失敗メッセージが明示的であることを確認する
  - _Requirements: 3.1, 3.2, 3.3, 3.4_
- [x] 4.3 schema_dir 上書きが validate と generate に反映されることを確認する
  - 入力継承が有効であることを検証する
  - _Requirements: 2.1, 2.2, 2.3_
- [x] 4.4 dry-run が非破壊であることを確認する
  - ファイル生成やマイグレーション適用が発生しないことを検証する
  - _Requirements: 5.1, 5.2_
