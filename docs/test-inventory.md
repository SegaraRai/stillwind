# テストとチェックの棚卸し

リポジトリ全体（Rust crates、packages、apps、tools、experiments）のテストと検査を、
再配置・削除・不足の判断に使える形で列挙する。

- 基準: HEAD `0668496d`（2026-09-21）
- Rust の件数: `cargo test --workspace -- --list` の列挙結果 666 件（実行はしていない）
- JS/TS の件数: `test(` 呼び出しサイトの静的カウント 137 件（ループで複数回実行されるものは 1 と数える）
- 実行結果の記録は [Rustユニットテストの対象棚卸し](rust-unit-test-audit.md) を参照

## 1. 実行経路

### 1.1 自動化されている検査

| 経路                     | 実行内容                                                                                                                                                  |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `.vite-hooks/pre-commit` | ステージ済みファイルへの `vp fmt` と `vp lint`、続いて `cargo fmt --check` と `cargo clippy --workspace --all-targets -- -D warnings`。テストは実行しない |
| CI                       | なし。`.github` も他の CI 設定も存在しない                                                                                                                |
| ルートタスク             | `vp run <task>` を手動で実行する。テストを集約するタスクは無い                                                                                            |

### 1.2 パッケージのタスク

| パッケージ                 | テスト系タスク                                                                  | コマンド                                                                                         |
| -------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| stillwind（packages/core） | `check`                                                                         | `node --test test/*.test.mts`（`typecheck` と `build` に依存）                                   |
| @stillwind/host            | `test`                                                                          | `node --test test/*.test.mts`（`build` に依存）                                                  |
| @stillwind/integration     | `test`、`test:vite`、`test:webpack`、`test:frameworks`                          | `node --test ...`（`cache: false`）                                                              |
| @stillwind/native          | `check`、`check:wasm`                                                           | `node --test test/loader.test.mjs` / `test/wasm.test.mjs`（`check:wasm` は `build:wasm` に依存） |
| @stillwind/play            | `test`                                                                          | `node --test test/*.test.mts`                                                                    |
| @stillwind/projection      | なし                                                                            | テストが存在しない                                                                               |
| @stillwind/tools           | `check`（crate 境界）、`tailwind:property-order:test`、`tailwind:contract:test` | `node crate-boundaries.mts` / `node --test ...`                                                  |
| @stillwind/fixtures-viewer | `check`                                                                         | `tsc --noEmit` のみ                                                                              |
| @stillwind/react-example   | `check`                                                                         | `tsc --noEmit` のみ                                                                              |

現状の注意点:

- テスト系タスク名が `check`（core、native）と `test`（host、integration、play）に分かれている。`vp run -r test` は core と native のテストを実行しない。[crate アーキテクチャ](crate-architecture.md) の検証コマンドは `vp run -r test` を挙げており、この 2 パッケージは抜ける
- `@stillwind/integration#test` のグロブは `*.test.mts` を列挙するため、`test/vite/framework-builds.mts` は `test:frameworks` でのみ実行される
- `cargo test --workspace` はどの自動経路にも入っていない（pre-commit にも CI にも無い）
- WASM の検証（`check:wasm`）は `build:wasm`（`wasm32-wasip1-threads` ターゲット）を前提とする手動タスク

## 2. Rust テスト

### 2.1 構成

- 実装ファイル内の `#[cfg(test)]` モジュールが基本。`src/**/tests.rs` 形式の分離も使う
- クレート境界のテストは `crates/*/tests/*.rs`（公開 API、プロセス E2E、バインディング形状）
- 共有フィクスチャは `#[path]` で取り込む。`crates/test-support/css.rs`、`crates/test-support/defaults.rs`、`crates/css/test-support/assertions.rs`。`crates/test-support` はワークスペースメンバーではなく、`Cargo.toml` を持たない
- プロパティベーステストは 11 ファイルで `proptest` を使用
- `#[ignore]` は 1 件（`builtin registry` の到達性を確認する手動監査）

### 2.2 実測件数

`cargo test --workspace -- --list` の結果は 49 テストターゲット、合計 666 件（benchmark 0）。

| クレート                | lib / main | 統合（tests） | 合計                 |
| ----------------------- | ---------- | ------------- | -------------------- |
| stillwind-compiler      | 92         | 187           | 279                  |
| stillwind-design-system | 163        | 15            | 178                  |
| stillwind-css           | 49         | 0             | 49                   |
| stillwind-js-oxc        | 3          | 45            | 48                   |
| stillwind-project       | 24         | 22            | 46                   |
| stillwind-napi          | 3          | 13            | 16                   |
| stillwind-source        | 9          | 6             | 15                   |
| stillwind-syntax        | 14         | 0             | 14                   |
| stillwind-diagnostics   | 3          | 6             | 9                    |
| stillwind-semantic      | 6          | 0             | 6（ほか doc test 1） |
| stillwind-cli           | 0          | 4             | 4                    |
| stillwind-ir            | 1          | 0             | 1                    |
| 合計                    | 367        | 298           | 666                  |

### 2.3 統合テストの内訳

| ファイル                                             | 件数 | 対象                                             |
| ---------------------------------------------------- | ---- | ------------------------------------------------ |
| compiler `static_compile/`（14 モジュール）          | 120  | 実ソースからのコンパイル受理スイート             |
| compiler `audit_regressions.rs`                      | 19   | コメント監査で見つかった実装不具合の回帰         |
| compiler `module_compiler.rs`                        | 15   | モジュールコンパイラの artifact、lint 方針、境界 |
| compiler `comment_contracts.rs`                      | 10   | ドキュメント監査で確認した公開 API 境界          |
| compiler `design_system_rendering.rs`                | 6    | スタイルシート描画                               |
| compiler `static_structure.rs`                       | 4    | スプレッドと固定キーの構造保持                   |
| compiler `animation_branches.rs`                     | 3    | アニメーション分岐                               |
| compiler `query.rs`                                  | 3    | エディタクエリ                                   |
| compiler `imports.rs`                                | 2    | import 修飾子とエラー                            |
| compiler `macro_contract.rs`                         | 2    | マクロ契約の連携                                 |
| compiler `utility_order.rs`                          | 2    | 上流 Tailwind のプロパティ順                     |
| compiler `stylesheet_map.rs`                         | 1    | ソースマップの物理ソース対応                     |
| compiler `tests/static_compile.proptest-regressions` | —    | proptest の失敗シード記録                        |
| js-oxc `parse.rs`                                    | 22   | パースと抽出                                     |
| js-oxc `projected_analysis.rs`                       | 10   | 投影解析と span 復元                             |
| js-oxc `macro_contract.rs`                           | 5    | マクロ実行境界                                   |
| js-oxc `ref_variants.rs`                             | 4    | ref variant の identity                          |
| js-oxc `analysis.rs`                                 | 3    | 解析入口の契約                                   |
| js-oxc `module_css_imports.rs`                       | 1    | 明示 module CSS の import 追跡                   |
| project `session.rs`                                 | 15   | セッション、公開時の再採番、名前プール           |
| project `composition_lint.rs`                        | 7    | 合成 lint                                        |
| design-system `loading.rs`                           | 15   | ローダー公開プロトコル                           |
| napi `binding.rs`                                    | 12   | Node へ渡す値の形状                              |
| napi `comment_contracts.rs`                          | 1    | UTF-16 座標                                      |
| diagnostics `contracts.rs`                           | 6    | コード、メッセージ、原因の対応                   |
| source `projection.rs`                               | 6    | 射影マップ                                       |
| cli `e2e.rs`                                         | 4    | プロセス起動                                     |

`crates/compiler/tests/fragment-execution.mjs` は `compiler/src/compiler/fragment/tests.rs` から子プロセスで起動される
Node ハーネスで、どのパッケージにも属さない JS 実行資産としてクレート外に置かれている。

### 2.4 コロケーションの良い例

- `crates/design-system/src/evaluation/tests/`（10 ファイル）: 実装モジュール内に主題別スイートを置き、`support.rs` にフィクスチャを集約している
- `crates/design-system/src/tests/theme.rs`: パッケージ CSS を読み込む theme 統合。`src/theme/*` の inline テストとは対象が分かれる
- `crates/compiler/src/compiler/{tests.rs,fragment/tests.rs,stylesheet/template/tests.rs}`、`crates/compiler/src/identifiers/tests.rs`: 実装内の分離テスト

## 3. JS/TS テスト

### 3.1 ランナーと前提

すべて `node:test` を `node --test` で実行する。import 先は `../src/*.ts`（Node の型ストリッピングで直接実行）と `../dist/*.js`（ビルド成果物）が混在する。

tsconfig の `typecheck` 対象:

| パッケージ           | test の型検査                          |
| -------------------- | -------------------------------------- |
| packages/core        | 対象（`test/**/*.mts` を include）     |
| packages/native      | 対象（`test/*.mts` を include）        |
| packages/integration | 対象（`test/**` を include）           |
| tools                | 対象                                   |
| apps/play            | 対象（`test` を include）              |
| packages/host        | 対象外（`src/**/*.ts` のみを include） |

### 3.2 ファイル別一覧

件数はテスト宣言の数。ループで複数回実行されるものは括弧内に実行回数を示す。

| ファイル                                                     | 件数                                   | import 先                                    |
| ------------------------------------------------------------ | -------------------------------------- | -------------------------------------------- |
| packages/core/test/runtime.test.mts                          | 4                                      | dist（react runtime）                        |
| packages/core/test/fragment.test.mts                         | 10                                     | dist（react/svelte/astro）と src（fragment） |
| packages/core/test/css.test.mts                              | 2                                      | dist（css entry）                            |
| packages/core/test/css-entrypoints.test.mts                  | 3                                      | パッケージファイル（fs 直接）                |
| packages/core/test/macro.test.mts                            | 1（6 フレームワーク）                  | dist（frameworks）                           |
| packages/core/test/framework-types.mts                       | 型のみ                                 | dist の型                                    |
| packages/host/test/conditional-flow.test.mts                 | 22                                     | src                                          |
| packages/host/test/composition.test.mts                      | 11                                     | src                                          |
| packages/host/test/sfc-types.test.mts                        | 8（SFC 3 種）                          | src                                          |
| packages/host/test/editor.test.mts                           | 6                                      | src                                          |
| packages/host/test/css-source-map.test.mts                   | 4                                      | src                                          |
| packages/host/test/frameworks.test.mts                       | 3（SFC 3 種 × 事例）                   | src                                          |
| packages/host/test/resolution.test.mts                       | 3                                      | src                                          |
| packages/host/test/naming.test.mts                           | 2                                      | src                                          |
| packages/host/test/macro-contract.test.mts                   | 1                                      | src                                          |
| packages/host/test/ref-variants.test.mts                     | 1                                      | src                                          |
| packages/host/test/rejected-names.test.mts                   | 1                                      | src                                          |
| packages/host/test/type-probes.test.mts                      | 1                                      | src                                          |
| packages/integration/test/entrypoints.test.mts               | 6                                      | dist（配布物検査）                           |
| packages/integration/test/parity.test.mts                    | 1（命名 2 通り）                       | dist                                         |
| packages/integration/test/vite/module-css.test.mts           | 6                                      | dist                                         |
| packages/integration/test/vite/session.test.mts              | 4                                      | dist                                         |
| packages/integration/test/vite/custom-properties.test.mts    | 2                                      | dist                                         |
| packages/integration/test/vite/naming.test.mts               | 2                                      | dist                                         |
| packages/integration/test/vite/source-map.test.mts           | 2                                      | dist                                         |
| packages/integration/test/vite/type-facts.test.mts           | 2                                      | dist                                         |
| packages/integration/test/vite/config-root.test.mts          | 1（2 プロジェクト）                    | examples/play の設定                         |
| packages/integration/test/vite/development-boundary.test.mts | 1                                      | dist                                         |
| packages/integration/test/vite/emission.test.mts             | 1                                      | src（vite/emission）                         |
| packages/integration/test/vite/frameworks.test.mts           | 1                                      | dist                                         |
| packages/integration/test/vite/framework-builds.mts          | 1（vue/svelte/astro）                  | dist（`test:frameworks` 専用）               |
| packages/integration/test/webpack/builds.test.mts            | 1（5 フレームワーク × 2 バックエンド） | dist                                         |
| packages/integration/test/webpack/emission.test.mts          | 1（2 バックエンド）                    | dist                                         |
| packages/native/test/loader.test.mjs                         | 12                                     | dist（公開エントリ）                         |
| packages/native/test/wasm.test.mjs                           | 0（loader を再帰 import）              | dist（WASM）                                 |
| packages/native/test/types.mts                               | 型のみ                                 | dist の型                                    |
| apps/play/test/playground.test.mts                           | 1                                      | src（server plugin）                         |
| tools/property-order-parity.test.mts                         | 4                                      | lib                                          |
| tools/tailwind-parity-contract.test.mts                      | 6                                      | lib                                          |
| packages/integration/test/vite/framework-dev.mts             | テストではない                         | HMR 用の手動ハーネス                         |

### 3.3 型のみの検査

`packages/core/test/framework-types.mts` と `packages/native/test/types.mts` は実行時テストではなく、`@ts-expect-error` を含む型契約を `tsc` で検証する。どちらも `node --test` のグロブには含まれない。

## 4. テスト以外のチェック

### 4.1 ルートタスク

| タスク                                     | 内容                                                  |
| ------------------------------------------ | ----------------------------------------------------- |
| `vp run check`                             | `vp check`（整形、lint、型認識 lint）                 |
| `vp run lint` / `vp run fmt`               | oxlint / oxfmt                                        |
| `vp run crates:check`                      | `node tools/crate-boundaries.mts`（クレート責務境界） |
| `vp run webref:check`                      | 生成済み Webref 資産と `@webref/css` の一致           |
| `vp run fixtures:check`                    | `fixtures/projects` の expected と現行出力の一致      |
| `vp run tailwind:parity` / `:all`          | Markdown フィクスチャと上流 Tailwind 出力の比較       |
| `vp run tailwind:property-order` / `:test` | 宣言順の等価判定とその単体テスト                      |
| `vp run tailwind:parity:analyze`           | 差分の分類レポート                                    |
| `vp run tailwind:numeric-domain-audit`     | 数値ドメインの監査レポート                            |
| `vp run tailwind:registry-audit`           | `--ignored` の手動監査（builtin 到達性）              |
| `vp run sourcemap:*`                       | ソースマップ研究のビルド、研究実行、受理テスト        |
| `vp run perf:*`                            | 性能測定（`--release` ビルド、プロファイル、集計）    |

### 4.2 tools のスクリプト

| スクリプト                             | 役割                                                                       | 実行タスク                                                               |
| -------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| `crate-boundaries.mts`                 | クレート依存の許可表と Oxc 所有権を検証                                    | `@stillwind/tools#check`、`crates:check`                                 |
| `generate-webref-css-domains.mts`      | Webref スナップショットの生成と `--check`                                  | `webref:generate` / `webref:check`                                       |
| `review-fixtures.mts`                  | `fixtures/projects` の expected 生成と `--check`                           | `review:refresh` / `review:check`、`fixtures:refresh` / `fixtures:check` |
| `regenerate-tailwind-v4-fixtures.ts`   | Tailwind v4 Markdown フィクスチャの再生成と `--check`                      | `fixtures:write` / `fixtures:check`                                      |
| `generate-utility-order-fixture.mts`   | 上流 Tailwind からユーティリティ順フィクスチャを生成                       | `tailwind:utility-order:generate` / `:fixture`                           |
| `verify-tailwind-v4-value-grammar.ts`  | value grammar カタログの受理・拒否を検証                                   | `tailwind:value-grammar`                                                 |
| `tailwind-parity.mts`                  | フィクスチャと上流出力の比較（オラクル）                                   | `tailwind:parity` / `:all` / `:analyze`                                  |
| `tailwind-property-order.mts`          | プロパティ順の制約検証                                                     | `tailwind:property-order`                                                |
| `tailwind-parity-contract.test.mts`    | 比較契約の単体テスト                                                       | `tailwind:contract:test`                                                 |
| `property-order-parity.test.mts`       | 順序等価判定の単体テスト                                                   | `tailwind:property-order:test`                                           |
| `tailwind-browser-contract.mts`        | 実ブラウザでのカスケード確認（`STILLWIND_BROWSER` 必須）                   | `tailwind:browser`                                                       |
| `tailwind-numeric-domain-audit.mts`    | 数値ドメインの監査                                                         | `tailwind:numeric-domain-audit`                                          |
| `tailwind-inventory.mts`               | カバレッジ棚卸しレポート                                                   | `tailwind:inventory`                                                     |
| `extract-tailwind-v4-registrations.ts` | 上流登録の抽出                                                             | `tailwind:extract`                                                       |
| `review-rust-comment-probes.py`        | 過去のコメント監査の再現（CLI ビルド前提、`--check` で修正後の挙動を要求） | 手動                                                                     |

`tailwind:value-grammar`、`tailwind:utility-order*`、`tailwind:inventory`、`tailwind:browser`、`tailwind:contract:test`、`tailwind:extract` はルートに対応タスクがなく、`vp run --filter @stillwind/tools <task>` でしか実行できない。

### 4.3 experiments

| 場所                                  | 内容                                                                                                                                                                                                                                                |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `experiments/module-graph`            | 独自 pnpm ワークスペース（ルートから除外）。`verify:*` タスク群（emission、conditional、composition、animation、view-transition、adapters、rspack、react-router、solidstart、tanstack、source-map）と `verify-source-map.test.mjs`（`node --test`） |
| `experiments/sfc-projection-research` | `verify.mjs`、`verify-normalization.mjs`、`verify-native.mjs`、`checker-comparison.mjs`（vitest の override だけを持つ独立ワークスペース）                                                                                                          |
| `experiments/performance-quick`       | `perf:*` タスクの実体（run、host、profile、summarize）                                                                                                                                                                                              |
| `experiments/source-map-research`     | Rust 製リサーチバイナリと `run.mjs`、`acceptance.mjs`                                                                                                                                                                                               |
| `experiments/ts7-shadow-checker`      | `ts7-shadow:poc` タスクの PoC                                                                                                                                                                                                                       |

## 5. 再配置（コロケーション）

### 5.1 判定基準

`crates/*/tests/**` の全 298 ケースを個別に確認し、次の基準で分類した。
本節は分類までで、移設は実施していない。

| 判定       | 意味                                                                                   | 扱い                            |
| ---------- | -------------------------------------------------------------------------------------- | ------------------------------- |
| A 維持     | 公開エントリ（プロセス、バインディング、パース、コンパイル）を通すこと自体が検証の対象 | クレート外に残す                |
| B 移設     | 単一モジュールが所有する規則で、公開型だけで入力を作れる                               | 所有モジュールの `tests` へ移す |
| C 帰属違い | 別クレートが所有する規則を、呼び出し側のテストで確認している                           | 所有クレートへ移す              |
| D 統合     | 同じ主張が既存テストと重なる                                                           | 片方に寄せて重複を消す（§6.2）  |
| E 削除     | 主張が実装や定数の再掲に留まる                                                         | 削除する（§6.1）                |

移設先は「その規則を壊したときに最初に直すファイル」を目安にした。テスト込みの手書き Rust ファイルは 900 行以内に
収める運用（[Rustユニットテストの対象棚卸し](rust-unit-test-audit.md)）があるため、そのままでは超える場合は
`src/<module>/tests/<subject>.rs` へ分割する。

配置の判定は、テストが親モジュールの項目（`super::*` で入る型・関数）を実際に使っているかで行う。
使っていないテストはそのモジュールの単体テストではないため、src のテストモジュールには置かず、
検証対象のモジュールを変えるか統合テスト（`tests/`）に残す。たとえばセッション経由でしか観測できない
名前予約や composition lint のケースは、`reserved.rs` や `composition_lint.rs` ではなく
`session/tests.rs`（ProjectSession の振る舞い）に置く。

src のファイル内に書くテストは、大掛かりなセットアップを必要としない単体テストに限る。
公開 API のパイプライン全体（design-system の読み込み、Oxc の解析、モジュールの公開）を回す
クレートテストは `tests/` に置く。`session.rs` は project クレートの主機能なので、そのテストは
`tests/session/` のクレートテストとして持ち、`session.rs` 自身にはテストモジュールを置かない。

### 5.2 crates/source/tests/projection.rs（6 件、全て B）

`ProjectionMap` / `ProjectionIndex` / `VolarMapping` は `src/projection/` が所有し、現在テストが無い。公開型だけで入力を作れる。

| ケース                                                              | 判定 | 移設先                         |
| ------------------------------------------------------------------- | ---- | ------------------------------ |
| exact_range_requires_two_anchors_and_preserves_file_identity        | B    | source/src/projection/index.rs |
| refuses_unmapped_interior_conflicting_duplicates_and_wrong_contents | B    | source/src/projection/index.rs |
| volar_intervals_use_utf16_and_reject_ambiguous_or_malformed_ranges  | B    | source/src/projection/volar.rs |
| adjacent_reordered_volar_ranges_keep_their_own_start_anchors        | B    | source/src/projection/volar.rs |
| v3_surrogate_unit_tokens_are_not_edit_boundaries                    | B    | source/src/projection/v3.rs    |
| empty_spans_require_an_exact_mapped_boundary                        | B    | source/src/projection/index.rs |

### 5.3 crates/diagnostics/tests/contracts.rs（5 件、全て B）

| ケース                                               | 判定 | 移設先                                            |
| ---------------------------------------------------- | ---- | ------------------------------------------------- |
| distinct_causes_share_codes_without_sharing_messages | B    | diagnostics/src/compilation.rs                    |
| one_message_template_can_report_multiple_codes       | B    | diagnostics/src/lint.rs                           |
| wrappers_roundtrip_owned_causes_labels_and_notes     | B    | diagnostics/src/compilation.rs                    |
| generator_failures_report_their_family_and_message   | B    | diagnostics/src/compilation.rs                    |
| suppression_uses_primary_label_and_typed_rule_only   | B/D  | diagnostics/src/suppression.rs（既存 3 件と統合） |

### 5.4 crates/cli/tests/e2e.rs（4 件、全て A）

プロセス起動、終了コード、実ファイルシステム、実パッケージ解決が検証の本体。

### 5.5 crates/design-system/tests/loading.rs（15 件、全て B）

ローダーのプロトコルは `src/loading/` が所有し、`src/loading/tests.rs` に既に 3 件ある。
テスト込み 900 行を超えるため `src/loading/tests/` への分割を前提とする。

| ケース                                                                 | 判定 | 移設先・備考                                                                                                    |
| ---------------------------------------------------------------------- | ---- | --------------------------------------------------------------------------------------------------------------- |
| imported_variant_blocks_obey_later_shorthand_overrides                 | B    | loading/imports.rs                                                                                              |
| loader_distinguishes_cycles_conflicting_identity_and_invalid_css       | B    | loading/state.rs                                                                                                |
| loader_tokenizes_imports_and_ignores_comments_strings_and_nested_rules | B/D  | loading/imports.rs の inline `imports_ignore_comments_strings_and_nested_rules_but_resolve_all_packages` と統合 |
| snapshot_validates_css_resolved_configuration                          | B    | loading/mod.rs                                                                                                  |
| resolved_registries_include_selected_builtins_and_authored_overloads   | B    | snapshot/registry.rs                                                                                            |
| imported_token_spans_refer_to_the_original_supplied_buffer             | B    | loading/expansion.rs（inline 3 件と同主題）                                                                     |
| cycles_are_reported_while_other_files_are_outstanding                  | B    | loading/state.rs                                                                                                |
| loader_validates_batches_against_previously_issued_ids                 | B    | loading/state.rs                                                                                                |
| snapshots_pin_revisions_and_fingerprint_content_without_source_spans   | B    | snapshot/data.rs                                                                                                |
| loader_traverses_opaque_names_in_authored_order_and_reuses_sources     | B    | loading/state.rs                                                                                                |
| partial_out_of_order_responses_preserve_fingerprints_and_spans         | B    | loading/state.rs                                                                                                |
| diamond_sources_share_pending_requests_and_preserve_graph_edges        | B    | loading/state.rs                                                                                                |
| package_updates_replace_loaded_definitions_without_implicit_defaults   | B    | snapshot/registry.rs                                                                                            |
| single_buffer_errors_keep_the_callers_source_identity                  | B/D  | src/loading/tests.rs の `parse_failures_keep_source_identity_and_typed_cause` と統合                            |
| expanded_css_errors_do_not_expose_synthetic_file_ids                   | B    | loading/expansion.rs                                                                                            |

### 5.6 crates/napi/tests/（13 件: A 9、B 3、D 1）

| ケース                                                                                      | 判定 | 根拠・移設先                                                                   |
| ------------------------------------------------------------------------------------------- | ---- | ------------------------------------------------------------------------------ |
| binding: rejected_names_are_transported_beside_symbols                                      | A    | JS へ渡す値の形                                                                |
| binding: lint_severities_keep_their_host_protocol_spellings                                 | B    | napi/src/options.rs（綴りの対応表）                                            |
| binding: projected_runtime_imports_stay_in_their_verified_script_realms                     | A    | バインディング境界                                                             |
| binding: projected_project_input_edits_the_sfc_and_reports_original_coordinates             | A    | 同上                                                                           |
| binding: serializes_a_layered_compile_result_for_node_consumers                             | A    | 同上                                                                           |
| binding: exposes_generated_symbols_for_build_wide_collision_checks                          | A    | 同上                                                                           |
| binding: serializes_cross_module_ref_summaries                                              | A    | 同上                                                                           |
| binding: requires_typescript_facts_for_cross_module_refs                                    | A    | 同上                                                                           |
| binding: assembles_project_fragments_under_one_shared_stylesheet                            | A    | 同上                                                                           |
| binding: serializes_editor_positions_for_diagnostics                                        | B    | napi/src/output.rs                                                             |
| binding: generates_theme_backed_shared_vocabulary_augmentations                             | D    | compiler/src/compiler/vocabulary.rs の inline と主題が同一。経由する差だけ残す |
| binding: project_api_preserves_configuration_errors                                         | B    | napi/src/options.rs                                                            |
| comment_contracts: transform_mappings_use_utf16_even_with_multibyte_source_and_replacements | A    | JS へ渡す座標単位                                                              |

### 5.7 crates/project/tests/（22 件: A 1、B 15、D 5、B/D 1）

composition_lint.rs（7 件）

| ケース                                                    | 判定 | 根拠                                                                                                                 |
| --------------------------------------------------------- | ---- | -------------------------------------------------------------------------------------------------------------------- |
| independent_bases_that_share_a_property_are_reported      | D    | src/composition_lint.rs の `independent_bases_with_one_property_are_reported` と同主題（合成 artifact 版が既にある） |
| independent_bases_with_disjoint_properties_stay_silent    | D    | inline の `unrelated_properties_stay_silent`                                                                         |
| base_pseudo_class_and_attribute_variants_stay_independent | D    | inline の `variant_contexts_do_not_interfere`                                                                        |
| a_shorthand_base_and_a_longhand_base_are_reported         | D    | inline の `a_shorthand_and_a_longhand_interfere`                                                                     |
| a_logical_base_and_a_physical_base_are_reported           | B    | composition_lint.rs（inline に論理/物理の組が無い）                                                                  |
| imported_base_composition_requires_checker_facts          | A    | モジュール 2 つと checker facts が必要                                                                               |
| suppression_directives_remove_the_findings                | B    | composition_lint.rs（diagnostics 側は字句のみ）                                                                      |

session.rs（15 件）: セッションと名前プールの規則は `src/session.rs`、`src/reserved.rs`、`src/project_stylesheet.rs` が所有する。

| ケース                                                                     | 判定 | 移設先・備考                                                                                               |
| -------------------------------------------------------------------------- | ---- | ---------------------------------------------------------------------------------------------------------- |
| publication_rekeys_publicly_edited_rules_before_sharing_them               | B    | session（公開時の再採番）                                                                                  |
| codegen_reload_replaces_artifacts_without_reloading_definitions            | B    | session                                                                                                    |
| module_stylesheets_include_support_without_repeating_document_rules        | B    | project_stylesheet.rs                                                                                      |
| call_identity_is_stable_and_module_and_namespace_specific                  | B/D  | compiler の `symbols.rs` inline `identities_normalize_paths_and_class_names_are_content_stable` と重複確認 |
| stylesheet_reference_counts_rules_and_preserves_last_good_reload           | B    | session と reserved                                                                                        |
| failed_source_update_preserves_watch_artifact_but_not_build_artifact       | D    | session.rs inline の watch/build 2 件と統合                                                                |
| unchanged_artifacts_are_reused_and_dependencies_are_removed                | B    | session                                                                                                    |
| dependency_replacement_retains_other_producers_and_clears_last_memberships | B    | session                                                                                                    |
| generated_names_never_reuse_an_authored_name_in_the_same_namespace         | B    | reserved.rs（inline 9 件と同主題群）                                                                       |
| distinct_pools_may_publish_the_same_spelling                               | B    | reserved.rs                                                                                                |
| session_generators_name_generated_classes                                  | B    | session                                                                                                    |
| a_helper_name_is_checked_in_the_custom_property_pool                       | B    | reserved.rs                                                                                                |
| css_a_producer_authors_is_reserved_when_the_host_supplies_it               | B    | reserved.rs                                                                                                |
| a_layer_name_is_checked_against_the_authored_document                      | B    | reserved.rs                                                                                                |
| a_generated_anchor_name_is_checked_against_the_authored_document           | B    | reserved.rs                                                                                                |

### 5.8 crates/compiler/tests/（187 件）

#### comment_contracts.rs（10 件: C 8、B 1、A 1）

名前が内容と一致しない。span、theme、resource の主題が 4 クレートにまたがる。

| ケース                                                               | 判定 | 移設先                                                                                                    |
| -------------------------------------------------------------------- | ---- | --------------------------------------------------------------------------------------------------------- |
| spanned_equality_includes_the_source_range                           | C    | source（`Spanned` の等価性）                                                                              |
| line_terminators_and_eof_are_addressable_scalar_boundaries           | C    | source/src/coordinates.rs（inline と関連）                                                                |
| agreeing_overlapping_projection_ranges_are_accepted                  | C    | source/src/projection/index.rs（§5.2 と同じ移設先）                                                       |
| active_theme_tokens_apply_resets_then_sort_by_name                   | C    | design-system/src/theme/index.rs                                                                          |
| ordinary_project_tokens_are_pruned_but_static_tokens_are_retained    | C    | design-system/src/theme/                                                                                  |
| failed_user_branch_can_resolve_through_the_builtin_registry          | C    | design-system/src/evaluation/utilities/                                                                   |
| unusable_checker_facts_become_a_compilation_error                    | A    | js-oxc と compiler の連携                                                                                 |
| document_rendering_keeps_the_module_fragment                         | B    | compiler/src/renderer.rs                                                                                  |
| an_empty_utility_is_unresolved_and_produces_no_rules                 | C    | design-system/src/evaluation/utilities/                                                                   |
| resource_specifiers_are_decoded_and_repeated_urls_share_placeholders | C    | css/src/resources.rs（inline `distinguishes_resources_from_strings_comments_and_absolute_urls` と同主題） |

#### audit_regressions.rs（19 件: C 11、B 6、A 2）

コメント監査で見つかった不具合の回帰を 1 ファイルに集めたもの。回帰の主題ごとに所有モジュールへ分ける。

| ケース                                                                         | 判定 | 移設先                                           |
| ------------------------------------------------------------------------------ | ---- | ------------------------------------------------ |
| deeply_nested_utility_values_fail_with_a_nesting_diagnostic                    | C    | css（`MAX_VALUE_NESTING`）                       |
| utility_precedence_is_independent_of_literal_and_array_order                   | C    | design-system（utility order）                   |
| css_values_survive_emission_and_importance                                     | C    | css（emission と declarations）                  |
| mixed_variant_forms_override_in_source_order_even_on_one_line                  | C    | design-system（variant order）                   |
| recursive_theme_macros_reject_the_candidate_without_partial_output             | C    | design-system（theme プレースホルダ）            |
| reset_tokens_are_absent_from_definition_fingerprints                           | C    | design-system（fingerprint）                     |
| escaped_utility_diagnostics_keep_the_literal_provenance                        | C    | js-oxc または syntax（候補のエスケープ）         |
| external_motion_identity_does_not_become_a_local_source_span                   | C    | js-oxc（identity と span の付与）                |
| local_motion_identity_stays_original_while_analysis_spans_are_mapped           | C    | js-oxc（投影）                                   |
| literal_values_and_explicit_fallbacks_survive_utility_emission                 | C    | design-system（値の fallback）                   |
| explicit_registration_dependencies_survive_apply_and_do_not_leak_into_css      | C    | design-system（custom property 登録）            |
| punctuation_labels_produce_valid_javascript_strings                            | B    | compiler/src/identifiers/name.rs                 |
| a_label_reaches_every_name_its_own_declaration_generates                       | B    | compiler/src/identifiers/tests.rs（同主題群）    |
| static_ref_projection_reads_the_use_site_receiver                              | B    | compiler/src/compiler/emission/ref_projection.rs |
| static_ref_projection_reads_a_member_receiver                                  | B    | 同上                                             |
| local_animation_definitions_follow_the_explicit_keyframes_slot                 | B    | compiler/src/compiler/stylesheet/template/       |
| motion_definitions_transport_explicit_property_dependencies_without_directives | B    | 同上                                             |
| motion_css_is_retained_for_uses_and_exports_only                               | A    | コンパイルと依存追跡の連携                       |
| dynamic_motion_keeps_every_checker_candidate_and_runtime_binding               | A    | TypeScript facts との連携                        |

#### 主題別の小さいファイル（38 件、B 中心）

| ファイル                   | 件数 | 判定     | 移設先                                                                                                                                                                                                      |
| -------------------------- | ---- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| query.rs                   | 3    | B        | compiler/src/query.rs（inline 1 件と同じモジュール）                                                                                                                                                        |
| imports.rs                 | 2    | B        | compiler/src/imports.rs                                                                                                                                                                                     |
| stylesheet_map.rs          | 1    | B        | compiler/src/stylesheet_map.rs（inline 4 件）                                                                                                                                                               |
| static_structure.rs        | 4    | B        | compiler/src/compiler/class_units/ と fragment/                                                                                                                                                             |
| animation_branches.rs      | 3    | B        | compiler/src/compiler/definitions/keyframes.rs                                                                                                                                                              |
| macro_contract.rs          | 2    | B        | compiler/src/compiler/fragment/render.rs                                                                                                                                                                    |
| utility_order.rs           | 2    | C 1・B 1 | `tailwind_resolved_property_sort_matches_upstream` は design-system（順序表の所有者）。`emitted_precedence_uses_property_sequences_counts_and_numeric_names` は compiler/src/compiler/utilities/emission.rs |
| design_system_rendering.rs | 6    | B        | compiler/src/renderer.rs（loader の順序保証は design-system 側で検証済み）                                                                                                                                  |
| module_compiler.rs         | 15   | B        | compiler/src/module_compiler.rs と compiled_module.rs                                                                                                                                                       |

`utility_order.rs` の移設では、`@stillwind/tools#tailwind:utility-order` が
`cargo test -p stillwind-compiler --test utility_order` を実行している点も合わせて更新する。

#### static_compile/（120 件: A 105、C 15）

受理スイートとして維持する。ただし次の 15 件は design-system が所有する規則の確認で、design-system の
inline スイートと主題が重なる。1 件ずつ「解決」と「emission」に分けるか、design-system 側へ寄せる。

| ケース                                                                           | 所有者                                                                                      |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| variants: reports_variant_selectors_that_do_not_depend_on_the_styled_element     | design-system（`document_scoped_selector`。compiler 側は lint 配線だけ残す）                |
| variants: theme_gate_emits_only_required_default_tokens_and_keyframes            | design-system（theme ゲート）                                                               |
| variants: breakpoint_variants_inline_queries_without_emitting_lookup_tokens      | design-system（breakpoint 解決）                                                            |
| variants: utility_ir_gate_keeps_fallbacks_and_selector_overloads                 | design-system（overload と fallback）                                                       |
| variants: emits_nested_supports_from_css_utility_definitions                     | design-system（inline `preserves_supports_and_media_conditions_from_css_utilities` と重複） |
| variants: emits_nested_media_from_css_utility_definitions                        | 同上                                                                                        |
| variants: emits_selector_bearing_css_utilities_on_their_target_children          | design-system（inline `preserves_a_nested_selector_from_a_css_utility` と重複）             |
| variants: emits_starting_style_variants_as_a_css_wrapper                         | design-system（variant body）                                                               |
| utilities: retains_authored_custom_properties_and_theme_selectors                | design-system（authored の保持）                                                            |
| utilities: resolves_v4_default_spacing_and_arbitrary_custom_properties           | design-system（既定 theme と arbitrary）                                                    |
| utilities: resolves_css_defined_custom_utilities_and_variants                    | design-system（inline custom_variants と重複）                                              |
| utilities: resolves_negative_and_transition_utilities_from_declared_theme_tokens | design-system（負値と transition）                                                          |
| theme: renders_effective_theme_custom_properties_with_tailwind_reset_semantics   | design-system（theme 出力モード）                                                           |
| theme: shadow_utilities_follow_multiple_theme_overrides_and_namespace_removal    | design-system（namespace reset）                                                            |
| ref_variants: an_inline_bounded_ref_keeps_the_ancestry_check                     | js-oxc（refs/validation の inline と重複寄り）                                              |

### 5.9 crates/js-oxc/tests/（45 件: A 26、B 19）

`parse` と `OxcSourceAnalyzer` はクレートの公開エントリなので、抽出・投影・テンプレートの契約はクレート外のままでよい。
次の 19 件は特定モジュールの規則で、入力は公開型だけで組み立てられる。

| ケース                                                                                  | 判定 | 移設先                                       |
| --------------------------------------------------------------------------------------- | ---- | -------------------------------------------- |
| parse: ir_span_mapping_relocates_nested_expressions_transactionally                     | B    | js-oxc/src/projected_scan.rs                 |
| parse: framework_registry_preserves_import_aliases_and_shadowing                        | B    | js-oxc/src/style_bindings.rs                 |
| parse: type_only_macro_imports_do_not_select_a_framework_or_expand_calls                | B    | js-oxc/src/style_bindings.rs                 |
| parse: selects_solid_from_the_macro_binding_and_rejects_mixed_frontends                 | B    | js-oxc/src/style_bindings.rs                 |
| parse: import_edit_metadata_tracks_macro_references_and_preserves_mixed_bindings        | B    | js-oxc/src/imports.rs                        |
| parse: module_specifier_spans_preserve_quotes_and_utf8_offsets                          | B    | js-oxc/src/imports.rs                        |
| parse: parses_named_and_bounded_ref_declarations                                        | B    | js-oxc/src/refs/definitions.rs               |
| parse: ref_definition_requires_one_boundary_identity                                    | B    | js-oxc/src/refs/definitions.rs               |
| parse: unclaimed_keyframes_uses_report_diagnostics                                      | B/D  | js-oxc/src/keyframes.rs（inline 1 件と統合） |
| parse: claimed_keyframes_uses_stay_diagnostic_free                                      | B/D  | 同上                                         |
| analysis: fresh_facts_with_invalid_symbolic_kinds_are_rejected_at_both_entries          | B    | js-oxc/src/analyzer.rs                       |
| analysis: analysis_is_design_system_independent_and_rejects_stale_facts                 | B    | js-oxc/src/analyzer.rs                       |
| analysis: scan_reports_expression_probes_for_non_local_asset_values                     | B    | js-oxc/src/analyzer.rs                       |
| macro_contract: calls_constructors_and_local_computation_need_no_pure_annotation        | B    | js-oxc/src/contract.rs                       |
| macro_contract: visible_mutations_produce_findings_even_in_empty_branches_or_pure_calls | B    | js-oxc/src/contract.rs                       |
| macro_contract: execution_boundaries_must_be_evaluated_outside_macros                   | B    | js-oxc/src/contract.rs                       |
| macro_contract: keyed_assets_and_macros_inside_function_bodies_are_validated            | B    | js-oxc/src/contract.rs                       |
| macro_contract: container_projection_rejects_extra_arguments                            | B    | js-oxc/src/contract.rs                       |
| module_css_imports: explicit_module_css_tracks_value_import_literals_only               | B    | js-oxc/src/imports.rs                        |

維持する 26 件は `parse.rs` の抽出契約 12 件、`ref_variants.rs` 4 件、`projected_analysis.rs` 10 件。
このうち `projection_failures_are_atomic_and_configuration_is_part_of_the_revision` は
`src/projected_scan.rs` への移設を要確認とする。

### 5.10 JS/TS: パッケージ外のケース

パッケージ直下の `test/` はこのリポジトリのコロケーション単位であり、大半は妥当。
対象が他パッケージにあるケースだけを挙げる。

| ケース                                                                                                  | 判定 | 根拠                                                                     |
| ------------------------------------------------------------------------------------------------------- | ---- | ------------------------------------------------------------------------ |
| integration/vite/config-root: examples/react keeps its project root when its config is loaded elsewhere | C    | 対象は examples/react の設定。統合側で守るなら、何を守るのかを名前で示す |
| integration/vite/config-root: apps/play keeps its project root when its config is loaded elsewhere      | C    | 同上                                                                     |
| core/css-entrypoints: the aggregate CSS entrypoint includes the built-in registry                       | B    | パッケージ構成の検査。tools の配布物検査へ寄せる選択肢がある             |
| core/css-entrypoints: every exported CSS import stays inside the published package                      | B    | 同上                                                                     |
| core/css-entrypoints: builtins.css imports every split registry file                                    | B    | 分割一覧の写し。生成側の検査にするか、実行時テストから外す               |

### 5.11 クレート内だがモジュール外のケース

| 現在                                                                                                    | 判定 | 移設先                                                                 |
| ------------------------------------------------------------------------------------------------------- | ---- | ---------------------------------------------------------------------- |
| design-system/src/tests/theme.rs（13 件）                                                               | B    | design-system/src/theme/tests/（`src/theme/*` の inline と主題が近い） |
| compiler/src/compiler/{tests.rs, fragment/tests.rs, stylesheet/template/tests.rs}、identifiers/tests.rs | 維持 | 実装モジュール直下の分離テストとして妥当                               |

## 6. 削除候補（ケース単位）

### 6.1 E: 他のテストや定義の再掲

| 対象                                                                        | 根拠                                                                                                              |
| --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| js-oxc/src/entry.rs::accepts_typescript_jsx                                 | TSX のパースは js-oxc/tests/parse.rs（22 件）が網羅する。診断が空であることだけを見るスモークテスト               |
| ir/src/framework/mod.rs::frameworks_declare_their_materialized_style_format | 直上に定義された静的テーブルの再確認。`class_property == "className" \|\| class_property == "class"` は恒真に近い |
| core/test/runtime.test.mts の merge ケース                                  | fragment.test.mts の merge ケースと同じ関数・同じ観点。1 ファイルに統合する                                       |

### 6.2 D: 同主題の重複（統合して片方を残す）

| 対象                                                                                                                                                                                                                                                                    | 統合先                                                                        |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| project/tests/composition_lint.rs の 4 件（independent_bases_that_share_a_property_are_reported、independent_bases_with_disjoint_properties_stay_silent、base_pseudo_class_and_attribute_variants_stay_independent、a_shorthand_base_and_a_longhand_base_are_reported） | src/composition_lint.rs の inline 4 件（合成 artifact 版）                    |
| project/tests/session.rs::failed_source_update_preserves_watch_artifact_but_not_build_artifact                                                                                                                                                                          | src/session.rs inline の watch/build 2 件                                     |
| design-system/tests/loading.rs::single_buffer_errors_keep_the_callers_source_identity                                                                                                                                                                                   | src/loading/tests.rs の `parse_failures_keep_source_identity_and_typed_cause` |
| design-system/tests/loading.rs::loader_tokenizes_imports_and_ignores_comments_strings_and_nested_rules                                                                                                                                                                  | src/loading/imports.rs の inline                                              |
| compiler/tests/comment_contracts.rs の C 8 件                                                                                                                                                                                                                           | 移設先（source、design-system、css）の既存 inline                             |
| compiler/tests/static_compile/variants.rs の supports・media・nested selector 3 件                                                                                                                                                                                      | design-system/src/evaluation/tests/relationships.rs と utility_domains.rs     |
| napi/tests/binding.rs::generates_theme_backed_shared_vocabulary_augmentations                                                                                                                                                                                           | compiler/src/compiler/vocabulary.rs の inline                                 |
| js-oxc/tests/parse.rs の keyframes 2 件                                                                                                                                                                                                                                 | js-oxc/src/keyframes.rs の inline                                             |

削除・統合の前後で `cargo test --workspace` と該当パッケージのテストを実行し、他のテストが同じ主張を検出することを確認する。

### 6.3 層間の重なり（削除ではなく役割の再確認）

| 組                                                                                                                             | 論点                                                                                                             |
| ------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| host/test/conditional-flow.test.mts（22 件）と compiler/tests/static_compile/{folding,conditional_flow,runtime_if}.rs（37 件） | 畳み込みと条件分岐の主題が重なる。host 側はソースマップと型 facts に固有の事例へ絞れるか確認する                 |
| native/test/loader.test.mjs（12 件）と host/test/editor.test.mts（6 件）                                                       | セッション復旧、type facts、revision 維持が両方にある。Rust の napi/tests/binding.rs を含めた 3 層で役割を決める |
| core/test/fragment.test.mts（10 件）と compiler/src/compiler/fragment/tests.rs（8 件と fragment-execution.mjs）                | 生成コードとランタイムを突き合わせる差分実行。重複ではなく対の検証なので維持する                                 |
| host/test/naming.test.mts と integration/test/vite/naming.test.mts                                                             | host の名前生成契約とプラグイン経由の出力。層が違うため維持を推奨                                                |
| compiler/tests/static_compile/parity.rs と tools/tailwind-parity.mts                                                           | フィクスチャ固定の回帰ゲートと上流オラクル比較。役割が違うため維持を推奨                                         |

### 6.4 残すもの

- proptest（UTF-16 境界、正規化の冪等性、順序不変性などの不変条件）
- fixtures/view-transitions/name-escapes.json を Rust と JS の双方が読む共有ベクタ
- `#[ignore]` の builtin 到達性監査（tailwind:registry-audit で手動実行）
- プロセス E2E、配布物検査（entrypoints.test.mts）、フレームワーク公式ビルドの受理テスト

## 7. 不足

### 7.1 実行基盤

1. CI が無く、pre-commit もテストを実行しない。`cargo test --workspace` と JS テストは手動実行に依存する
2. テストを集約するルートタスクが無い。`vp run -r test` の対象は host、integration、play のみ
3. `#[ignore]` の監査を定期実行する経路が無い（手動タスクのみ）
4. カバレッジ計測（Rust の llvm-cov、JS 側の coverage）が無い
5. `crates/compiler/tests/fragment-execution.mjs` はどのパッケージにも属さず、`cargo test` から Node を起動して実行される。依存と実行条件（Node のバージョン、`packages/core` の dist の有無）がタスクグラフに現れない

### 7.2 対象範囲

6. `packages/projection`: テストがゼロ。SFC 投影（vue、svelte、astro）と checker-source は host のテストから間接的にしか触れられていない
7. `packages/fixtures-viewer` と `examples/react`: 挙動テストが無い（型検査のみ）
8. `packages/host/test/` が `tsc` の対象外。12 ファイル、約 1900 行が型検査されていない
9. `crates/ir`（1 件）と `crates/semantic`（6 件、うち doc test 1）が薄い。モデル、spans、composition は他クレート経由の間接検証に留まる
10. tools の主要スクリプト自体のテストが無い（`review-fixtures.mts`、`tailwind-parity.mts`、`tailwind-numeric-domain-audit.mts`）。フィクスチャ生成の誤りは下流のゲートでは検出できない
11. フレームワーク別 runtime（preact、solid、vue）の直接テストが無い。`createFragmentRuntime` の薄いラッパーであり、react と svelte/astro の 2 形状が検証されているため実害は小さいが、class 属性名の対応表は型検査のみで守られている
12. `apps/play/server/preview.ts` は `playground.test.mts` の 1 件でしか触れられていない
13. `crates/js-oxc` の macro 契約（`contract.rs`）と解析入口（`analyzer.rs`）には実装内テストが無く、契約は `tests/` にのみ存在する
14. `crates/compiler/src/renderer.rs`、`crates/project/src/session.rs`、`crates/compiler/src/module_compiler.rs` など、公開面の主要モジュールのテストが `tests/` にしかない（§5 の B 判定）

### 7.3 Vitest 移行

現状は全パッケージが `node:test` を使い、`pnpm-workspace.yaml` には `vitest: 4.1.11` の override だけがある。移行時の作業単位はパッケージの `test/` ディレクトリで、import を `vite-plus/test` に置き換える。

移行時に一緒に解消できるもの:

- テスト系タスク名を `test` に統一し、ルートの集約タスクを追加する
- `packages/host/test` を typecheck の対象に含める
- `src` と `dist` の import 規則を決める（現在は混在し、dist 依存のテストはビルドを要求する）

注意が必要なテスト:

- ビルドを伴う統合テスト（`framework-builds.mts`、`webpack/*.test.mts`、`module-css.test.mts`、`session.test.mts`）はタイムアウトと並列度の調整が要る
- `packages/native/test/wasm.test.mjs` は環境変数を立てて `loader.test.mjs` を再 import する構造
- `tools/*.test.mts` は top-level await で `test` を呼ぶ
- `packages/integration/test/vite/framework-dev.mts` はテストではなく HMR 用ハーネスなので移行対象から外す

## 8. 実行コマンド早見表（現状）

```powershell
# Rust
cargo test --workspace
cargo test --workspace -- --list          # 件数の確認
cargo test -p stillwind-design-system audits_builtin_registry_reachability_from_parity_fixtures -- --ignored

# JS/TS
$env:CARGO_INCREMENTAL='0'
vp run --filter stillwind check           # core: build + typecheck + tests
vp run --filter @stillwind/host test
vp run --filter @stillwind/integration test
vp run --filter @stillwind/integration test:frameworks
vp run --filter @stillwind/native check
vp run --filter @stillwind/play test

# 検査
vp run check
vp run crates:check
vp run fixtures:check
vp run webref:check
vp run tailwind:parity:all
```

## 9. 実施記録（2026-09-21）

### 9.1 実施した整理

| 対象                                                                  | 内容                                                                                                                                                                                                                                                                            |
| --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| source/tests/projection.rs（6）                                       | src/projection/tests.rs へ移設し、統合ターゲットを削除                                                                                                                                                                                                                          |
| diagnostics/tests/contracts.rs（6）                                   | compilation.rs（4）、lint.rs（1）、suppression.rs（1、既存 3 件へ統合）へ移設                                                                                                                                                                                                   |
| design-system/tests/loading.rs（15）                                  | src/loading/tests.rs へ 13 件を移設。loader_tokenizes_... は loading/imports.rs の inline へ url() 例として統合、single_buffer_errors_... は parse_failures_... へ統合                                                                                                          |
| project/tests/session.rs（15）                                        | 一度 src/session/tests.rs へ寄せた後、クレートテストとして tests/session/ へ戻し、補助関数の単位で分割した（§9.4）。failed_source_update_... は watch/build の 2 件へ統合                                                                                                       |
| project/tests/composition_lint.rs（7）                                | ルール単体と重複する 4 件を削除、2 件を src/composition_lint.rs へ移設。checker facts が要る 1 件のみ統合ターゲットに残置                                                                                                                                                       |
| project の reserved 系（6）と composition lint のセッション経由 2 件  | tests/session/naming.rs と tests/session/reserved.rs、composition lint の 2 件は tests/composition_lint.rs へ（同じ補助関数を使うファイルに寄せる）                                                                                                                             |
| project の publication_rekeys / call_identity                         | 前者は src/project_stylesheet.rs へ移設、後者は compiler の symbols テストへ namespace 検証として統合                                                                                                                                                                           |
| compiler/tests/query.rs（3）、imports.rs（2）、stylesheet_map.rs（1） | src/query.rs、src/imports.rs、src/renderer.rs（provenance の記録は描画側の単位）へ移設                                                                                                                                                                                          |
| compiler/tests/comment_contracts.rs（10）                             | source（text.rs、coordinates.rs、projection/tests.rs）3 件、css（resources.rs）1 件、design-system（theme/index.rs 2 件、evaluation/utilities/resolve.rs 1 件、loading/tests.rs 1 件）、compiler（query.rs、renderer.rs、module_compiler.rs）2 件へ分配し、統合ターゲットを削除 |
| packages/core/test/runtime.test.mts                                   | merge ケースを fragment.test.mts へ統合（frozen 入力と delta の検証を追加）                                                                                                                                                                                                     |

件数: Rust 666 → 657（統合・削除 9 件、移設 64 件）。統合テストターゲットは 26 → 18（全体 49 → 41）。

### 9.2 検証

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace           # 全ターゲット成功（657 件、ignored 1）
vp run --filter stillwind check  # core の typecheck とテスト
vp run fixtures:check            # 8 プロジェクトの出力一致
```

### 9.3 残り（未実施）

| 対象                                                                                                                                                            | 予定                                                                                                                 |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| compiler/tests/static_compile の重複 4 件                                                                                                                       | design-system の inline と重複する supports、media、nested selector、custom-utilities-variants を削除                |
| compiler/tests の主題別小ファイル（static_structure 4、animation_branches 3、macro_contract 2、design_system_rendering 6、module_compiler 15、utility_order 2） | 所有モジュールへ移設（module_compiler は src/module_compiler/tests.rs へ分割）                                       |
| compiler/tests/audit_regressions.rs（19）                                                                                                                       | C 11 件を所有クレートへ、B 6 件を実装モジュールへ分配                                                                |
| crates/js-oxc/tests（19）                                                                                                                                       | contract.rs、analyzer.rs、imports.rs、style_bindings.rs、refs/definitions.rs、keyframes.rs、projected_scan.rs へ移設 |
| crates/napi/tests（3 移設、1 統合）                                                                                                                             | options.rs、output.rs へ移設し、vocabulary の 1 件は compiler の inline と統合                                       |
| packages/integration/test/vite/config-root.test.mts、packages/core/test/css-entrypoints.test.mts                                                                | 対象が他パッケージまたは配布物構成のケース。tools の検査へ寄せるか、対象アプリ側へ移す                               |

### 9.4 project のセッションテストの分割（2026-09-22）

セッションのテストはクレートテストとして `crates/project/tests/` の直下にフラットに置く。
`tests/` にディレクトリは作らず、各ファイルが使う補助関数だけをローカルに持つ（テストターゲットは別クレートなので、
補助関数はファイル間で共有しない。共通の `crates/test-support/css.rs` だけを `#[path]` で取り込む既存の方式に合わせる）。

| ファイル                   | 行数 | ローカルの補助関数                                     | 内容                                                                    |
| -------------------------- | ---- | ------------------------------------------------------ | ----------------------------------------------------------------------- |
| tests/session_reload.rs    | 174  | module、reload_snapshot、reload_module、failing_module | design-system 再読み込みの失敗と watch/build の差                       |
| tests/session_artifacts.rs | 288  | snapshot、module                                       | artifact の差し替え・再利用と依存インデックスの増減                     |
| tests/session_naming.rs    | 228  | snapshot、module                                       | 注入した generator が決める名前と family ごとの prefix                  |
| tests/session_reserved.rs  | 139  | snapshot、ref_module                                   | 著者 CSS との衝突（family ごとのプール、producer の CSS、公開時の検出） |
| tests/composition_lint.rs  | 108  | snapshot、analysis、session                            | composition lint の 3 件（facts 必須、論理と物理、suppression）         |

- フラットなファイルは cargo が自動検出するため `[[test]]` の宣言は不要（ディレクトリ型ターゲットを作らない）
- `session.rs` に残るテストは無い。セッションの検証はすべてパイプライン全体を必要とするため、src 内に置く単体テストは該当しない
- `crates/project/src/lib.rs` の `#[cfg(test)] mod css_fixture` は src 側で使わなくなったため削除する
