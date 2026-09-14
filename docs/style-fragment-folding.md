# Style fragment folding

`style()` は [マクロ引数の契約](macro-contract.md) を前提として縮約する。
引数の純粋性は利用者の契約であり、frontend が通常の呼び出しごとに証明するものではない。
明白な契約違反は frontend のコンパイルエラーとなる。

## 処理と責務

1. CSS lowering が class identity と CSS の順序を確定する。
2. `fragment/expression.rs` が寄与を `Fragment` にまとめる。
3. `fragment/fold.rs` が consumer の要求と表現可能性を検査する。
4. `fragment/render.rs` が採用した構造を `Code` として描画し、必要な helper を登録する。
5. transform と host が式のマッピングを最終 source map に変換する。

`Demand::Fragment` は symbolic key による合成途中の値、`Demand::Props` は FW ごとの
`class` / `className` と object / string style を持つ最終値を要求する。
子へ最終 props の要求を無条件に伝播せず、条件分岐の下では symbolic fragment を使用する。

## 縮約規則

- 空の寄与を取り除き、sequence を再帰的に平坦化する。
- class・custom property・attribute を表現可能な record にまとめる。
- 評価順への依存は禁止されているため、通常の関数呼び出しが交互に現れてもまとめられる。
- class の連結順と同名 property の上書き順を維持する。最終 props のキー順は runtime と一致させる。
- 空 class の有無、通常の `style` と custom property の混在、文字列 style の同名 custom property
  など、表現の同値性を証明できない場合は一般形へ戻す。
- opaque props / style payload と値を持つ条件分岐は境界として保持する。
- 両枝が空の条件式は呼び出しを含んでいても除去する。
- clsx の引数位置では compiler-owned 配列と class group を展開し、空要素を取り除く。
- `{ class: … }` の配列リテラルも frontend で再帰的に引数列へ展開し、穴と空配列を除く。
  spread を含む配列は iterable の意味を保つため丸ごと保持し、条件式内の配列も枝内に残す。
- 引数位置の deferred clsx は外側の clsx に統合する。条件分岐内の helper を枝の外へ出さない。
- 元ソース由来の opaque 引数文字列は再解析・展開しない。

例えば以下は直接 props にできる。

```ts
style({ "--x": f() }, { "data-value": g() }, { "--y": h() });
// {"data-value":g(),style:{"--x":stillwindCssValue(f()),"--y":stillwindCssValue(h())}}
```

また、class 値の合成は次のように縮約できる。

```js
stillwindClsx("base", [null, flag && "busy"], stillwindClsx(external));
// stillwindClsx("base", flag && "busy", external)
```

表現できない場合の fallback は部分縮約済み fragment を根の `stillwindMerge` へ渡す。
不正な入力を fallback によって受理しない。表現選択は CSS identity や import を副作用的に変更せず、
採用計画を描画するときだけ helper の binding を割り当てる。

## ソースマッピング

`Code` は生成テキストと出現ごとの元ソース区間を保持する。同じ式の文字列でも出現を統合しない。
引数の移動は `Code::join` で生成位置だけを移動し、元の span を保持する。
生成 syntax / helper / key は synthetic として扱う。削除した式は mapping も削除する。
transform は UTF-8 の対応を検証して絶対 UTF-16 座標へ変換し、host が prefix や import 除去を反映する。

## 検証

契約の診断は frontend の AST と projected input に対して検証する。
folding は FW ごとの結果・キー順・重複値・空値・条件分岐を一般形と比較する。
実行比較の入力関数・getter・変換は契約を満たす安定した値を返す。
副作用のログ順や意図的な例外順を比較して通常の関数呼び出しの最適化を禁止しない。
source map は並べ替え、同じ文字列の別出現、非 ASCII の条件式、属性エンコードを検証する。
