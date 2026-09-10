# Framework projection とソース編集の設計・実装目標

Status: 実装中。2026-09-10 の合意を記録する。実装状況は下記で区別し、未完了の API 名は契約を説明するための案とする。

本設計では、host が Vue/Svelte/Astro を JS/TS/JSX/TSX に投影し、Rust が OXC で解析する。Rust は元 SFC を構文解析しない。`__VLS_ctx` や `className` / `class` の違いに必要な最小限の知識は、1フレームワーク1ファイルに集約する。解析後の編集は元 SFC に直接適用し、投影コードから SFC を再生成しない。

この範囲について、本書は [compiler-design.md](compiler-design.md) の将来 frontend 方針を具体化・更新する。[crate-architecture.md](crate-architecture.md) の依存方向と既存の snapshot/session の責務は維持する。

## 1. 到達目標

1. JS/TS/JSX/TSX と Vue/Svelte/Astro が、共通のマクロ解析・IR・CSS生成・編集処理を利用する。
2. フレームワーク追加・削除で変更する Rust 実装は、当該ファイルと `framework/mod.rs` の登録だけにする。公開型の生成、host converter 登録、fixture の追加・削除は別途必要とする。
3. 元ソースの未編集部分を byte 単位で保持する。改行・コメント・引用符・SFC の markup/style を再整形しない。
4. 編集範囲は map と元テキストの検証で確定する。曖昧な推測による編集を行わない。
5. JS 側のソース編集を最小化し、native で確定した編集計画から一度の走査で出力コードと対応情報を生成する。
6. import アンカー、runtime import、明示的 module-css import を、通常ソースと SFC で共通に扱う。
7. 既存の React/Preact/Solid 出力と診断を維持し、追加 framework は実際の converter と native と bundler を通して検証する。

型情報の新規連携、TS checker の投影対応、SFC parser の Rust 実装、任意の framework compiler 出力の復元は今回の範囲外。既存の型情報利用経路を壊さないことは回帰条件に含める。

## 2. 責務と配置

```text
host: SFC converter 呼び出し、map の受け渡し、module 解決、bundler lifecycle
                 |
                 v
js-oxc: OXC parse / binding 解決 / 共通パターン照合
                 | framework の記述を参照
                 v
ir: framework 契約 / 中立な解析結果 / 編集元の provenance
                 |
                 v
compiler: lowering / CSS / import 計画 / 検証済みソース編集
                 |
                 v
host: virtual module の提供、framework compiler へ編集済み SFC を返す
```

Rust の集約先は既存の依存方向に合わせ、次を第一案とする。新規 crate は作らない。

```text
crates/ir/src/framework/
  mod.rs       # 共通の契約、識別子、選択、登録
  react.rs
  preact.rs
  solid.rs
  vue.rs
  svelte.rs
  astro.rs
```

`ir` に OXC AST 型や converter 依存を持ち込まない。各ファイルは中立な設定・パターン・判定に必要な記述を返す。OXC AST をその記述と照合する汎用処理は `js-oxc`、出力する共通処理は `compiler` に置く。既存 `compiler/react/props.rs` の共通 props 生成は中立な名前に整理し、現在の Solid 判定のような分岐は descriptor 参照へ置き換える。

framework 記述が所有するもの:

- class property、props/style 出力上の差異、対応 capability。
- マクロ/runtime の import specifier とその認識規則。
- 投影固有の補助 receiver や wrapper の許容形と、その provenance 要件。
- script/template の対応可否、必要な import 配置制約。

共通契約は概念的に `FrameworkSpec { id, imports, props, projection, capabilities }` とする。`projection` は検出・比較・原文対応のための限定された拡張点である。単なる prefix 一覧には固定しない。宣言的な規則で表せない変形は、中立な式の view と読み取り専用の binding/map 情報を受ける framework 内の処理として実装できるようにする。

コアは OXC AST からその view を提供し、framework は対応候補と根拠を返す。framework 処理が AST や編集計画を直接変更することは認めず、source の境界・対応の一意性・原文の検証は共通処理が担う。汎用操作の追加が必要な場合も framework 名による分岐を持ち込まない。

Rust の実装コードで `__VLS_ctx` などの固有名を使うのは当該 framework ファイルだけとする。テスト・文書・host の converter 呼び出しにはこの制約を適用しない。host に出力規則を重複実装しない。

選択は host が渡す明示的 framework ID を基本とする。既存 import による選択は共通 registry による検証に統合する。明示指定と import が衝突する場合は診断し、別 framework へ黙って切り替えない。通常入力／投影入力の区別と framework の種類は独立した軸にする。

## 3. 入力と map の契約

### 通常入力

`code, map?` を受け取る。`code` が OXC の解析対象かつ編集対象。`map` は入力 code から上流原本への map であり、存在しても編集対象を上流原本に切り替えない。

### 投影入力

`code, fw_code, fw2js_map, fw_map?` を受け取る。

| フィールド | 意味                                                                            |
| ---------- | ------------------------------------------------------------------------------- |
| code       | OXC が解析する投影 JS/TS/JSX/TSX                                                |
| fw_code    | 編集対象となる元 SFC                                                            |
| fw2js_map  | converter が SFC→JS 生成時に作る map。検索方向は生成 code の位置→fw_code の位置 |
| fw_map     | fw_code→さらに上流の原本への任意 map                                            |

両モードとも module ID、parser source type、framework ID を明示する。投影入力には converter 名・version・設定 fingerprint を持たせ、対応パターンの検証と cache invalidation に利用する。必要な script region / import realm の対応情報は host が converter の結果から渡す。Rust で SFC タグを探して補完しない。

`fw2js_map` は欠落不可。空・不正・異なる source の map は編集可能と見なさない。Volar mapping は v3 JSON と偽装せず、タグ付き mapping 入力として保持する。v3 と Volar を共通の位置／範囲検索インターフェースへ正規化する。Volar→v3 変換を使う場合は境界・未マップ区間・多対一対応を保持できることを先に検証する。

OXC span は UTF-8 byte、source-map column と JS offset は UTF-16。型または明示的な変換関数で座標系を分離し、surrogate 内部・不正な UTF-8 境界・範囲外を拒否する。異なる source の同じ数値 offset を混同しない。

## 4. 投影による変形を検出・比較・編集から分離する契約

framework converter は、変数参照、call、リアクティブな読み取りなどを別の JS 表現へ変形できる。元の式と投影式の構文・文字列が一致することを、マクロ検出や式比較の前提にしない。Vue の補助 receiver はその一例であり、除去処理だけを特別扱いする設計にしない。

| 共通処理が必要とする操作 | framework の拡張点が提供するもの                                 |
| ------------------------ | ---------------------------------------------------------------- |
| マクロ・参照・引数の検出 | 投影上の形から、元の binding／式に対応する候補を認識する規則     |
| 一致比較                 | 許容された変形を考慮した論理的な参照・式の view と、その対応根拠 |
| 元 span の検証           | 合成部分と元由来の部分の区別、元の式との照合に必要な対応情報     |
| 編集結果での再利用       | 検証可能な元式の範囲。出力文字列は共通処理が元 source から取得   |

式の比較では、binding と構造に基づく一致と、編集対象の原文照合を区別する。投影式の文字列が違うだけで検出に失敗したり、同じ property 名というだけで別 binding を一致扱いしたりしない。正規化は任意の JavaScript の意味的同値性を判定するものではなく、対応 converter の既知の変形に限定する。原文照合は引き続き必要であり、論理的な一致で代用しない。

拡張点の結果は「対応できた」「対象外」「対応不能／曖昧」を区別する。マクロ候補と分かったものの対応に失敗した場合は診断し、通常の無関係な式として黙って落とさない。認識・比較・編集で別々の prefix 除去を実装せず、同じ対応結果を利用する。

実装中の共通経路は、OXC から中立な式の view を作り、framework が返す保持範囲を map と原文で検証して、内部の解析用 source を構築する。この source を OXC で解析することで、既存の binding 追跡と式比較にも同じ正規化を適用する。元投影との断片対応は `NormalizedProjection` が保持する。出力用 IR の全 span は元 source に検証付きで移し、元投影の span は `SourceProvenance` に残す。解析用 source を最終出力として返さない。追加の parse と断片検索のコストは性能検証の対象に含める。

Vue language-core の実出力では、補助 receiver に加え `{label}` が `{label: __VLS_ctx.label}` になることを確認した。`vue.rs` は同名 key/value の既知の形を候補とし、共通処理が両方の対応先が同じ元 identifier の範囲内にあることを検証する。ユーザーが書いた `{label: label}` は別々の位置に対応するため省略しない。

### 全フレームワーク共通のマクロ認識

import binding の追跡、alias、shadowing、script realm の区別は全フレームワーク共通の解析責務である。名前だけの `style(...)` や同名のユーザー関数は対象にしない。framework ファイルは投影固有の参照形を宣言し、共通解析がそれを binding に対応付ける。生成コードだけでは対応が確定しない場合は converter 由来の provenance を利用し、名前から推測しない。

### Vue の例: 引数を含む式の検出・比較・原文への対応

Vue の課題は callee の `__VLS_ctx.style` を認識するだけでは終わらない。マクロの中で使う変数にも `__VLS_ctx` が付くため、投影コードの式を replacement にコピーすると、元 SFC に存在しない補助変数への参照が残る。

次は変形を説明する模式例であり、style API の具体的な引数仕様を定義するものではない。

```text
元 SFC の式:     style(active ? primary : fallback)
投影上の式:     __VLS_ctx.style(__VLS_ctx.active ? __VLS_ctx.primary : __VLS_ctx.fallback)
再利用する条件: active
誤った条件:     __VLS_ctx.active
```

動的な値、条件式、属性値、ネストしたオブジェクトや関数引数など、生成結果に残すユーザー式すべてにこの契約を適用する。call の先頭だけを正規化しても解決しない。

- 解析は投影 AST を使い、編集結果へ再利用する式は元 SFC の検証済み span から取得する。
- `vue.rs` は converter が追加した receiver/wrapper の認識規則を所有する。共通処理が map とその規則から、式全体と内部の再利用部分の元 span を確定する。
- IR は「解析用の式」と「出力に使う元の式」を区別する。lowering、guard 生成、runtime helper の引数生成などが投影 source の slice を直接コピーしないようにする。
- 正規化は文字列の一括削除ではない。文字列リテラル、property 名、ユーザーが実際に書いた同名 receiver、template ローカル変数の意味を保持する。
- 複合式は、許容された合成部分を除いた対応断片と元の式全体を照合する。投影上の文字数をそのまま元範囲の長さとして使わない。元の連続範囲と内容を確定できない形は診断する。

出力する原文は投影コードを再印字して作るのではなく、元 SFC の slice を使う。これにより、補助参照の漏出を防ぐとともに、ユーザーが書いた式の括弧・コメント・空白も保持する。

### Svelte 等への適用と疎結合の条件

Svelte のリアクティブ変数、store 参照、runes 等についても、採用する投影 converter が参照や式をどう表現するかを実 fixture で確認する。runtime compiler の変形と投影 converter の変形を混同せず、現段階では具体的な生成形や無変形を仮定しない。

その結果に必要な規則は `svelte.rs` に置く。Vue でも Svelte でも、コアは「検出した参照」「比較用の式」「検証済み元 span」を扱い、補助 receiver 名やリアクティブ変数の具体的な変形を知らない。副作用や読み取り回数を持つ wrapper を単に透明なものとして扱わず、元式へ対応付けられる条件を framework 側で明示する。

追加・変更時に、検出器・比較器・lowering・編集 renderer の各所へ framework 分岐を追加せずに対応できることを設計の受け入れ条件にする。未知の変形は adapter の明示的な非対応として扱い、コアの文字列処理で補わない。

## 5. span 検証と出力

成功した変換結果には macro の import・呼び出し・値参照を残さない。変換対象外の参照や原文対応を検証できない参照が残る場合は、import を保持して成功とせず、module 全体をコンパイルエラーにする。部分的な CSS や編集計画を出力しない。converter が生成した参照も、除外できる根拠を検証するまではこの条件の対象とする。

解析用の projected span と編集用の original span を区別し、安易に上書きしない。IR に両方の provenance を持たせ、compiler が読み取る式は編集元の source に対応させる。

編集範囲の決定:

1. framework 固有の許容変形を考慮し、元のテキストに対応する候補を決める。
2. map で開始位置を元ソースへ戻す。対象 source・領域・曖昧さを検証する。
3. 無変形の候補はテキスト長から終端を求め、元範囲と完全一致を確認する。合成 receiver 等を含む式は第4節の規則で元範囲を確定・照合する。投影式全体の長さを元範囲に流用しない。
4. 未マップ区間をまたぐ合成範囲や別 realm への対応を拒否する。内部 map の全境界一致は必須にしないが、map が連続対応を保証しない場合に GLB の開始位置だけを根拠に受理しない。
5. 全編集の範囲外・重複・衝突を検証してから、一括で適用する。

Svelte の import では排他的 end の map がずれるため、start/end を独立に map することを唯一の判定方法にしない。同一テキストが複数箇所にある場合、検索で別の箇所へ付け替えない。不一致・曖昧なマクロ編集がある module は途中まで更新せず、診断を返す。

整列後は原文・置換文字列を前から一度走査し、code と map 対応情報を同時に生成する。目標計算量はソートを除き O(入力長＋出力長＋編集数)。元 SFC を再生成・pretty print しない。

編集後 code→編集前 code、または編集後 SFC→fw_code の map を生成し、任意の上流 map と合成する。`fw2js_map` は解析・診断・編集範囲の逆対応に使い、最終 SFC の出力 map に投影 JS の位置を混ぜない。

既存 map の exact/anchor/unmapped の方針を維持する。GLB だけでなく LUB、重複 segment、行末を比較する。oxc_sourcemap 採用と host での追加合成は独立した判断とし、採用を前提にしない。

## 6. import と module CSS

マクロ import を既知の挿入・置換アンカーにする。ファイル先頭への挿入で SFC の script 外にコードを出さない。複数 script では当該マクロの realm の import を使う。安全なアンカーがなければ推測で script を新設しない。

- マクロ import の不要 specifier のみを除去する。混在する必要な import は保持する。
- runtime helper は必要なものだけを当該 realm に追加し、既存 binding との衝突を避ける。
- 明示的な module-css import は、当該 module の CSS 値を export する virtual JS module への import に置き換える。元ファイルへ巨大な CSS literal を埋め込む必要はない。
- 明示的 module-css の経路では、その module の自動 CSS side-effect import を抑制する。runtime helper の import は必要に応じて維持する。他 module の CSS 注入には影響させない。
- virtual ID の割り当てと load/HMR は host、必要 export とソース編集計画は native の責務とする。host が確定 ID を先に供給するか、構造化した import 要求を一度で確定する API とし、生成文字列の後処理を増やさない。

CSS 更新・module 削除時は virtual module と CSS contribution を無効化する。既存 session の watch 時の直前成功状態保持と build 時の失敗通知を維持する。

## 7. 実装順序と完了条件

| 段階 | 作業                                                  | 完了条件                                                                                                          |
| ---- | ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| 1    | framework 契約と6ファイル、既存分岐の移設             | React/Preact/Solid の出力回帰なし。実装の固有知識が各ファイルに閉じる                                             |
| 2    | 通常／投影入力、座標型、map adapter、provenance       | v3/Volar、UTF-8/UTF-16、曖昧な範囲の公開 API テストが通る                                                         |
| 3    | 投影の検出・比較・原文対応の共通契約と framework 実装 | Vue の補助参照と Svelte のリアクティブ式を実 converter で検証。コアへの固有分岐追加なしで検出・比較・再利用できる |
| 4    | native の編集 renderer と import 計画                 | 通常入力と元 SFC を一括編集。code と map の対応が一致                                                             |
| 5    | host converter と virtual CSS 接続                    | Astro/Svelte/Vue の実 converter→native→編集済み SFC→framework compile が通る                                      |
| 6    | bundler/HMR・性能・回帰                               | 全対象 fixture と既存 gate が成功し、制約と再実行手順が文書化される                                               |

各段階を独立してレビューできる変更単位にする。6ファイルの存在だけを6 framework 対応完了とは扱わない。能力が未検証の framework は capability により明示的に未対応とする。

全体で10〜50コミットを目安とし、設計、framework registry、map契約、投影解析、編集、host接続、各frameworkの実検証、回帰・性能をレビュー可能な機能単位で記録する。コミット数のためだけに変更を細分化しない。

## 8. 受け入れテスト

- 既存 JS/TS/JSX/TSX: class 属性、dynamic props、refs/scopes、runtime helper、CSS 出力、診断の回帰。
- SFC: script/template、複数 script、Astro client/server、Svelte module/instance、Vue normal/setup、alias、namespace import の対応可否。
- binding: ローカルの同名 style、template loop 変数、ユーザー定義 `__VLS_ctx`、computed/optional access、未知 wrapper を誤認しない。
- Vue: callee だけでなく、引数中の変数・条件式・ネストした式が runtime helper や props の生成後も元の参照を保つことを検証する。converter 由来の補助参照が残らず、ユーザー由来の同名 receiver・文字列リテラルは保持されることを確認する。
- 投影の比較: 同じ元 binding の変形された参照を認識し、別 binding は一致させない。callee・引数・ネスト式について、検出、比較、原文照合、出力を一連で検証する。
- Svelte: リアクティブ変数、store 参照、runes を採用 converter の出力で確認する。無変形の場合も記録し、runtime 出力から挙動を推測しない。
- map: 日本語・絵文字・CRLF・複数行・末尾・空範囲・重複した同一式・複数 source・未マップ・非連続・範囲外・重複編集。
- imports: script realm、既存 import との混在、helper 名衝突、明示 module-css による自動 CSS import 抑制と runtime 維持。
- output: 未編集 slice は byte 一致。変更後の SFC を実 framework compiler でビルドし、代表画面で class/style の適用も確認する。
- session: converter 設定・framework ID・map・原文変更の cache invalidation、HMR、削除、失敗からの復帰。
- 性能: 既存 source-map research の小入力と大量編集の両方で比較。N-API と host を含む時間・メモリを記録する。特定 fixture の倍率をアプリ全体の性能として扱わない。

Rust unit/integration、native public API、host/bundler、実 framework build、画面上の動作確認を別の結果として報告する。formatter、crate 境界検査、関連 check を完了させる。テストタスクは既存の Vite+ 方針に従う。

## 9. 根拠と残る検証

[SFC projection 調査](../experiments/sfc-projection-research/README.md) では Astro Go/WASM `convertToTSX` と svelte2tsx が各28/28 span、Vue language-core が script 系21/21 spanで位置と原文一致を確認した。Vue template 7件は context receiver により未対応だった。本設計の Vue 正規化はその制約を解消するための追加実装であり、既に検証済みではない。

[source-map 調査](../experiments/source-map-research/README.md) は線形 renderer の有効性と、追加 map 合成での LUB 差異を確認している。まず契約を維持して線形化し、map ライブラリ交換は切り離す。

実装前半で確定すべき事項は、Vue binding provenance の最小 metadata、Volar の区間表現、複数 realm のアンカー、virtual module の既存公開 API との整合である。これらは共通 matcher／transport の詳細として詰め、framework ごとの知識を散在させる理由にはしない。
