# 軽量パフォーマンス調査 — 2026-09-11

最も大きい改善候補は `ProjectSession::update_module` の全体走査。1,000 モジュールの初回処理を計装したところ、更新前後の全体 CSS 生成が約 66%、依存インデックス再構築が約 19% を占めた。N-API の型付きオブジェクト化より、こちらの規模依存コストを先に改善する価値がある。

本体の最適化は行っていない。追加したのは計測スクリプト、Vite+ タスク、測定結果と、一時 checkout にだけ適用した計装差分。

## 測定条件

- Windows x64 / AMD Ryzen 7 9800X3D / 16 logical CPUs。
- Node v24.21.0、rustc 1.98.0、release native binary。
- 現行: `3b4a8d2`。比較: JSON transport が残る `9f4e668`。
- 比較には napi / napi-derive / napi-build の patch version 更新も含まれる。JSON transport だけの因果効果を分離した実験ではない。
- 現行・旧版を交互に別プロセスで実行。最終集計は同一バイナリで揃えた runs 5/6/7 の各中央値の中央値。各ケースは warmup 2 回＋9 回、fresh graph は warmup 1 回＋5 回。
- 旧版の数値には呼び出し側の `JSON.stringify` / `JSON.parse` を含む。現行はオブジェクトを直接渡す。
- Node 起動・CSS ファイルのディスク読み込み・ビルド時間は native 計測に含めない。CSS loader の解析・定義構築は含める。
- graph は 10/100/300/1,000 モジュール、各 8 個の `style('flex red')` 呼び出し。単一ファイルの返却負荷は 1/100/1,000 呼び出し。依存辺のない合成データで、実アプリ全体のビルド／HMR／バンドラーキャッシュの数値ではない。
- 6 種類の生成 JS/CSS fingerprint と入力 CSS hash が新旧・全採用 run 間で一致することを集計時に assert。実行中も診断ゼロ、モジュール数、変換出力を確認。
- 普段のデスクトップ環境での短時間測定。プロセス中央値の幅は信頼区間ではなく、微小な差や GC の影響を速度改善として断定しない。RSS は終端 snapshot のみで、リーク検査ではない。

全ケース・各反復の生値は [results](./results/)、新旧比較は [summary.json](./results/summary.json)。最初の探索・ビルドと重なった測定は集計対象から外した。

## 規模を増やした結果

現行の中央値。括弧は 3 プロセスの中央値の最小～最大。

| モジュール数 |                  初回コンパイル | 全件変更なし再実行 | 1 モジュール編集 |
| ------------ | ------------------------------: | -----------------: | ---------------: |
| 10           |                         1.35 ms |            0.55 ms |          0.14 ms |
| 100          |          25.44 ms (24.15–26.04) |            6.16 ms |          0.37 ms |
| 300          |       168.88 ms (155.77–177.55) |           23.40 ms |          0.93 ms |
| 1,000        | 1,743.92 ms (1,612.24–1,910.13) |           91.63 ms |          2.95 ms |

100→1,000 モジュールで初回時間は約 69 倍。変更なし再実行では artifact の再コンパイルを省略するが、解析や返却値生成は残る。単一編集も resident モジュール数に応じて伸びる。

257,888 bytes の全組み込み CSS を読み込む loader は **31.09 ms**、その定義を毎回構築する stateless compile は **35.81 ms**。minimal CSS の stateless compile は 0.10 ms。既存 host の loader/session 再利用を維持し、hot path に stateless 呼び出しを入れないことが重要。

## N-API 変更前との比較

値は旧版→現行。位置情報は native の structured mapping entries までで、host での v3 map/VLQ 生成や bundler への合成は含まない。

| ケース                                    | 旧 JSON API | 現行 object API | 現行 / 旧版 |
| ----------------------------------------- | ----------: | --------------: | ----------: |
| 1 ファイル・1,000 calls、変更なし compile |    6.823 ms |        6.782 ms |        0.99 |
| 100 modules、CSS＋位置情報返却            |    1.728 ms |        1.319 ms |        0.76 |
| 300 modules、CSS＋位置情報返却            |    5.582 ms |        4.387 ms |        0.79 |
| 1,000 modules、CSS＋位置情報返却          |   24.511 ms |       18.539 ms |        0.76 |
| 全組み込み CSS、base＋位置情報返却        |    2.972 ms |        2.420 ms |        0.81 |
| 300 modules、全件変更なし compile         |   19.061 ms |       23.402 ms |        1.23 |
| 1,000 modules、全件変更なし compile       |  100.425 ms |       91.625 ms |        0.91 |

CSS と位置情報の返却は、この測定では約 19–24% 短縮。コンパイル全体が一様に速くなる変更ではない。300 modules の変更なし compile は約 23% 遅く、現行のプロセス中央値 23.13–26.27 ms に対して旧版は 17.07–20.06 ms だったので、GC／返却オブジェクト生成を含めた追加調査候補。ただし 1,000 modules では方向が逆で、単一の回帰率として一般化しない。

## 初回コンパイルの内訳

[instrumentation.patch](./instrumentation.patch) を一時 checkout に適用し、`update_module` 内を 5 区間に分けて計測した。各サイズ 1 プロセス・1 fresh graph の補助測定であり、上の反復ベンチマークとは別。結果は [profile.json](./results/profile.json) と各 `profile-*.log`。

| 1,000 モジュールでの区間                      |       時間 | 呼び出し全体に占める割合 |
| --------------------------------------------- | ---------: | -----------------------: |
| 更新前の全体 CSS 生成                         |   557.8 ms |                    32.7% |
| 更新後の全体 CSS 生成・比較                   |   563.6 ms |                    33.0% |
| 依存インデックス再構築                        |   320.5 ms |                    18.8% |
| `backend.compile`                             |    85.0 ms |                     5.0% |
| symbol 検査・artifact 格納                    |    34.9 ms |                     2.0% |
| 上記以外（scan/finalize/output/FFI/計装など） |   146.4 ms |                     8.6% |
| 呼び出し全体                                  | 1,708.4 ms |                     100% |

`old_css = self.css()` と更新後の `self.css()` が各更新で蓄積済みモジュールを走査し、`reindex()` も全 artifact を走査する。初回に N モジュールを登録すると走査量が累積する構造で、測定した非線形な増え方と整合する。N-API は同じ呼び出し内でモジュールごとに `update_module` を呼んでいる。

優先候補は、更新をまとめた際の CSS 比較の集約と依存インデックスの差分更新。CSS の順序・共有 rule の参照数・watch 時の失敗復旧契約を保った設計が必要。上記割合は現状の内訳であり、そのまま実現可能な短縮率ではない。

参照: [ProjectSession](../../crates/project/src/session.rs)、[N-API session](../../crates/napi/src/session.rs)。

## Host・型解析

同じ minimal CSS の小さな例で `compileModule` を測定。各 converter の初回 import は事前実行で除き、SFC の projection と native compile は含めた。SFC から実行用 JS への後段フレームワークコンパイルは含めない。2 プロセスの中央値の範囲:

| 経路                           |       変更なし |     ソース編集 |
| ------------------------------ | -------------: | -------------: |
| React literal                  | 0.058–0.060 ms | 0.084–0.111 ms |
| React semantic、checker 再利用 | 0.835–2.099 ms | 1.543–2.758 ms |
| Vue projection                 | 1.110–1.241 ms | 0.760–0.814 ms |
| Svelte projection              | 0.992–1.116 ms | 0.824–0.875 ms |
| Astro projection               | 1.015–1.092 ms | 0.819–1.209 ms |

semantic 例で checker と session を毎回作り直す場合は **46.5–61.9 ms**。既存の lazy creation／再利用を保つ価値がある。大規模な依存グラフや型の複雑さは今回の範囲外。

## 二次候補

`CompiledModule::to_output` は完全な stylesheet を描画する一方、N-API session はその後 `result.css` を `result.module_css` で置き換えている。完全 CSS の構築が出力に使われない経路があるため、次回の計測候補。ただし描画はエラー検出も兼ねるので、単純削除できるとまでは検証していない。

`scan` の後の `finalize` が返却対象では 2 回走ること、`attach_asset_support` が各入力を走査すること、変更なしでも scan・key 計算・出力変換が走ることも確認した。今回、これらの個別所要時間は分離していない。

参照: [to_output](../../crates/compiler/src/module_compiler.rs)、[asset support](../../crates/ir/src/analysis.rs)。

## 再実行

リポジトリ root の PowerShell から:

```powershell
vp run perf:quick -- --label current-local
vp run perf:host -- --label host-local
vp run perf:summarize
```

最初の 2 タスクは native release build に依存し、計測自体はキャッシュしない。`perf:summarize` のデフォルトは保存済み runs 5/6/7 の集計・出力一致検査。`--label` は結果ファイル名になる。

旧版も測り直す場合:

```powershell
$perfRepo = (Get-Location).Path
$perfOld = Join-Path $env:TEMP 'stillwind-perf-baseline-rerun'
git worktree add --detach $perfOld 9f4e668
$env:CARGO_INCREMENTAL = '0'
cargo build --manifest-path "$perfOld/crates/napi/Cargo.toml" --release --target x86_64-pc-windows-msvc --target-dir "$perfRepo/target" --locked
Copy-Item -LiteralPath "$perfRepo/target/x86_64-pc-windows-msvc/release/stillwind_napi.dll" -Destination "$perfOld/baseline.node"
vp run perf:quick -- --legacy "$perfOld/baseline.node" --label legacy-local
```

この例は Windows x64 用。比較用に同じ `target` を使用するため native build は並列に走らせない。macOS/Linux では target と共有ライブラリ名を変更する。

区間計装の再実行は、同じ一時 checkout を `3b4a8d2` に切り替え、`git apply "$perfRepo/experiments/performance-quick/instrumentation.patch"` をその checkout で行う。同じ cargo build 後に DLL を `instrumented.node` としてコピーし、root で `vp run perf:profile -- --binding "$perfOld/instrumented.node"` を実行する。計装差分は production checkout に適用しない。
