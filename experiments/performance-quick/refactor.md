# モジュール更新と stylesheet 描画の分離

2026-09-11、[軽量調査](./README.md)で確認したボトルネックに対して 3 段階で変更した。新しいキャッシュ層、dirty flag、互換モードは追加していない。

## 実装

1. `18ca378`: `ProjectSession::update_module` は `ModuleUpdate`（変換変更の有無と診断）を返し、全体 CSS を描画しない。全体 CSS の変更判定は `SessionReload` に属する。削除は producer の存在を返し、共有 CSS の変化と区別する。
2. `62009b5`: `DependencyIndex` に artifact の依存の差し替え・削除を集約した。既存 artifact の依存を使うため、別の逆引き状態は持たない。watch 失敗では公開済み artifact と index を保持し、build 失敗では両方から除く。
3. `CompiledModule::to_module_output` は fragment の出力変換、`StylesheetRenderer::render_compilation` は standalone 出力の組み立てを担当する。内部 pipeline に残っていた document 描画も除去した。CLI と standalone compiler は renderer を通り、N-API の project/session は fragment をそのまま返す。

欠けた utility slot や authored CSS の未解決 theme 参照は描画 API で検出する。standalone API の `SHCSS001` とソース範囲、失敗時に生成コードを返さない契約は維持した。既に描画不能な状態でも、修正した CSS への reload は成功できる。失敗した reload は現行の正常出力を保持する。

## 段階ごとの性能

同じ Windows / release binary / synthetic module graph で測定。各ケース warmup 後 9 回、fresh graph は 5 回の中央値。最終段階は別プロセスでも再実行した。全段階の 6 種類の生成 JS/CSS fingerprint が変更前と一致した。

| ケース                               |      変更前 | 1: 更新と描画を分離 | 2: index 差し替え | 3: fragment と document を分離 |
| ------------------------------------ | ----------: | ------------------: | ----------------: | -----------------------------: |
| 100 modules 初回                     |    25.49 ms |            17.45 ms |          15.12 ms |                 11.41–11.61 ms |
| 300 modules 初回                     |   151.30 ms |            67.39 ms |          43.88 ms |                 38.82–46.13 ms |
| 1,000 modules 初回                   | 1,600.90 ms |           590.17 ms |         169.50 ms |               157.52–161.34 ms |
| 1,000 resident modules 中 1 件を編集 |    4.663 ms |            1.024 ms |          0.305 ms |                 0.173–0.208 ms |
| 1,000 modules 全件変更なし           |    88.77 ms |           106.96 ms |         102.18 ms |                 84.69–85.42 ms |

このデータでは初回が約 90%、単一編集が約 96% 短縮した。3 単独の大規模ケースでの差は小さいが、責務の重複を削除できたため採用した。変更なしの全件再実行はほぼ同じで、scan/finalize・artifact key・返却値生成は引き続き必要になる。

Host の最終 2 プロセスでは literal の変更なし 0.047–0.048 ms、semantic の変更なし 0.839–1.149 ms、semantic 編集 1.601–2.065 ms、Vue/Svelte/Astro の projection はおおむね 0.8–1.2 ms。checker の新規起動を含むケースは 60.6–65.3 ms で、初期調査の 46.5–61.9 ms に比べてばらつきがある。ここにはコンパイラー変更の効果を帰属させない。

CSS query の単体時間や小さなケースにはプロセス間のばらつきがあり、軽微な増減を改善率として扱わない。実アプリ全体のビルド・HMR・メモリリークの測定ではない。

## 検証

- Rust workspace テスト成功。最後の reload 復旧調整後にも ProjectSession と native 公開 API のテストを再実行した。
- Native 公開 API 11 件・生成 TypeScript 宣言の型検査。
- Host 28 件、Vite 15 件、webpack/Rspack 10 件、公式 Vue/Svelte/Astro build 3 件。
- 6 review fixtures の生成結果一致、11 crates の責務境界チェック。
- shared rule の保持と削除、依存の差し替えを fresh session と比較、watch/build の失敗時 index、描画エラーと修復を検証。
- format、lint、workspace Clippy。

生データ: `results/before-refactor.json`、`step1.json`、`step2.json`、`step3.json`、`step3-repeat.json`。Host は `host-step1.json` と `host-step3*.json`。測定時点は各実装のコミット前なので、JSON 内の Git revision は直前のコミットを指す。各 JSON の binary hash と本書の段階対応を併用する。

再実行:

```powershell
vp run perf:quick -- --label after-local
vp run perf:host -- --label host-after-local
```

baseline との出力一致検査:

```powershell
$before = Get-Content experiments/performance-quick/results/before-refactor.json -Raw | ConvertFrom-Json -AsHashtable
$after = Get-Content experiments/performance-quick/results/after-local.json -Raw | ConvertFrom-Json -AsHashtable
foreach ($key in $before.fingerprints.Keys) {
  if ($before.fingerprints[$key] -ne $after.fingerprints[$key]) { throw "Output changed: $key" }
}
```

現時点で残る全体走査には、生成 symbol の衝突検査と semantic support の探索がある。今回の 1～3 には追加の状態管理を導入しておらず、これらの最適化は再計測に基づく別の判断とする。
