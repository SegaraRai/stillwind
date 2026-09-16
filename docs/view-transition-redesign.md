# view transition の再設計: class identity と name の分離

2026-09-16。`style.viewTransition()` を view-transition-class の identity として扱い、
view-transition-name を独立に与える設計。設計・実装エージェントへの引き渡し用。

## 0. 前提（実装済み。再実装しない）

- 宣言 bucket は `group`（animation option 文字列）/ `old` / `new` / `imagePair`（utility 文字列）。
  utility bucket は **media / supports 条件 variant のみ**を受け付け、それ以外は
  `InvalidViewTransitionVariant`（SHVIEW003）で拒否する。無条件 utility は1つの規則にまとめ、
  条件付き utility は base 規則の後に1規則ずつ出力する。
- `require-element-scope`（`LintKind::DocumentScopedSelector`、既定 deny）。
  要素にも宣言 identity にも紐づかない選択子（`[:host]:`、`[::view-transition-old(*)]:` など）を lint する。
  文書全体の規則は stylesheet に書く、という分担の強制装置。
- 上記2つは作業ツリーに実装済み。以下はそれらを前提に、VT の identity と名前を組み替える設計。

## 1. 現状（棚卸しの結論）

- `style.viewTransition({ group?, old?, new?, imagePair? })` が宣言。identity は
  「正規化 module path + callsite span + kind + namespace」。
- **1つの文字列**（name プールの `--sw-v-<8hex>`）が4役を兼ねる:
  (a) runtime に渡す identity、(b) `view-transition-class` の値、(c) `view-transition-name` の値、
  (d) bucket 選択子 `::view-transition-old(.<token>)` の綴り。
- use の lower: クラス `_s_<hash>`（規則は
  `--sw-vt-name:none;--sw-vt-class:none;view-transition-name:var(--sw-vt-name);view-transition-class:var(--sw-vt-class);`）
  ＋ runtime payload `stillwindViewTransitionStyle(value, key, candidates, "--sw-vt-name", "--sw-vt-class")`。
- `.for(key)` は runtime メソッド（`stillwindViewTransition` が返す）。キーは `-k-<hex codepoints>` で符号化。
- 1 call 1 VT（SHVIEW004）。VT を持つ base の合成は SHCOMP006 で拒否。
- `IdentifierKind::ViewTransitionClass` はプール（著者 CSS からの予約収集）だけを持ち、
  接頭辞も生成器も無い（`builtin_prefix(ViewTransitionClass) == ""`、`ViewTransitionClassRequest` なし）。
  napi のホスト生成器は name 側のみ。

歪み: name（文書一意）と class（分類）の役割混同、class 側衝突検査の空洞、
ホスト改名の非対称、複数クラス・著者名の受け皿が無い、`--sw-vt-*` が「上書き前提」の構造になっている。

## 2. 決定事項

1. **identity はクラス**（A案: 1トークン）。クラスファミリがトークンを生成し、
   名前はそこから派生する。綴りは現行の `--sw-v-<8hex>` のまま（所有権だけが変わる）。
2. **宣言値は `{v:"<token>"}`** に lower する（既存の `{c:"s_…"}` マーカーと同じ系）。
   `stillwindViewTransition()` は廃止し、宣言は plain object literal にする。
3. **view transition class のスタイルは `style.viewTransition()` のみ**が持つ。
   use 側の identity 束縛 variant（`pageTitle.old` 等）は作らない。
   文書全体の規則（`(*)`、root、`@view-transition`、`:active-view-transition-type()`）は stylesheet。
4. **名前は `.for(...)` か明示宣言だけが与える**。素の `vt` はクラスのみ（参加しない）。
   明示宣言 = `[view-transition-name:…]`、`view-transition-name` を宣言するユーザー `@utility`、
   `--*` 値経由のもの、など。
5. **`.for` はマクロ**（コンパイラが畳み込む構文）。runtime にメソッドを置かない。
   `.for()`（0引数）と `.for("")` を許容し、末尾 `-` の名前をそのまま許容する。
   特別扱いしない。
6. **名前 = `serializeIdentifier(token + "-" + String(key))`**。区切りは `-` のみで `-k-` は使わない。
   ホスト提供 generator が `-` を含むトークンを返す場合の曖昧さはホスト責務（契約に明記）。
7. **`view-transition-name` は宣言に従属しない free-form な pairing key**。異なる identity
   （別の宣言・別のコンポーネント）が同じ値を使ってよく、それが cross-component の pairing 手段になる。
   コンパイラは名前の値について予約・一意性・宣言との対応を**検査しない**（§3.4）。
8. **`--sw-vt-name` / `--sw-vt-class` は廃止**。直接 CSS（静的）か inline（動的名）を出す。
9. **name プールは削除**。著者の `view-transition-name` 値の収集・予約も行わない（§3.8）。
10. **lint は2本**（名前の供給源の欠落 / 重複）。既定 warn。値の検査はしない。SHVIEW004 は削除。
11. **エスケープは共有実装**。Rust は既存の `cssparser::serialize_identifier`（`name.rs` の
    `escaped_identifier`）。JS は `CSS.escape` があればそれ、無ければ spec 準拠のフォールバック。
12. **`group` の option 条件は option 単位**（候補A、§3.9）。条件付きの option は同じ選択子の
    規則として無条件の規則より後ろに並べ、プロパティ単位の差分を作る。keyframes 型の定義分岐は
    使わない。

## 3. 仕様

### 3.1 宣言と値

```tsx
const vt = style.viewTransition({ old: "opacity-0", new: "opacity-100" });
// ⇒ const vt = {v:"--sw-v-3f2a91cd"};
```

`group`/`old`/`new`/`imagePair` の少なくとも1つを要求する現行契約は維持する
（bucket 無しの宣言は意味を持たない。参加は名前が与えるものになったため）。

### 3.2 use の lower 行列

| identity | `.for`   | 出力                                                                      |
| -------- | -------- | ------------------------------------------------------------------------- |
| 静的     | なし     | クラスのみ（CSS）。名前は他の宣言が与える                                 |
| 静的     | リテラル | CSS: `view-transition-name: <escaped>; view-transition-class: <token>;`   |
| 静的     | 動的     | CSS（class）＋ inline（name = `stillwindViewTransitionName(token, key)`） |
| 動的     | なし     | 候補クラス表（各クラス規則は class のみ）                                 |
| 動的     | リテラル | 候補クラス表（各候補の規則が自分の名前を静的に持つ）                      |
| 動的     | 動的     | 候補クラス表 ＋ inline（helper には値 `{v}` を渡す）                      |

```tsx
// 静的・無キー: クラスのみ
<article {...style("rounded-xl", vt)} />
// ⇒ <article className="_s_ab12" />
```

```css
._s_ab12 {
  view-transition-class: --sw-v-3f2a91cd;
}
```

```tsx
// 静的・リテラルキー: 完全に CSS
<article {...style("rounded-xl", vt.for("bar"))} />
// ⇒ <article className="_s_cd34" />
```

```css
._s_cd34 {
  view-transition-name: --sw-v-3f2a91cd-bar;
  view-transition-class: --sw-v-3f2a91cd;
}
/* vt.for("") なら view-transition-name: --sw-v-3f2a91cd-; （末尾の - は valid な ident） */
```

```tsx
// 静的 identity・動的キー: 名前だけ inline
<article {...style("rounded-xl", vt.for(product.id))} />
// ⇒ <article className="_s_ef56"
//      style={{ "view-transition-name": stillwindViewTransitionName("--sw-v-3f2a91cd", product.id) }} />
```

```css
._s_ef56 {
  view-transition-class: --sw-v-3f2a91cd;
}
```

### 3.3 動的 identity のクラス表

identity が実行時に決まる場合は、候補ごとにクラス規則を出し、class 属性を表引きで選ぶ
（既存の ref マーカー `({"s_…":"_s_…"})[value.c]` と同じ形）。

```tsx
const t = compact ? vtA : vtB; // {v:"--sw-v-a"} | {v:"--sw-v-b"}
<article {...style("rounded-xl", t.for("hero"))} />;
// ⇒ <article className={stillwindAssetClass(t, {
//        "--sw-v-a": "_s_1",   /* ._s_1 { view-transition-name: --sw-v-a-hero; view-transition-class: --sw-v-a; } */
//        "--sw-v-b": "_s_2",   /* ._s_2 { view-transition-name: --sw-v-b-hero; view-transition-class: --sw-v-b; } */
//      })} />
```

リテラルキーの動的 identity はこの形で **inline 不要**。動的キーが絡む場合のみ
`stillwindViewTransitionName(<value>, <key>)` を inline で使う（helper は `{v}` から token を読む）。

### 3.4 名前の与え方と lint

名前を与えるのは次の2系統のみ。

- `.for(...)` を付けた適用（その適用が名前を与える）
- その call の解決済み宣言に `view-transition-name` が現れるもの
  （`[view-transition-name:…]`、ユーザー `@utility`、`--*` 値を参照する任意プロパティなど）

名前は宣言や class に従属しない **pairing key** であり、異なる identity が同じ値を共有してよい。
たとえば遷移前を A コンポーネント、遷移後を B コンポーネントが描く場合、両者が同じリテラル名
（`[view-transition-name:hero]`）を書けば名前で pairing し、class はそれぞれの宣言のスタイルを当てる
（old 側は A のクラス、new 側は B のクラス）。したがってコンパイラは名前の値について
予約・一意性・宣言との対応を検査しない。以下の lint は**供給源の数**だけを見る。

lint 2本（既定 warn、host policy で変更可）。

| 規則       | 条件                                                               |
| ---------- | ------------------------------------------------------------------ |
| 名前の欠落 | その call に VT クラスがあり、名前の供給源が0                      |
| 名前の重複 | 名前の供給源が2つ以上（`.for` 2つ、`.for` と明示宣言の併存、など） |

判定材料は IR から構造的に得られる（`.for` の有無と、解決済み宣言中の
`view-transition-name` プロパティ）。別 call・グローバル CSS・ユーザーの inline style が
名前を与える場合は見えないため、error ではなく warn に留める。

`style(vt)` は「名前の欠落」に当たる。参加したい場合は `vt.for()`（末尾 `-` を許容する最短形）
または明示宣言を書く。`[view-transition-name:none]` は opt-out としてそのまま機能し、
lint は名前の供給源として数える（警告を出さない）。

複数指定の実際の勝敗は cascade 依存（動的名の inline が勝つ）なので、
「明示名は同じ call に1つだけ書く」を契約として文書化する。

### 3.5 複数適用

クラスは union、`view-transition-name` は1要素につき1値（供給源が2つ以上なら lint 対象）。
`view-transition-class` は単一プロパティなので、
**同一 call 内の全クラスを1つの宣言に列挙**する。

```tsx
style(vt1, vt2, "[view-transition-name:foo]");
// className="_s_x _s_ff00"
// ._s_x    { view-transition-class: --sw-v-a --sw-v-b; }
// ._s_ff00 { view-transition-name: foo; }

style(vtMain.for("bar"), vtSub);
// ._s_z { view-transition-name: --sw-v-main-bar; view-transition-class: --sw-v-main --sw-v-sub; }
```

条件ユニット（`cond && vtSub`）では union を表現できないため、
各条件ユニットが自分のクラスに専用のカスタムプロパティを書き、**要素のクラスリスト側が
`var(--slot,)` を並べる**方式を実装で採用した（実装後の改訂）。解決後のリストは要素が
宣言した slot の union になり、同時に成立した条件が互いを消さず、条件の入れ子も自然に扱える。

```tsx
style("rounded", cond && vtSub, vtMain.for("bar"));
// base unit:        view-transition-class: --sw-v-main var(--sw-vtc-…,);
// conditional unit: --sw-vtc-…: --sw-v-sub;
```

class unit を超えたマージは未対応のままとする。合成（`style(base, …)`）の base が遷移クラス
**だけ**を宣言している場合は合成でき（クラスは普通のクラス寄与として1つの payload に乗る）、
base が実行時に組む名前を持つ場合は従来どおり prop を持つ base として拒否する。base と自 call が
別々の遷移クラスを宣言した場合は、`view-transition-class` が単一プロパティなので後から来る
derived の規則が base のリストを置き換える（slot を call 横断で共有する案は未対応）。この
置き換えは `SplitViewTransitionClasses`（`single-view-transition-class`、既定 warn）が知らせる。

### 3.6 `.for` の仕様（マクロ）

- 引数は0個または1個。2個以上は診断（現行 SHVIEW002 のメッセージを
  「0個または1個」に緩める）。
- 引数が無い場合と `""` は同じ結果（`token-`）。
- 引数がリテラルなら、コンパイル時に連結とエスケープを確定して CSS に出す。
- 引数が動的なら、runtime helper を呼ぶ式に lower する。

導出は宣言ごとに一意な名前を得るための便宜であり、名前と宣言の対応関係を意味しない。
別の宣言が同じ値を書くことも、リテラル名で別コンポーネントと pairing することも自由。

- 宣言の外（`export const keyed = vt.for(id)` 等）では、リテラルなら `{v:"…", k:"…"}`、
  動的なら `stillwindViewTransitionKey(vt, id)` 呼び出しに書き換える
  （runtime に `.for` メソッドを置かない）。

### 3.7 エスケープ

- 仕様は CSS Syntax の "serialize an identifier"（CSSOM の `CSS.escape` と同じアルゴリズム）:
  NUL → U+FFFD、制御文字は `\hex `、先頭が数字、2文字目が数字で先頭 `-`、単独の `-`、
  `[A-Za-z0-9_-]` と U+0080 以上はそのまま、それ以外は `\c`。
  トークンは `--` で始まるため実際に効くのは key 側（空白・記号・制御文字）。
- Rust: `crates/compiler/src/identifiers/name.rs` の `escaped_identifier`
  （`cssparser::serialize_identifier`）をそのまま使う。
- JS: `packages/core/src/runtime/shared.ts` に同アルゴリズムの純関数を置く。
  `CSS.escape` が存在すればそれを使い、無ければフォールバック。
  **両者が一致することをテストで確認**する（ブラウザ実行のテストに1ケース、加えて共有ベクタ）。
- 共有ベクタ: `(token, key) → name` の表を fixture（例 `fixtures/view-transitions/name-escapes.json`）に置き、
  Rust テストと TS テストの両方が読む。key に空白・引用符・バックスラッシュ・先頭数字・CJK・絵文字を含める。
- コンパイル時に確定した名前を JS リテラルへ埋める場合は、エスケープ済み文字列の
  バックスラッシュをさらに JS 文字列として直列化する（`"--sw-v-x\\-y"`）。ここもテスト対象。

### 3.8 識別子プール

| 対象                                                                                     | 変更                                                                                                                             |
| ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `IdentifierKind::ViewTransitionClass`                                                    | 接頭辞（既定 `--sw-v-`）・digest 8・`ViewTransitionClassRequest`・`view_transition_class()` を追加。ホスト生成器の対応表にも追加 |
| `IdentifierKind::ViewTransitionName` / `ViewTransitionNameRequest` / napi の name 生成器 | 削除                                                                                                                             |
| `IdentifierPool::ViewTransitionName` / `NamePool::ViewTransition`                        | 削除                                                                                                                             |
| `NamePool::ViewTransitionClass`（著者 CSS からの予約収集）                               | 維持。クラストークン生成時の衝突検査に使う                                                                                       |
| 著者の `view-transition-name` 値の収集・予約                                             | 行わない。名前は identity 間で共有されうる値であり、生成側の検査対象にしない（§3.4）                                             |

### 3.9 `group` の option 条件

`group` の option にも media / supports 条件を書ける。条件は option 文字列の中に variant の形で
書き、条件付きの option は同じ選択子の規則として、無条件の option の規則より後ろに出力する
（候補A: option 単位）。

```tsx
style.viewTransition({
  group: "duration-180 ease-linear both motion-reduce:duration-0",
  old: "opacity-0",
  new: "opacity-100",
});
```

```css
::view-transition-group(.--sw-v-a9ce35ce) {
  animation-duration: calc(180 * 1ms);
  animation-timing-function: linear;
  animation-fill-mode: both;
}
@media (prefers-reduced-motion: reduce) {
  ::view-transition-group(.--sw-v-a9ce35ce) {
    animation-duration: calc(0 * 1ms);
  }
}
```

無条件の option は1つの規則にまとめる。条件付きの option は書いた順に1規則ずつ、base と同じ
選択子のまま並べる。複数の条件を重ねた option は wrapper を外側へ積み、同時成立した条件は
後ろの規則が勝つ。特異性が同じで後勝ちになるため、条件は option 列の**プロパティ単位の差分**
になる（この例で条件成立時に変わるのは `animation-duration` だけで、`ease-linear` と `both` は
そのまま効く）。

`old` / `new` / `imagePair` の条件規則と選んだ形が同じで、4つの bucket はいずれも宣言に紐づく
規則である。規則は要素の有無と無関係に出力されるため、条件も要素側の状態ではなく宣言側の
wrapper で表すことになる。§3.5 の条件ユニットが要素側のクラスリストで union を解くのとは経路が
別で、group には要素が介在しないため union を解く必要がない。

自然な理由:

- `::view-transition-*` の擬似要素は UA が文書 root の下に生成するもので、装飾した要素の子孫
  にはならない。`&` を使う variant はこの規則に届かず、規則と要素を結ぶ手掛かりは
  `view-transition-name` / `view-transition-class` のトークンだけになる。条件を選択子の中へ
  持ち込む手段が無く、規則の外側に置く形が唯一の合成手段になる。
- 一方、規則そのものは通常の style rule（選択子＋宣言）で、`animation-duration` などの宣言を
  書く。`@media` / `@supports` の中の規則は cascade で普通の要素規則と同じに解決されるので、
  同じ選択子の規則を後ろに足せばプロパティ単位の差分が得られる。定義規則に条件を許す判定
  （`conditions_only`）が通す media / supports が、そのままこの形に載る。
- `@keyframes` は選択子も入れ子規則も持たないグローバルな at-rule で、条件は定義全体を包む形
  しかない。同名定義は部分マージされないため、条件ごとに完全な定義を作る D2 の分岐が要る
  （`docs/design-intent-improvement-plan.md` の「animation は定義 object の variant key で
  分岐する」）。group は option 列の差分で足りるので、この形を持ち込まない。条件で option 列を
  丸ごと置き換える要求が出た場合は、そのとき定義分岐（候補B）を検討する。

制約:

- 条件は media / supports のみ。要素依存の選択子、`@container`、任意 at-rule を option に
  書いた場合は `InvalidViewTransitionVariant`（SHVIEW003）で拒否する（他の3 bucket と揃える）。
- 負値と `!important` は拒否したまま（option は値の記述で、修飾付き utility ではない）。
  option 自体の文法の誤り（未知の名前、負値、`!important`）は現行どおり
  `UnknownAnimationOption`（SHANIMATION002）、variant の位置の誤りは bucket 側の
  `InvalidViewTransitionVariant`（SHVIEW003）と使い分ける。
- `[transition-duration:75ms]` のようなプロパティ投影も同じ経路に載せる。条件を剥がして解決し、
  投影した宣言を同じ wrapper に置く（投影が対象外プロパティを落とす規則は変えない）。
- 実装箇所は `crates/compiler/src/compiler/definitions/view_transition.rs` の
  `emit_view_transition_definition` と `crates/compiler/src/compiler/animation_options.rs` の
  `resolve_options`（option ごとに条件と宣言を分け、bucket 側と同じ wrapper を組む）。
  `animation(options)` は条件を受け付けないため、variant を持つ option は従来どおり
  `UnknownAnimationOption` のままとする。

## 4. 実装計画

依存順に5フェーズ。各フェーズで workspace のゲートを通す。

**Phase 1: 識別子ファミリの一本化**（出力は変えない）

- `crates/compiler/src/identifiers/{kind,request,generator,name,mod}.rs`
- `crates/napi/src/naming.rs`（name 生成器を削除し class 生成器を追加）、`packages/native/binding.d.ts`
- `crates/project/src/reserved.rs`（name プール削除、class プール維持）
- 綴りは不変なので fixture は通るはず。通らなければ綴りの差分を洗い出す。

**Phase 2: 値の形と `.for` マクロ**

- `crates/js-oxc/src/view_transition.rs`（`.for` の 0/1 引数、式 vs リテラルの判定）
- `crates/ir/src/model.rs`（`StyleInput::ViewTransitionUse` の key を任意化、
  宣言値の形は emission 側の表現）
- `crates/compiler/src/compiler/emission/`（`{v}` literal、静的 CSS、
  `stillwindViewTransitionName` 呼び出し、`--sw-vt-*` と `stillwindViewTransitionStyle` の削除）
- `crates/diagnostics/src/compilation.rs`（SHVIEW002 の緩和、SHVIEW004 の削除）
- `packages/core/src/runtime/shared.ts`（helper の入れ替え）、`packages/core/src/index.ts`（`for(key?)`）

**Phase 3: 動的 identity**

- 候補クラス表（`stillwindAssetClass` の `{v}` 対応）と、候補クラス規則の生成
- リテラルキーは候補クラス側に静的名を持たせる

**Phase 4: lint**

- `crates/diagnostics/src/lint.rs` に2種、既定 warn を `crates/design-system/src/theme/config.rs` に登録
- 判定は call 単位（emission の解決結果を利用）。ルール名は実装時に決める
  （例 `require-view-transition-name` / `single-view-transition-name`）

**Phase 5: fixture とドキュメント**

- `fixtures/projects/view-transition-matrix` を static/dynamic × key の行列、明示名併用、
  複数適用、条件ユニットまで拡張して refresh
- エスケープ共有ベクタの新設
- 更新対象: `docs/macro-contract.md`、`docs/golden-case.md`、
  `docs/generated-identifiers.md`（E/F 行と「class は生成関数を持たない」記述）、
  `docs/custom-properties.md`（`--sw-vt-*` の記述）、`docs/css-ordering.md`（クラス union と条件ユニット）、
  `docs/review-fixtures.md`、`docs/design-intent-improvement-plan.md` §8

## 5. 検証計画

- Rust: `cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`、`cargo fmt --all --check`。
- fixture: `vp run fixtures:refresh` → `vp run fixtures:check`。
- TS: `vp run @stillwind/core#check`、`@stillwind/host#test`、`@stillwind/vite#test`、
  `vp run fmt`、`vp run check`。
- 断言すべき契約:
  - 静的ケースは runtime import も style prop も出さない（純 CSS）。
  - 動的キーは `view-transition-name` だけが inline に出る。
  - 動的 identity は候補クラス表になり、リテラルキーなら inline が出ない。
  - `view-transition-class` の複数リストが条件ユニットで完全列挙される。
  - `.for()` / `.for("")` の名前が末尾 `-` になる。
  - エスケープ: Rust と TS が同一の名前を返す（共有ベクタ）。
  - lint 2本の検出と、`[view-transition-name:none]` が警告を出さないこと。
  - SSR（文字列 style）で `view-transition-name:<escaped>` が出る。
- ブラウザ確認: 現状 VT 用の `verify:*` スクリプトは experiments に無い。
  動的キー・動的 identity・複数適用の実挙動を確認するには新設が要る（Chrome で
  `::view-transition-group(.class)` が期待どおり当たること、名前の一意性が保たれること）。

## 6. 未決・注意点

- lint のルール名と既定 severity の最終決定（本設計は warn を推奨）。
- 条件ユニットでのクラス完全列挙は実装規則として明文化する（表 3.5）。
- ホスト generator の契約に「`-` を含むトークンを返さない」を明記する
  （`token + "-" + key` の単射性はホスト責務）。
- `view-transition-name` の値は検査しない（予約・一意性・宣言との対応のいずれも対象外）。異なる identity が同じ名前を使うのは正当な pairing 手段で、capture 時の一意性は文書側の責務。
- 別 call・グローバル CSS が名前を与える場合、lint の欠落警告は誤検知になり得る（warn に留める理由）。
- 動的名の inline は cascade 上いちばん強い。同一 call で明示名と併存した場合は lint が知らせるが、
  実際の勝敗は cascade 依存であることをドキュメントに書く。
- レイヤによる優先順位制御（生成側の添付宣言を低レイヤに置く案）は**未採用**。
  明示名を同一 call に書く運用で足りるため。
- class を著者が指定する API は無い。任意プロパティ経路（`[view-transition-class:…]`）は他の
  プロパティと同じく開いており、生成トークンとの衝突検査が走査するのは著者 CSS（design system と
  host が producer 用に渡した CSS）の予約収集だけである。

## 7. 却下した案（再検討しない）

- use 側の identity 束縛 variant（`pageTitle.old`）: 書いた要素がスコープにならず、
  効果が宣言の全 participant に及ぶため。スタイルは宣言が持つ。
- wildcard/global の第一級 variant（`vt-any:` 等）: 文書全体の規則は stylesheet に書く
  （`require-element-scope` が同じ原則を強制している）。
- 名前専用マクロ（`style.viewTransitionName(...)`）: `.for()` と `[view-transition-name:…]` で足りる。
- 匿名クラス・自動名: 参加は名前が与えるものになり、自動名は `.for()` が担う。
  生成クラスは著者から狙えず styleable でないため、匿名クラスの用途が残らない。
- 2トークン（name と class を独立生成）: A案（1トークン）を採用。
- `-k-` マーカーや長さ前置による単射性担保: ホスト責務とした。
- `--sw-vt-name` / `--sw-vt-class` の indirection: 上書き前提の構造なので削除。
- 名前値の予約・一意性検査: name は異なる identity が共有してよい pairing key で、定義との主従関係が無いため。
