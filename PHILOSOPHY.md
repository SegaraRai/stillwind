# PHILOSOPHY of Stillwind

This document describes the principles and design philosophy of this project.
To reduce the maintenance burden on maintainers, this file is written in Japanese.
As a general rule, this file should only be edited by humans.
All implementations and documentation in this project must adhere to these principles.
The principles themselves will also be updated as the implementation evolves.

---

- このファイルの存在意義
  - コードもドキュメントも AI に書かせるようになった結果、自分の本当の意見がどこにも残らなくなった
    - 都度監督して軌道修正はさせてるが、もはや全部を見きれない
  - AI を介した情報は得てしてブレるし、無駄に長くなる
  - 思想の原液をコンパクトに残しておかないと歪んでいきそうという危機感
    - _この危機感を抱いてから書き始めたのでまだ書きかけ_

- コンセプト
  - バンドラ使用を前提とした Tailwind CSS
    - Tailwind CSS v4 を JS や FW にニュートラルな方向に舵を切ったものと捉えるなら、Stillwind はむしろそれらの使用を前提にしたもの
    - この前提を加えることで、Tailwind ではできなかったことができるようになる（＆Tailwind でできていたことは一部できなくなる）という仮説
      - たとえばバンドラのない環境やビルドツールを用いないプロジェクトは意図的にスコープ外
    - Tailwind がやめた JS config file を Stillwind で復活させたのも、この延長線上として捉えることができる
  - AI agent 時代の堅牢な CSS エンジン
    - いまの AI はだいたい下記のことをしでかす（2026年10月現在）
      - 任意のブレークポイント使う（`max-[768px]:` とか）
      - 任意の色を使う（`text-[#abcdef]` みたいな）
    - AI に限らず思想を持って作ると面倒なことがある
      - `group` や `peer` につける名前が枯渇したり、どうネストされるかを考えて一意性を担保しないといけないとか
        - これらの名前間違えても検査できないし
      - keyframes や view transition をグローバルで定義するとだんだんカオスになってくとか
      - CSS anchor positioning の名前の話とか
        - `attr()` でなんとかできるようになりつつはあるけど……
      - JS で media query やるときにブレークポイントの値が重複定義になるとか
    - Tailwind は文字列走査なのでやれることに限界がある（そのぶんどこに書くかとかは柔軟だけど）
  - 設計キーワード
    - SSOT: 重複定義をしない。意味論に基づいて定義する（偶然の一致はまとめない）。
    - モジュラー: グローバル空間を汚さない。コードのまとまりは容易に追加、削除、変更、移動可能にする。
    - コロケーション: スタイル定義は DOM 要素のすぐ近くに配置する。Web アプリでは文書とスタイルの分離は不可能。スタイル定義は DOM 構造に引っ張られる（密結合する）ので、近い場所に書くべき。
    - シングルトンなし: シングルトンは存在しない。あらゆるものは何かを親とする。いわゆるシングルトンはプロセスインスタンスが親であるとみなせる。親を何にするか（つまり、そのオブジェクトのインスタンスはper-何で管理すべきか）を慎重に見極める。プロセスインスタンスを親とするべきケースはかなり稀（キャッシュや WeakMap など、そうすべき場合もある）。
  - あと個人的な思想の押し付け
    - CSS でできることは CSS でやる。JS でやらない。
    - 固定クラス名への依存をしない。全部ユーティリティクラスなら `hover:` とか任意のバリアントに載せられて素敵だよね。
    - CSS クラスのトグルは好きじゃない。
      - 要素の状態に依存するものは `disabled:` とか `aria-pressed:` とかそういうので表すべき
      - バリアントの違いは CSS カスタム変数でやるべき
      - これらができない場合は `data-*` 属性で指示すればいい
      - そうすれば要素に付与する CSS クラスは常に固定で済む。カスケード順みたいな面倒なこともあまり考えなくて良くなる。
        - まあそうはいっても Stillwind は条件式に基づくカスケード順の配置をやってるんだけどね。これらは万人受けする思想じゃないだろうから、そうじゃない思想向けに（AI もそういうコード書くし）。
  - あと面白そうなことやりたい
    - CSS if とか。実装したけどあれ実用性あんのかな。

- 通常使用における Tailwind CSS v4 との 100% 互換性は必須
  - 移行しやすくするため。style() 呼び出しが必要なので drop-in にはできないが、中身の検証が不要であれば大幅に省力化できる。
  - ユーザーのリポジトリにあわせてコーディングエージェントに専用＆使い捨ての codemod を書かせて一括で変換もできるだろう
  - 継続的に Tailwind CSS の更新を追う＆静的に互換性と網羅性の検証ができるようにしておく（セットアップ済み）

- Tailwind CSS v4 で未対応の新規 CSS プロパティの先行対応は慎重にしたい
  - たとえば `@import "stillwind/experimental/position-anchor";` みたいな感じで明示的に import させるべき
  - Tailwind CSS v4 に実際にサポートされた際に構文の差異が発生しないようにしたいので

- Stillwind において必要なのは Agent DX を最大化すること
  - なので CLI で lint や format などができる必要がある
    - Tailwind は VSCode 拡張でしかできないことがあるので
  - Eval でベンチマークを取る必要がある
  - このへんは Hono の手法を参考にする

- LSP なども人間向けのデモやヒューマンレビュー用として残す必要がある
  - コード品質に拘る場合、エディタでコードを見ることがあり、その際 LSP や拡張機能がないとしんどい
