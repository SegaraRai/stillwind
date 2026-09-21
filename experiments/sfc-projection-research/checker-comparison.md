# SFC checker comparison: TypeScript 6 and 7

2026-09-21 に実施。結論は **checker は TypeScript 7.0.2 を維持する**。
TypeScript 6.0.3 へ戻すことで解消する障害は、この比較では見つからなかった。
converter が使用する TS6 は当面独立して維持する。

## 実験方法

```powershell
cd F:\Projects\shorth\experiments\sfc-projection-research
vp run compare-checkers
```

`checker-comparison.mjs` が fixture と仮想ファイルをメモリ内に生成し、
`results/checker-comparison.json` に測定結果を保存する。結果を保存した後、
候補・原文対応・診断・増分更新を assertion で検証する。
依存は既存の実験 workspace、`packages/host`、`packages/integration`、
`packages/projection` のインストールを使用する。追加インストールは行っていない。

- 本番の `projectFrameworkSource` で Vue / Svelte / Astro を投影する。
- 本物の `stillwind/vue` / `stillwind/svelte` / `stillwind/astro` の型を使う。
- 投影 AST の宣言 call に identity の隠し引数を注入する。
- identity は元 SFC の UTF-8 宣言位置と binding 名から作る。
- `styleFlow` は使用しない。checker が返す型の identity を読む。
- TS6 は `createProgram`、TS7 は `unstable/sync` の `API` を使う。
- checker に渡す投影・型環境・設定は両バージョンで同じにする。
- entry だけを root とし、barrel、generic helper、別 SFC の producer を
  相対 import で解決する。SFC import は仮想 `.ts` / `.tsx` へ書き換える。
- TS7 の filesystem callback が全推移的モジュールを読んだことを検証する。
- Vue の補助型、Svelte の公式 shim / JSX 宣言、Astro の `env.d.ts` を使用する。
- probe の AST 範囲を shadow 編集前へ戻し、converter map を介して原文の
  UTF-16 範囲に対応させ、UTF-8 範囲も記録する。日本語・絵文字・CRLF を含む。

独立した 12 シナリオを両 checker で比較し、さらに Astro client script の
未加工／分離後を両 checker で比較する。TS7 では各 framework について
初回 → producer identity 更新 → 依存追加 → 依存削除の snapshot 列も検証する。

## 結果

| 対象                                                          | TS6.0.3                         | TS7.0.2 |
| ------------------------------------------------------------- | ------------------------------- | ------- |
| Vue script / template / props / v-for                         | 候補・原文範囲一致              | 同じ    |
| Svelte script / template / $props / $state / store / each     | 候補・原文範囲一致              | 同じ    |
| Astro frontmatter / template / Astro.props / map callback     | 候補・原文範囲一致              | 同じ    |
| Vue normal script + setup、Svelte module + instance の参照    | 候補取得成功                    | 同じ    |
| SFC → TS barrel → SFC producer、TS generic helper             | 候補取得成功                    | 同じ    |
| 明示的 union の可変変数、generic、object property、関数戻り値 | 有限候補取得成功                | 同じ    |
| ref、keyframes、view transition                               | 宣言 identity 取得成功          | 同じ    |
| identity によって不正になった代入                             | TS2322、原文位置に対応          | 同じ    |
| any、広い StyleResult、identity を消す仮引数                  | 有限候補へ変換不可、TS 診断なし | 同じ    |
| Svelte DOM props と文字列 identity の共存                     | 診断なし                        | 同じ    |
| Astro client script 内の import を含む未加工投影              | TS1232 等                       | 同じ    |

文字列 identity property を使用する現在の正常系では、fixture の
syntactic / semantic diagnostics はゼロ。候補・map の失敗はゼロ。
不正代入では候補が A のみに見えるが、両 checker が TS2322 を返す。
診断のある結果を採用しないことがサイレントな欠落を防ぐ条件になる。
TS7 の診断位置は `pos/end`、TS6 は `start/length` で取得する。

identity 消失は TypeScript の診断だけでは拒否できない。
今回の probe は有限候補へ変換できず `null` になることを検証している。
本番では、その結果を必要な合成元に対する明示的エラーとして拒否する。
任意の型アサーションによる虚偽、診断抑制 directive の安全性は、この結果が
保証する範囲に含めない。

## バージョン共通の実装課題

### Svelte の identity property

初回実験では `[styleIdentity]: string` が Svelte の attachment 用 symbol index
signature と衝突し、TS6 / TS7 とも TS2345 になった。template 出力境界だけで
identity を除去する helper により解決できることも確認した。

現在の本番型は `data-__stillwindStyleIdentity` という文字列 property を使う。
本物の更新済み型を使った再実行では、helper なし／あり双方で正常系の診断がゼロ。
比較 fixture の helper は旧回避案との比較用に残すが、本番には不要。

### Astro の client script

現在の `convertToTSX(..., {includeScripts:true})` は client script の内容を
JSX 内の関数に置く。そこに import があると、両 checker で TS1232 が出る。

実験では、script 内容を改行と長さを維持して空白化した SFC の投影と、
client script の独立した仮想 `.ts` を同じ project に渡す。client 側の
identity 候補が取れ、元 `.astro` の範囲に戻り、診断がゼロになることを確認した。
原文を分離・保存する必要はない。

この fixture は一つの埋め込み module script を対象とする。production では
converter の script metadata から region を取得し、複数 script、外部 src、
inline / module の scope と解決規則を維持する必要がある。

## TS7 の更新処理

三者すべてで以下が成功した。

1. entry から推移的に取得した候補は `remote / remote2`。
2. producer の shadow identity を変更して `fileChanges.changed` を送ると
   `remote / remote3` に更新。古い候補は残らない。
3. helper の依存先を新しい仮想 TS モジュールへ変え、`created` を通知しても
   同じ候補を維持し、診断はゼロ。
4. その依存を削除して `deleted` を通知すると TS2307。欠落を成功扱いしない。

この更新試験は checker の invalidation を対象とする。原文更新後の converter
再実行、Rust session、Vite HMR までを通した結合試験ではない。

## 判断と適用範囲

TS7 に必要な型・AST・診断・仮想 filesystem・増分更新 API は今回の経路で
動作した。TS6 一本化には従来 API と依存を揃えられる利点はあるが、本件では
checker の移植作業に見合う互換性上の改善を確認できなかった。
**TS7.0.2 を維持し、converter の TS6 を隔離する構成を推奨する。**

比較スクリプト自体は checker の実験であり、Rust の最終出力までを検証しない。
本番実装と結合テストは [SFC 型情報](../../docs/sfc-type-checking.md) を参照。

仮想ファイルは host が需要のある import をたどって非同期 converter で準備し、
checker が filesystem callback で読む方式。同期 `readFile` 内で非同期 converter
を実行できることや、TS7 content mapper API の利用を証明したものではない。
map はこの fixture 群の対応を検証する研究用実装であり、未知の投影や曖昧な
mapping に対応する production 用の範囲検証器ではない。

所要時間も JSON に記録するが、固定順序の少数・同一プロセス測定であり、
公平な速度比較ではない。性能によるバージョン判断には使用しない。
