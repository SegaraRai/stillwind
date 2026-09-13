# Rustコメント修正の全差分

2026-09-12監査開始時点と最終状態の比較。`-`が監査前、`+`が監査後。既存のユーザー変更を基準にしている。

## [crates/cli/src/arguments.rs](../../crates/cli/src/arguments.rs)

```diff
--- before/crates/cli/src/arguments.rs
+++ after/crates/cli/src/arguments.rs
@@ -2,5 +2,6 @@
 //!
 //! Arguments are matched by exact name, so a flag never depends on its position and an
-//! unknown argument is ignored rather than mistaken for a value. Only the command word
+//! unknown argument is skipped during name lookup. A value lookup accepts the next
+//! argument verbatim, even if it looks like another flag. Only the command word
 //! is positional. Nothing here parses `--name=value`; the CLI uses the separated form
 //! throughout.
```

## [crates/cli/src/loading.rs](../../crates/cli/src/loading.rs)

```diff
--- before/crates/cli/src/loading.rs
+++ after/crates/cli/src/loading.rs
@@ -26,5 +26,5 @@
     #[error("{0}")]
     Source(#[from] SourceError),
-    /// The file exists but could not be read.
+    /// The file could not be read, including when it does not exist.
     #[error("cannot read {}: {source}", .path.display())]
     Read {
```

## [crates/cli/src/main.rs](../../crates/cli/src/main.rs)

```diff
--- before/crates/cli/src/main.rs
+++ after/crates/cli/src/main.rs
@@ -1,6 +1,5 @@
 //! Bundler-facing command line adapter for the Stillwind compiler.
 //!
-//! The CLI is the only place in the Rust workspace that touches the filesystem and
-//! package resolution: it reads the source and theme files, resolves CSS imports
+//! The CLI owns filesystem and package resolution for command-line compilation: it reads the source and theme files, resolves CSS imports
 //! through `oxc-resolver`, and rebases resource URLs relative to the entry. Everything
 //! downstream is composed from the library crates in the same order a host would use
```

## [crates/compiler/src/compiler/emission.rs](../../crates/compiler/src/compiler/emission.rs)

```diff
--- before/crates/compiler/src/compiler/emission.rs
+++ after/crates/compiler/src/compiler/emission.rs
@@ -67,5 +67,5 @@
     runtime_assets: &'a mut BTreeSet<IdentityId>,
     runtime: &'a mut RuntimeBindings<'b>,
-    /// Canonical content of the base unit, in source order.
+    /// Canonical candidate and asset spelling for the base unit, in source order.
     static_canonical: String,
     /// Utilities that will form the base unit's rule.
@@ -814,5 +814,5 @@
 #[derive(Default)]
 struct LiteralContribution {
-    /// Canonical rule content of the literal, in source order.
+    /// Canonical candidate and wrapper spelling of the literal, in source order.
     canonical: String,
     /// Utilities to emit as this leaf's rule.
```

## [crates/compiler/src/compiler/handles.rs](../../crates/compiler/src/compiler/handles.rs)

```diff
--- before/crates/compiler/src/compiler/handles.rs
+++ after/crates/compiler/src/compiler/handles.rs
@@ -2,12 +2,12 @@
 //!
 //! Marker class names and anchor custom-property names are hashes of the declaration's stable
-//! identity, so a declaration keeps its name across recompiles and two declarations never
-//! share one. A declaration's identity is independent of its projections: `anchor`,
+//! identity, so unchanged declarations keep their names across recompiles. Truncated
+//! hashes may collide; symbol validation reports conflicting identities. A declaration's identity is independent of its projections: `anchor`,
 //! `position`, `scope`, and `container` all reference the same generated name, which is what
 //! makes anchor positioning resolve to the element the ref was attached to.
 //!
-//! [`summary_runtime_assets`] decides which assets the module actually references. A definition
-//! nothing references is erased to `undefined` in the output, so an unused declaration does
-//! not keep a runtime call, nor CSS, alive.
+//! [`summary_runtime_assets`] seeds runtime reachability from module exports; call
+//! emission adds uses. Unreferenced producer calls are replaced by `undefined`, but
+//! animation and view-transition definition CSS is emitted independently of that set.

 use std::collections::BTreeSet;
```

## [crates/compiler/src/compiler/output.rs](../../crates/compiler/src/compiler/output.rs)

```diff
--- before/crates/compiler/src/compiler/output.rs
+++ after/crates/compiler/src/compiler/output.rs
@@ -23,7 +23,7 @@
 /// Everything one compilation produced.
 ///
-/// The parts are consistent with each other: every entry in `classes` and `symbols` describes
-/// a rule inside `module_css`, and `transforms` are non-overlapping replacements of the
-/// original source. All of them are empty when `diagnostics` is not.
+/// Successful pipeline output contains CSS, generated identifiers and source edits.
+/// Marker classes and asset symbols need not own a CSS rule. Failed pipeline output
+/// clears generated output; public construction does not enforce that relationship.
 #[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
 pub struct CompileResult {
```

## [crates/compiler/src/compiler/style_variants.rs](../../crates/compiler/src/compiler/style_variants.rs)

```diff
--- before/crates/compiler/src/compiler/style_variants.rs
+++ after/crates/compiler/src/compiler/style_variants.rs
@@ -10,5 +10,6 @@
 //! [`StyleVariantPlan::canonical`] is the identity spelling used for class hashing. It records
 //! the ref's generated identifiers and the authored prefix, independent of how the source spelled
-//! them, so two sources that mean the same thing generate the same class name.
+//! them. This is one part of class identity; module, callsite, and branch information
+//! also participate, so equivalent wrappers alone do not guarantee equal class names.

 use std::fmt::Write as _;
```

## [crates/compiler/src/compiler/stylesheet/properties.rs](../../crates/compiler/src/compiler/stylesheet/properties.rs)

```diff
--- before/crates/compiler/src/compiler/stylesheet/properties.rs
+++ after/crates/compiler/src/compiler/stylesheet/properties.rs
@@ -3,6 +3,6 @@
 //! Registration is driven by usage: a definition is emitted when the CSS declares or references
 //! its name, or when one of its activating properties appears. Activation pulls in each
-//! definition's dependencies and group members up to a fixed point, so an emitted registration
-//! never references a property that is not registered too. An authored `@property` rule wins over
+//! definition's dependencies and group members up to a fixed point. Only registered
+//! definitions can be emitted; a missing dependency name has no definition to emit. An authored `@property` rule wins over
 //! a built-in of the same name, and built-in definitions are emitted through the compatibility
 //! wrapper the built-in stylesheet uses, so a browser that lacks the at-rule still gets fallback
@@ -24,6 +24,6 @@
 ///
 /// # Errors
-/// Returns [`PropertyUsageError`] when the CSS uses a registered property in a way its syntax does
-/// not allow.
+/// Returns [`PropertyUsageError`] when CSS usage cannot be parsed. Values are not
+/// checked against the registered `@property` syntax descriptor.
 #[allow(clippy::too_many_lines)]
 pub(in crate::compiler) fn render_used_properties(
```

## [crates/compiler/src/compiler/stylesheet/render.rs](../../crates/compiler/src/compiler/stylesheet/render.rs)

```diff
--- before/crates/compiler/src/compiler/stylesheet/render.rs
+++ after/crates/compiler/src/compiler/stylesheet/render.rs
@@ -3,5 +3,5 @@
 //! Both entry points delegate to the output template; the only difference is whether authored
 //! document CSS and Preflight participate. The error set is deliberately about assembly - a
-//! missing utilities slot, an unusable `--theme()` reference, or an invalid `@property` usage -
+//! missing utilities slot, an unusable `--theme()` reference, or CSS usage parsing failure -
 //! rather than about candidate resolution, which reports through diagnostics elsewhere.

@@ -20,5 +20,5 @@
     #[error("generated utilities require an @stillwind-slot utilities; in the CSS entry graph")]
     MissingUtilitiesSlot,
-    /// A generated or authored `@property` usage is invalid for its registered syntax.
+    /// CSS could not be parsed while collecting custom-property usage.
     #[error("{0}")]
     PropertyUsage(#[from] PropertyUsageError),
```

## [crates/compiler/src/compiler/stylesheet/template.rs](../../crates/compiler/src/compiler/stylesheet/template.rs)

```diff
--- before/crates/compiler/src/compiler/stylesheet/template.rs
+++ after/crates/compiler/src/compiler/stylesheet/template.rs
@@ -55,5 +55,5 @@
 /// # Errors
 /// Returns [`StylesheetError`] for unusable authored CSS, a missing utilities slot, or invalid
-/// `@property` usage.
+/// CSS usage parsing.
 pub(crate) fn render_mapped(
     template: &CssOutputTemplate,
```

## [crates/compiler/src/compiler/symbols.rs](../../crates/compiler/src/compiler/symbols.rs)

```diff
--- before/crates/compiler/src/compiler/symbols.rs
+++ after/crates/compiler/src/compiler/symbols.rs
@@ -221,5 +221,5 @@
 ///
 /// The identity keeps the whole canonical input - namespace, module, callsite, branch, and rule
-/// content - rather than the truncated name, so two producers can prove they generated the same
+/// spelling - rather than the truncated name, so two producers can prove they generated the same
 /// class or detect that they generated different ones with equal names.
 pub(super) fn generated_class_symbol(
```

## [crates/compiler/src/module_compiler.rs](../../crates/compiler/src/module_compiler.rs)

````diff
--- before/crates/compiler/src/module_compiler.rs
+++ after/crates/compiler/src/module_compiler.rs
@@ -46,8 +46,7 @@
 /// Content identity of one generated CSS rule.
 ///
-/// Two rules with equal keys have byte-identical serialization, which is what lets one
-/// producer hand a rule to another without re-verifying it. The doctest pins that no
-/// conversion from a bare [`Fingerprint`] exists, so a rule cannot be keyed by an
-/// unrelated hash by accident.
+/// Compiler-produced keys hash the rule serialization. The project layer trusts
+/// that correspondence when sharing rules. Public tuple construction can supply an
+/// unrelated fingerprint; the doctest only rules out an implicit conversion.
 ///
 /// ```compile_fail
@@ -98,6 +97,7 @@
 ///
 /// The compiler cannot know where a file lives, so each reference becomes a unique
-/// placeholder string and the host rewrites it later. Two references never share a
-/// placeholder, even when they name the same URL from different modules.
+/// placeholder string and the host rewrites it later. Repeated references with the
+/// same owner and decoded specifier share a placeholder; different owners are hashed
+/// separately.
 #[derive(Clone, Debug, Eq, PartialEq, Serialize)]
 pub struct ModuleResourceReference {
@@ -106,5 +106,5 @@
     /// Source path of the module whose CSS contained the reference.
     pub owner: String,
-    /// The authored URL, exactly as it was written in the CSS.
+    /// Decoded URL specifier, without the surrounding CSS syntax.
     pub specifier: String,
 }
@@ -116,5 +116,5 @@
     }

-    /// Lossless boundary between the existing emitter and rule ownership. CSS
+    /// Splits emitted CSS into independently owned rules without changing its bytes. CSS
     /// tokenization keeps braces inside strings, functions and comments intact.
     ///
@@ -189,5 +189,5 @@
     /// Identity of the source file, which may differ from the module id after projection.
     pub source_file_id: u32,
-    /// Which frontend produced the analysis, for example `react` or `svelte`.
+    /// Which frontend produced the analysis, for example `oxc`.
     pub frontend_kind: String,
     /// Adapter mode the analysis was extracted under.
@@ -209,9 +209,9 @@
     /// Theme token names whose values appear in the module's generated CSS.
     pub theme_tokens: BTreeSet<String>,
-    /// Every utility definition fingerprint, since resolving any candidate may use one.
+    /// All currently registered utility definition names, recorded conservatively.
     pub utilities: BTreeSet<String>,
-    /// Every variant definition fingerprint, for the same reason.
+    /// All currently registered variant definition names, recorded conservatively.
     pub variants: BTreeSet<String>,
-    /// Every custom-property definition fingerprint.
+    /// All currently registered custom-property definition names.
     pub properties: BTreeSet<String>,
     /// Identities of the assets the module used.
@@ -222,6 +222,7 @@
 ///
 /// This is the compiler-to-project boundary. The fields are produced together, so a
-/// consumer may rely on them agreeing: for example, every class in `classes` names a rule
-/// in `css`, and every transform in `code_transforms` is non-overlapping with the others.
+/// consumer receives generated classes and symbols alongside CSS and source edits.
+/// Marker classes need not have a CSS rule. Transform validity is checked when the
+/// edits are rendered.
 /// They are only meaningful when the module compiled without diagnostics.
 #[derive(Clone, Debug)]
@@ -369,6 +370,6 @@
     /// Compiles the module together with the support modules its assets came from.
     ///
-    /// Support modules contribute CSS and symbols but never source transforms of their own:
-    /// their declarations are lowered in the importing module's file.
+    /// Support modules contribute declaration CSS and symbols, with resources owned
+    /// by their own source paths. Their source transforms are not returned here.
     fn compile_support_closure(&self, module: &ModuleAnalysis) -> CompileResult {
         let mut result = Compiler::compile_with_snapshot(
````

## [crates/compiler/src/options.rs](../../crates/compiler/src/options.rs)

```diff
--- before/crates/compiler/src/options.rs
+++ after/crates/compiler/src/options.rs
@@ -30,5 +30,5 @@
     pub tailwind_prefix: String,
     /// When set, `group`/`peer`-style relationship candidates are reported through the
-    /// `prefer-typed-relationships` lint instead of compiling.
+    /// `prefer-typed-relationships` lint. Candidate compilation still proceeds.
     pub forbid_tailwind_relationships: bool,
     /// Extra namespace mixed into class identity, so two independently built projects
@@ -52,7 +52,7 @@
     /// call and one branch, never to shared content.
     ///
-    /// `call` is the call-site byte offset and `branch` is the emission-local class
-    /// index, which is `0` for the base unit and increments per conditional leaf. The
-    /// canonical rule content is appended by the caller, so the same utilities written
+    /// `call` is the call-site byte offset. `branch` contains the caller's base/leaf
+    /// discriminator and canonical candidate/asset spelling, not the resolved CSS
+    /// declaration bytes. The same utilities written
     /// in two branches still hash to different names. Length prefixes keep the
     /// variable-length namespace and module boundaries unambiguous, and the whole
```

## [crates/compiler/src/query.rs](../../crates/compiler/src/query.rs)

```diff
--- before/crates/compiler/src/query.rs
+++ after/crates/compiler/src/query.rs
@@ -30,6 +30,6 @@
 /// Everything one candidate produces for an editor.
 ///
-/// `generated_rules` is empty exactly when `diagnostics` is not, so a caller can use either
-/// as the validity check.
+/// Use `diagnostics` as the validity check. The rules are a candidate preview,
+/// without the document's theme and property-support closure.
 #[derive(Clone, Debug)]
 pub struct CandidateResolution {
@@ -40,5 +40,6 @@
     /// Rendered rules for the candidate, keyed by their own serialization.
     pub generated_rules: Vec<GeneratedRule>,
-    /// Custom properties the rendered CSS uses and that should be registered.
+    /// Registered property names textually present in the preview. This is a lexical
+    /// hint, not the renderer's activation, dependency, and group closure.
     pub required_properties: Vec<String>,
     /// Theme tokens the rendered CSS references.
```

## [crates/compiler/src/renderer.rs](../../crates/compiler/src/renderer.rs)

```diff
--- before/crates/compiler/src/renderer.rs
+++ after/crates/compiler/src/renderer.rs
@@ -53,7 +53,7 @@
     /// Assembly failures become diagnostics and invalidate the generated output.
     ///
-    /// The module's own CSS is cleared on success, because `css` then holds the assembled
-    /// document stylesheet; `module_css` keeps the unwrapped rules for a host that
-    /// transports CSS per module.
+    /// On success `css` holds the assembled document stylesheet and `module_css`
+    /// retains the unwrapped module rules. Assembly failure clears both output rules
+    /// and source edits and records a diagnostic.
     pub fn render_compilation(
         &self,
```

## [crates/compiler/src/runtime.rs](../../crates/compiler/src/runtime.rs)

```diff
--- before/crates/compiler/src/runtime.rs
+++ after/crates/compiler/src/runtime.rs
@@ -97,6 +97,6 @@
     /// Repeated uses of one helper in one realm share a single name and a single import.
     /// The name is the export name unless it collides with a reserved name, in which case
-    /// a numeric suffix is appended until it is free; allocation does not depend on the
-    /// order in which uses are recorded beyond that deterministic search.
+    /// a numeric suffix is appended until it is free. Allocation is deterministic
+    /// for a fixed use order; different realms compete for names in one reserved set.
     pub(crate) fn require(&mut self, helper: RuntimeHelper, at: Span) -> String {
         let realm = self
```

## [crates/compiler/src/transforms.rs](../../crates/compiler/src/transforms.rs)

```diff
--- before/crates/compiler/src/transforms.rs
+++ after/crates/compiler/src/transforms.rs
@@ -22,5 +22,5 @@
     #[error("invalid stillwind transform span: {0}")]
     InvalidSpan(#[source] SourceError),
-    /// `span` starts before `previous_start`, or at the same offset as another edit.
+    /// `span` overlaps the preceding edit or shares its start offset.
     #[error("overlapping stillwind transform span {span:?}, previous start {previous_start}")]
     Overlap {
```

## [crates/compiler/tests/static_compile/conditional_flow.rs](../../crates/compiler/tests/static_compile/conditional_flow.rs)

```diff
--- before/crates/compiler/tests/static_compile/conditional_flow.rs
+++ after/crates/compiler/tests/static_compile/conditional_flow.rs
@@ -1,5 +1,5 @@
 //! Structure-preserving lowering of conditional class expressions.
 //!
-//! Pins that every test expression is emitted once and evaluated once, that `&&` and `?:` keep their
+//! Checks emitted code structure and occurrences of test expressions (without executing JS), that `&&` and `?:` keep their
 //! short-circuiting and source evaluation order (attribute values included), that base rules precede
 //! conditional rules in the stylesheet, and that unsupported forms keep reporting their existing
```

## [crates/compiler/tests/static_compile/parity.rs](../../crates/compiler/tests/static_compile/parity.rs)

```diff
--- before/crates/compiler/tests/static_compile/parity.rs
+++ after/crates/compiler/tests/static_compile/parity.rs
@@ -3,5 +3,5 @@
 //! Fixture candidates are compiled through Stillwind's own entry points, and the resulting rules are
 //! compared to Tailwind's output after selector substitution and, separately, after minification, so
-//! a difference in ordering or spacing shows up instead of being normalized away. Variant generation
+//! the direct and normalized comparisons cover different serialization properties. Variant generation
 //! order is compared against fixtures whose candidates are spelled out of canonical order, so the
 //! check cannot pass by preserving the input order.
```

## [crates/css/src/blocks.rs](../../crates/css/src/blocks.rs)

```diff
--- before/crates/css/src/blocks.rs
+++ after/crates/css/src/blocks.rs
@@ -2,13 +2,12 @@
 //!
 //! These helpers read `@directive header { body }` and `@directive name;` forms out of
-//! raw CSS text using cssparser tokens, so braces inside strings, urls and comments
-//! never split a block. They operate without understanding the declarations they carry:
+//! raw CSS text. Extraction uses cssparser tokens; stripping and brace helpers use
+//! simpler textual scans with different handling of strings, escapes and comments. They operate without understanding the declarations they carry:
 //! callers decide what a body means.
 //!
 //! Two behaviours matter for callers. Malformed input is never half-consumed: an
 //! unclosed block is skipped (or, when stripping, preserved from the directive onward)
-//! so the CSS parser can still report it. And `{modifier}` is treated as literal text
-//! rather than a block opener, because it appears inside `@theme` headers and
-//! interpolation syntax.
+//! so the CSS parser can still report it. The textual stripping helpers specially
+//! skip `{modifier}` markers; that exception is not shared by the token scanner.

 use cssparser::{ParseError, Parser, Token};
@@ -170,6 +169,6 @@
 ///
 /// `{modifier}` is skipped as literal text rather than counted as an opening brace. An
-/// unterminated block is preserved from the directive onward, so the text still fails
-/// in the CSS parser instead of being truncated here.
+/// unterminated body is preserved from the directive onward. This textual helper
+/// does not distinguish strings or comments from directive text and braces.
 pub fn strip_blocks(css: &str, directive: &str) -> String {
     let mut output = String::new();
```

## [crates/css/src/colors.rs](../../crates/css/src/colors.rs)

```diff
--- before/crates/css/src/colors.rs
+++ after/crates/css/src/colors.rs
@@ -31,7 +31,6 @@
 /// additionally converted to Tailwind's relative-color fallback.
 ///
-/// Returns `None` when any shadow in the list cannot be parsed or has no color to
-/// replace; the caller must then keep the original value rather than emit a partially
-/// rewritten list.
+/// Returns `None` when any shadow cannot be parsed or serialized. An omitted color
+/// uses the parser's default color; the whole list is rewritten or no result is returned.
 #[must_use]
 pub fn shadow_with_color_override(
@@ -169,6 +168,7 @@
 }

-/// True when the whole value is one quoted CSS string: both quotes present, non-empty,
-/// and the closing quote unescaped.
+/// Checks matching quote delimiters and rejects a backslash immediately before the
+/// last quote. This lexical approximation accepts empty strings and does not validate
+/// interior quotes or count backslashes to determine escape parity.
 pub(super) fn is_css_string(value: &str) -> bool {
     let Some(quote) = value
```

## [crates/css/src/math/expression.rs](../../crates/css/src/math/expression.rs)

```diff
--- before/crates/css/src/math/expression.rs
+++ after/crates/css/src/math/expression.rs
@@ -6,6 +6,6 @@
 //! becomes an `Atom` and is preserved verbatim.
 //!
-//! Serialization is driven by precedence, so the printed expression reparses to
-//! the same tree; callers must not reformat the result themselves.
+//! Serialization uses operator precedence. It may flatten equal-precedence sums
+//! and products, so the printed syntax tree need not be identical to the stored tree.

 #[derive(Clone, Debug, PartialEq)]
@@ -152,6 +152,6 @@
     ///
     /// A negation on the right of a sum prints as subtraction, and division
-    /// prints its right operand one level tighter, so the output reparses to the
-    /// same tree. Callers must not rewrite the result.
+    /// prints its right operand one level tighter. Equal-precedence sums and
+    /// products may be flattened during printing.
     pub(crate) fn serialize(&self, parent_precedence: u8) -> String {
         let precedence = self.precedence();
```

## [crates/css/src/math/folding.rs](../../crates/css/src/math/folding.rs)

```diff
--- before/crates/css/src/math/folding.rs
+++ after/crates/css/src/math/folding.rs
@@ -2,6 +2,6 @@
 //!
 //! Folding is textual replacement guided by a real parse: every `calc(` is
-//! located, its matching parenthesis resolved, and only a successfully parsed
-//! expression is reprinted. A `calc()` that fails to parse, or whose matching
+//! located by substring search, its matching parenthesis resolved, and a successfully
+//! parsed expression is reprinted. The outer search does not skip CSS strings or comments. A `calc()` that fails to parse, or whose matching
 //! parenthesis is missing, is copied through unchanged, so malformed input
 //! still reaches the CSS parser and produces a real error there.
@@ -57,5 +57,5 @@
 ///
 /// The unit applies only to a value that really folded to the bare number zero,
-/// never to an authored `0` or to an atom.
+/// including `calc(0)`, but never to a bare `0` outside `calc()` or to an atom.
 #[must_use]
 pub fn fold_calc_expressions_with_zero_unit(value: &str, zero_unit: Option<&str>) -> String {
```

## [crates/css/src/math/parser.rs](../../crates/css/src/math/parser.rs)

```diff
--- before/crates/css/src/math/parser.rs
+++ after/crates/css/src/math/parser.rs
@@ -12,6 +12,6 @@
 /// One cursor over an expression string.
 ///
-/// The parser borrows its input and allocates nothing until an atom is
-/// accepted, so it can read a temporary slice of a larger CSS string.
+/// The parser borrows its input and allocates owned units, atom text, and boxed
+/// operator nodes as needed; the input may be a slice of a larger CSS string.
 pub(crate) struct MathParser<'a> {
     /// The expression text, without the surrounding `calc(` and `)`.
```

## [crates/css/src/output.rs](../../crates/css/src/output.rs)

```diff
--- before/crates/css/src/output.rs
+++ after/crates/css/src/output.rs
@@ -57,5 +57,5 @@
         origin: Option<Span>,
     },
-    /// One `@layer` order statement, emitted before any layer body.
+    /// One `@layer` order statement, retained at its position in the template.
     LayerOrder {
         /// The order statement text.
```

## [crates/css/src/resources.rs](../../crates/css/src/resources.rs)

```diff
--- before/crates/css/src/resources.rs
+++ after/crates/css/src/resources.rs
@@ -19,5 +19,5 @@
     /// The entire `url()` expression, or the string in `image-set()`.
     pub range: Range<usize>,
-    /// The raw specifier text, exactly as written.
+    /// Decoded specifier text, with CSS escapes resolved by the tokenizer.
     pub specifier: String,
     /// True when the reference was written as a quoted string (`image-set`
```

## [crates/css/src/values/properties.rs](../../crates/css/src/values/properties.rs)

```diff
--- before/crates/css/src/values/properties.rs
+++ after/crates/css/src/values/properties.rs
@@ -225,9 +225,7 @@
 }

-/// True for a CSS custom identifier: it must not start with a digit and must
-/// not be one of the CSS-wide keywords.
-///
-/// Non-ASCII characters are allowed, matching CSS identifier rules; this is
-/// deliberately not the narrower ASCII alphabet used for generated names.
+/// Approximate custom-identifier check using character classes and lowercase
+/// CSS-wide keyword exclusions. It accepts non-ASCII characters but does not
+/// implement CSS escape decoding or the full identifier grammar.
 pub(crate) fn custom_ident(value: &str) -> bool {
     value
```

## [crates/css/src/values/syntax.rs](../../crates/css/src/values/syntax.rs)

```diff
--- before/crates/css/src/values/syntax.rs
+++ after/crates/css/src/values/syntax.rs
@@ -4,7 +4,6 @@
 //! `content` value, a transform function body, a `var()` reference - and they exist so
 //! that a value type without a typed parser is still checked by something stricter than
-//! "looks plausible". All of them reject more than they accept: a false negative
-//! surfaces as an unsupported arbitrary value, while a false positive would let invalid
-//! CSS reach the output.
+//! "looks plausible". Some helpers still use lexical shape checks, so acceptance is
+//! not proof that a browser will accept the whole value.

 use lightningcss::properties::{Property, PropertyId};
```

## [crates/css/src/values/validation.rs](../../crates/css/src/values/validation.rs)

```diff
--- before/crates/css/src/values/validation.rs
+++ after/crates/css/src/values/validation.rs
@@ -80,5 +80,5 @@
     /// `bg-size`: `<bg-size>`.
     BackgroundSize,
-    /// `shadow`: a box-shadow list.
+    /// `shadow`: one box shadow; list splitting is a separate operation.
     Shadow,
     /// `content`: the primitive `content` productions; bare identifiers are rejected.
@@ -96,5 +96,5 @@
     /// `skew-y-arguments`: the argument list of `skewY()`.
     SkewYArguments,
-    /// `animation`: a `<single-animation>` shorthand value.
+    /// `animation`: an animation shorthand value, including comma-separated animations.
     Animation,
     /// `font-family`: a font family list.
```

## [crates/design-system/src/evaluation/builtins/metadata.rs](../../crates/design-system/src/evaluation/builtins/metadata.rs)

```diff
--- before/crates/design-system/src/evaluation/builtins/metadata.rs
+++ after/crates/design-system/src/evaluation/builtins/metadata.rs
@@ -15,5 +15,5 @@
 use crate::{ThemeIndex, ThemeNamespace};

-/// Every `@property` definition selected by the theme, in authored order.
+/// Every `@stillwind-property` definition selected by the theme, in authored order.
 pub fn builtin_property_definitions(theme: &ThemeIndex) -> &[CustomPropertyDefinition] {
     &theme.registry.properties
```

## [crates/design-system/src/evaluation/builtins/registry.rs](../../crates/design-system/src/evaluation/builtins/registry.rs)

```diff
--- before/crates/design-system/src/evaluation/builtins/registry.rs
+++ after/crates/design-system/src/evaluation/builtins/registry.rs
@@ -26,5 +26,6 @@
 ///
 /// A pattern matches by exact name or by a trailing-`*` prefix; among matching patterns
-/// the longest prefix wins, so a specific registration outranks a broad one.
+/// an exact name wins over wildcards, otherwise the longest prefix wins. Equal
+/// specificity selects the last matching entry.
 /// [`usize::MAX`] sorts an unregistered property last.
 pub(crate) fn arbitrary_property_order(property: &str, theme: &ThemeIndex) -> usize {
```

## [crates/design-system/src/evaluation/tests/audit.rs](../../crates/design-system/src/evaluation/tests/audit.rs)

```diff
--- before/crates/design-system/src/evaluation/tests/audit.rs
+++ after/crates/design-system/src/evaluation/tests/audit.rs
@@ -1,7 +1,7 @@
 //! Registry audits over the vendored CSS and the parity fixtures.
 //!
-//! Pins that negative-value definitions state their sign explicitly and that every
-//! built-in definition is reachable from the parity fixtures, so a definition cannot be
-//! added without a spelling that exercises it.
+//! The regular test checks that negative-value definitions state their sign explicitly.
+//! The ignored, opt-in audit checks built-in reachability from parity fixtures; normal
+//! test runs do not enforce that exhaustive coverage requirement.

 use super::*;
```

## [crates/design-system/src/evaluation/tests/mod.rs](../../crates/design-system/src/evaluation/tests/mod.rs)

```diff
--- before/crates/design-system/src/evaluation/tests/mod.rs
+++ after/crates/design-system/src/evaluation/tests/mod.rs
@@ -3,6 +3,7 @@
 //! Each suite parses real registry CSS, either directly or through a snapshot built from
 //! CSS that is already in memory, and asserts the resolved values, because most
-//! evaluation contracts are about what users write rather than about internal types. Only
-//! `tests/loading.rs` reads CSS files from disk. [`support`] holds the shared fixtures;
+//! evaluation contracts are about what users write rather than about internal types.
+//! Shared fixtures may expand package CSS from disk; `tests/loading.rs` also exercises
+//! caller-driven filesystem loading. [`support`] holds the shared fixtures;
 //! the other modules own one area each: built-in registry contents, utility matching and
 //! domains, value families, variants and custom variants, overloads, relationships,
```

## [crates/design-system/src/evaluation/utilities/formatting.rs](../../crates/design-system/src/evaluation/utilities/formatting.rs)

```diff
--- before/crates/design-system/src/evaluation/utilities/formatting.rs
+++ after/crates/design-system/src/evaluation/utilities/formatting.rs
@@ -24,5 +24,6 @@
 /// The input is a declaration list, so the marker is applied per declaration rather
 /// than once at the end of the string. An empty body stays empty instead of gaining a
-/// stray marker.
+/// stray marker. This helper splits on every semicolon, including semicolons inside
+/// strings or function arguments; it does not tokenize the declaration list.
 pub(crate) fn important(declaration: &str) -> String {
     declaration
```

## [crates/design-system/src/evaluation/utilities/matching.rs](../../crates/design-system/src/evaluation/utilities/matching.rs)

```diff
--- before/crates/design-system/src/evaluation/utilities/matching.rs
+++ after/crates/design-system/src/evaluation/utilities/matching.rs
@@ -2,6 +2,6 @@
 //!
 //! Because user CSS is the authoritative registry layer, a project definition must be
-//! able to replace any built-in name; a failed user branch therefore never falls back
-//! to a built-in spelling. Within one layer, every matching definition is a typed
+//! able to replace any built-in name when its branch resolves. If all user branches
+//! fail, resolution still tries the built-in registry. Within one layer, every matching definition is a typed
 //! declaration branch, and a candidate selects the branches whose value domains accept
 //! it.
@@ -41,6 +41,6 @@
 /// Resolves a named candidate through the registry layers.
 ///
-/// Project CSS is consulted first; a project definition for a name wins even when the
-/// built-in registry also defines it. There is deliberately no property-prefix
+/// Project CSS is consulted first; a successfully resolved project branch wins. If
+/// no project branch resolves, built-in definitions are tried. There is deliberately no property-prefix
 /// fallback: a definition that rejects the candidate's value type is a non-match, not
 /// an invitation to reinterpret the candidate.
```

## [crates/design-system/src/evaluation/utilities/resolve.rs](../../crates/design-system/src/evaluation/utilities/resolve.rs)

```diff
--- before/crates/design-system/src/evaluation/utilities/resolve.rs
+++ after/crates/design-system/src/evaluation/utilities/resolve.rs
@@ -113,7 +113,7 @@
 ///
 /// Variants, negation, and importance are rejected: an animation option describes a
-/// value, not a candidate. Only a branch that produces exactly one plain declaration is
-/// accepted, because an option cannot carry a selector or conditionals into an
-/// `animation` shorthand.
+/// value, not a qualified utility. A plain declaration list is accepted; selectors,
+/// conditionals and additional targets are rejected. The declaration list can
+/// contain several properties.
 #[must_use]
 pub fn resolve_animation_option(raw: &str, theme: &ThemeIndex) -> Option<String> {
```

## [crates/design-system/src/evaluation/values/colors.rs](../../crates/design-system/src/evaluation/values/colors.rs)

```diff
--- before/crates/design-system/src/evaluation/values/colors.rs
+++ after/crates/design-system/src/evaluation/values/colors.rs
@@ -1,13 +1,12 @@
 //! Color compatibility branches and theme shadow color modifiers.
 //!
-//! Tailwind emits a color declaration twice: a legacy branch that works everywhere and
+//! The compatibility path can emit a color declaration twice: an sRGB branch and
 //! a modern branch guarded by a capability query. The legacy branch mixes in sRGB, the
 //! modern branch uses `oklab`, and both are produced here so the two passes stay
 //! visibly paired.
 //!
-//! Two spellings deliberately skip mixing: `currentcolor` in the legacy branch, and CSS
-//! wide keywords or `transparent`, for which no synthesizable sRGB fallback exists. In
-//! those cases the legacy and modern texts are identical and the emitter can drop the
-//! conditional wrapper.
+//! Legacy `currentcolor` skips mixing, while its modern branch can still mix.
+//! `inherit` and `transparent` select oklab in both passes when no extra modern alpha
+//! is supplied. Identical branch text can then be emitted without a capability wrapper.

 use stillwind_css::{shadow_with_color_fallback, shadow_with_color_override};
@@ -32,5 +31,5 @@
     /// `currentcolor`, which the legacy branch cannot mix.
     CurrentColor,
-    /// A CSS-wide keyword or `transparent`, which has no sRGB mix.
+    /// `inherit` or `transparent`, selecting the modern space in both passes by default.
     ModernOnlyKeyword,
     /// Any other authored color.
@@ -58,6 +57,6 @@
 /// The argument list is a domain list: a color and an alpha, optionally with a second
 /// alpha for the modern branch. `100%` alpha and legacy `currentcolor` are emitted
-/// without a `color-mix()` so their text matches what the modern branch produces and
-/// the pair can be emitted unconditionally.
+/// without a `color-mix()` at the initial mixing step. The modern branch may still
+/// differ, particularly for `currentcolor` or an additional modern alpha.
 pub(crate) fn resolve_compat_color(arguments: &str, mode: CompatColorMode) -> Option<String> {
     let arguments = split_domains(arguments);
```

## [crates/design-system/src/evaluation/values/macros.rs](../../crates/design-system/src/evaluation/values/macros.rs)

```diff
--- before/crates/design-system/src/evaluation/values/macros.rs
+++ after/crates/design-system/src/evaluation/values/macros.rs
@@ -2,5 +2,6 @@
 //!
 //! These macros are resolved by text substitution in place, and each one consumes a
-//! whole macro invocation, so the loops always shrink the body and terminate. An
+//! whole macro invocation. Replacement text may be longer or contain another macro;
+//! these loops do not enforce a decreasing measure or detect cycles. An
 //! unclosed or unresolvable macro fails the whole body rather than leaving the
 //! placeholder text in generated CSS.
```

## [crates/design-system/src/evaluation/values/placeholders.rs](../../crates/design-system/src/evaluation/values/placeholders.rs)

```diff
--- before/crates/design-system/src/evaluation/values/placeholders.rs
+++ after/crates/design-system/src/evaluation/values/placeholders.rs
@@ -4,6 +4,6 @@
 //! text: nested macros first, then `--value(...)`, then the modifier forms, then
 //! `--alpha(...)`, and finally the color forms. Every remaining placeholder at the end
-//! fails the declaration, so an unrecognized or malformed placeholder can never reach
-//! generated CSS.
+//! from the recognized marker list fails the declaration. Unknown function names
+//! are not rejected by this check and can remain in generated CSS.
 //!
 //! The capture a placeholder sees is the candidate text after the matched definition's
```

## [crates/design-system/src/evaluation/values/theme.rs](../../crates/design-system/src/evaluation/values/theme.rs)

```diff
--- before/crates/design-system/src/evaluation/values/theme.rs
+++ after/crates/design-system/src/evaluation/values/theme.rs
@@ -22,6 +22,7 @@
 /// Order matters: a typed arbitrary or variable capture wins over a named token, a
 /// named token wins over the numeric spacing scale, and a value that matches nothing
-/// returns [`None`]. A non-ASCII capture is rejected because the candidate grammar only
-/// admits ASCII identifiers, even though the same name is a valid CSS custom property.
+/// returns [`None`]. The named-token path rejects non-ASCII captures; this is a
+/// restriction of this lookup, not of the general candidate parser. Typed arbitrary
+/// and variable captures are attempted before that restriction.
 pub(crate) fn theme_value(
     namespace: ThemeNamespace,
@@ -34,6 +35,6 @@
             format!("var({value})")
         }
-        // Tailwind's candidate grammar accepts ASCII identifier tokens only;
-        // Unicode theme names remain valid CSS but are not utility candidates.
+        // This named-token lookup is ASCII-only; the earlier arbitrary-value and
+        // variable paths do not share that restriction.
         _ if !value.is_ascii() => return None,
         _ if let Some(token) = theme.token_in(namespace, value) => {
```

## [crates/design-system/src/evaluation/variants/order.rs](../../crates/design-system/src/evaluation/variants/order.rs)

```diff
--- before/crates/design-system/src/evaluation/variants/order.rs
+++ after/crates/design-system/src/evaluation/variants/order.rs
@@ -8,5 +8,5 @@
 //!
 //! Within a group, a variant's sort key is the value of a theme token; numeric values
-//! compare by magnitude before the authored direction is applied, and anything that is
+//! compare by unit and then magnitude, with direction applied to magnitude. Anything that is
 //! not a number falls back to text order so the comparison stays total and
 //! deterministic.
@@ -247,5 +247,5 @@
 ///
 /// Each stack contributes the distinct positions of its variants, and the positions are
-/// compared in descending order, so the outermost important variant decides first. Ties
+/// compared in descending order, so the highest registry position decides first. Ties
 /// are broken variant by variant within a shared position, which makes the comparison
 /// total for any two stacks.
```

## [crates/design-system/src/evaluation/variants/resolve.rs](../../crates/design-system/src/evaluation/variants/resolve.rs)

```diff
--- before/crates/design-system/src/evaluation/variants/resolve.rs
+++ after/crates/design-system/src/evaluation/variants/resolve.rs
@@ -33,6 +33,5 @@
     /// A fixed selector suffix appended to the utility's own selector.
     SelectorSuffix(&'static str),
-    /// A selector template containing `:--value()` for the capture and `&` for the
-    /// utility.
+    /// A resolved selector template containing `&` for the utility selector.
     SelectorTemplate(String),
     /// A selector template that also declares properties of its own.
@@ -143,5 +142,5 @@
 /// can find them across independent `style()` calls.
 ///
-/// Only an unmodified, unnegated, variant-free candidate can produce a marker, and only
+/// Only a non-important, unnegated, variant-free candidate can produce a marker, and only
 /// when the registry lists the name as a marker utility. The returned class is emitted
 /// as written, including any named modifier, so two independent calls agree on the
```

## [crates/design-system/src/evaluation/variants/substitution.rs](../../crates/design-system/src/evaluation/variants/substitution.rs)

```diff
--- before/crates/design-system/src/evaluation/variants/substitution.rs
+++ after/crates/design-system/src/evaluation/variants/substitution.rs
@@ -3,7 +3,7 @@
 //! Substitution is a fixed sequence of textual replacements rather than a template
 //! language: `--value()`, an escaped capture marker, the modifier forms, and the two
-//! placeholders used by `nth`-style selectors are replaced in that order, so a capture
-//! that itself contains a marker cannot be re-substituted. Capture text is escaped for
-//! selector use, and underscores become spaces, matching the candidate grammar.
+//! placeholders used by `nth`-style selectors are replaced in that order. Inserted
+//! capture text can therefore be changed by later replacements. This path replaces
+//! underscores and modifier slashes; it is not a general CSS selector escaper.
 //!
 //! Composition goes the other way: an inner variant's selector is folded into the outer
@@ -196,6 +196,5 @@
 /// Fills a definition body's capture placeholders.
 ///
-/// A body without any placeholder is returned unchanged, so the common case costs no
-/// allocation. An arbitrary selector capture loses its `&` and underscore escapes, and
+/// A body without any placeholder is returned as an unchanged owned copy. An arbitrary selector capture loses its `&` and underscore escapes, and
 /// a modifier is escaped as `\/` so it stays part of the selector rather than starting
 /// a new one.
```

## [crates/design-system/src/loading/expansion.rs](../../crates/design-system/src/loading/expansion.rs)

```diff
--- before/crates/design-system/src/loading/expansion.rs
+++ after/crates/design-system/src/loading/expansion.rs
@@ -170,6 +170,7 @@
     /// `None` when the span is not attributable.
     ///
-    /// `None` covers synthetic wrapper text, spans that start in a length-changing
-    /// substitution, ranges that cross a file boundary or a gap in the original,
+    /// A length-changing substitution maps to its full authored expression, including
+    /// queries into its interior. `None` covers synthetic wrapper text and ranges
+    /// that cross a file boundary or a gap in the original,
     /// reversed spans, and coordinates beyond 32 bits. Callers must treat a missing
     /// mapping as "no original location" rather than falling back to the generated
```

## [crates/design-system/src/loading/imports.rs](../../crates/design-system/src/loading/imports.rs)

```diff
--- before/crates/design-system/src/loading/imports.rs
+++ after/crates/design-system/src/loading/imports.rs
@@ -26,5 +26,5 @@
         /// Byte offset of the modifier in the buffer.
         offset: usize,
-        /// Modifier text as written, including its leading dot.
+        /// Unsupported modifier name reported by the token parser.
         modifier: String,
     },
```

## [crates/design-system/src/loading/protocol.rs](../../crates/design-system/src/loading/protocol.rs)

```diff
--- before/crates/design-system/src/loading/protocol.rs
+++ after/crates/design-system/src/loading/protocol.rs
@@ -47,5 +47,5 @@
     /// Name of the source that authored the import.
     pub importer: CssSourceName,
-    /// Verbatim specifier text from the authored `@import`.
+    /// Decoded CSS import specifier text; escapes are resolved by the tokenizer.
     pub specifier: String,
     /// Kind of dependency that produced this request.
@@ -98,5 +98,5 @@
     /// Name of the source that authored the reference.
     pub owner: String,
-    /// Verbatim specifier the host must resolve.
+    /// Decoded resource specifier the host must resolve.
     pub specifier: String,
     /// Span of the authored reference in its own source buffer.
```

## [crates/design-system/src/registry/definitions.rs](../../crates/design-system/src/registry/definitions.rs)

```diff
--- before/crates/design-system/src/registry/definitions.rs
+++ after/crates/design-system/src/registry/definitions.rs
@@ -34,5 +34,5 @@
     /// All `@stillwind-variant` definitions in authored order.
     pub variants: Vec<CustomVariant>,
-    /// All `@property` definitions in authored order.
+    /// All `@stillwind-property` definitions in authored order.
     pub properties: Vec<CustomPropertyDefinition>,
     /// Exact-name utilities, grouped without losing authored order.
@@ -52,5 +52,5 @@
     /// Function names that accept a declaration format as an argument.
     pub function_formats: BTreeSet<String>,
-    /// Property names whose values relate to each other for ordering checks.
+    /// Candidate patterns identifying compatibility relationships.
     pub relationships: BTreeSet<String>,
     /// Utilities that emit only a marker class and no declarations.
```

## [crates/design-system/src/registry/mod.rs](../../crates/design-system/src/registry/mod.rs)

```diff
--- before/crates/design-system/src/registry/mod.rs
+++ after/crates/design-system/src/registry/mod.rs
@@ -5,5 +5,6 @@
 //! carry a `builtin_order`, while user-authored directives (`@utility`, `@variant`,
 //! `@custom-variant`) do not. Order is used later to decide candidate precedence, so a
-//! parser here must return definitions in authored order. That order is carried as a
+//! built-in parsers preserve authored order. User variants are grouped by syntax form
+//! and do not preserve cross-form order. Built-in order is carried as a
 //! byte offset where a definition must compare against another kind — utilities and
 //! arbitrary properties share one ordering space — and as a simple counter for variants,
```

## [crates/design-system/src/registry/normalization.rs](../../crates/design-system/src/registry/normalization.rs)

```diff
--- before/crates/design-system/src/registry/normalization.rs
+++ after/crates/design-system/src/registry/normalization.rs
@@ -2,6 +2,6 @@
 //!
 //! Vendored definitions are re-serialized so a built-in body is byte-identical
-//! regardless of how the CSS file was formatted, which keeps fingerprints and generated
-//! output stable. The rules are conservative: quoted content is preserved
+//! after removing selected whitespace. Preserved spacing and the raw registry source
+//! can still affect serialization and snapshot fingerprints. The rules are conservative: quoted content is preserved
 //! byte-for-byte, spacing that Tailwind's own serializer keeps after commas in selected
 //! functions and custom-property declarations is retained, and a separator before a
```

## [crates/design-system/src/registry/variants.rs](../../crates/design-system/src/registry/variants.rs)

```diff
--- before/crates/design-system/src/registry/variants.rs
+++ after/crates/design-system/src/registry/variants.rs
@@ -44,5 +44,5 @@
 #[derive(Clone, Debug, Eq, PartialEq)]
 pub struct CustomVariant {
-    /// Authored name without the capture `*`.
+    /// Decoded name, retaining a trailing capture `*` when `wildcard` is true.
     pub name: String,
     /// Whether the final `*` in `name` captures a value. Escaped `\*` in a definition
@@ -140,7 +140,7 @@
 ///
 /// Only blocks whose body contains `@slot` are treated as `@variant` definitions; a
-/// `@variant` block without a slot cannot produce a rule and is ignored. Results keep
-/// authored order, and a repeated name is a separate definition rather than a
-/// replacement.
+/// `@variant` block without a slot is ignored. Results are grouped by syntax form:
+/// selector `@custom-variant`, block `@custom-variant`, then block `@variant`. Source
+/// order is retained within each group, not across forms; repeated names are retained.
 #[must_use]
 pub fn parse_variant_definitions(css: &str) -> Vec<CustomVariant> {
```

## [crates/design-system/src/snapshot/data.rs](../../crates/design-system/src/snapshot/data.rs)

```diff
--- before/crates/design-system/src/snapshot/data.rs
+++ after/crates/design-system/src/snapshot/data.rs
@@ -65,5 +65,6 @@
     /// Authored lint severities.
     pub lint_policy: Fingerprint,
-    /// One fingerprint per effective theme token name.
+    /// Last authored fingerprint per token name, including reset entries. This map
+    /// is built before effective-token filtering; use `tokens()` for active values.
     pub theme_tokens: BTreeMap<String, Fingerprint>,
     /// One fingerprint per utility name group.
@@ -136,6 +137,6 @@
     /// excluded from every fingerprint: they depend on the order in which host reads
     /// completed, and identical content must fingerprint identically regardless of that
-    /// order. The built-in definition set is selected by the theme rather than
-    /// injected, so a stylesheet that imports no registry yields no built-ins.
+    /// order. Built-ins are parsed from the supplied theme CSS rather than injected;
+    /// that CSS may declare a registry directly or receive one through imports.
     ///
     /// # Errors
@@ -353,5 +354,5 @@
     }

-    /// `@property` definitions from the selected built-ins and the project CSS.
+    /// `@stillwind-property` definitions from the selected built-ins and the project CSS.
     pub fn properties(&self) -> &PropertyRegistry {
         &self.inner.properties
@@ -364,6 +365,6 @@
     }

-    /// The built-in definition set selected by the loaded CSS, or `None` when none were
-    /// selected.
+    /// The built-in definition set selected by the loaded CSS. Current construction
+    /// returns `Some` even for an empty registry; emptiness is not represented by `None`.
     pub fn builtins(&self) -> Option<&Arc<BuiltinDefinitions>> {
         self.inner.builtins.as_ref()
```

## [crates/design-system/src/snapshot/registry.rs](../../crates/design-system/src/snapshot/registry.rs)

```diff
--- before/crates/design-system/src/snapshot/registry.rs
+++ after/crates/design-system/src/snapshot/registry.rs
@@ -68,5 +68,5 @@
 pub type VariantRegistry = DefinitionRegistry<CustomVariant>;

-/// `@property` definitions, keyed by property name.
+/// `@stillwind-property` definitions, keyed by property name.
 pub type PropertyRegistry = DefinitionRegistry<CustomPropertyDefinition>;

```

## [crates/design-system/src/theme/declarations.rs](../../crates/design-system/src/theme/declarations.rs)

```diff
--- before/crates/design-system/src/theme/declarations.rs
+++ after/crates/design-system/src/theme/declarations.rs
@@ -19,9 +19,10 @@
 #[derive(Clone, Copy, Debug, Eq, PartialEq)]
 pub enum ThemeTokenMode {
-    /// Ordinary token: emitted as a custom property and referenced by `var()`.
+    /// Ordinary token: emitted when required and referenced by `var()`.
     Default,
     /// Token inlined into the generated declaration instead of being emitted.
     Inline,
-    /// Token that is always emitted, whether or not a declaration references it.
+    /// Token included in the required set without a reference; later resets and
+    /// registry pruning can still remove it.
     Static,
 }
@@ -64,5 +65,5 @@
     /// Which namespace and name the declaration resolved to.
     pub kind: ThemeTokenKind,
-    /// Whether the declaration replaces or adds to the built-in value.
+    /// Emission policy: ordinary referenced token, inline value, or static retention.
     pub mode: ThemeTokenMode,
 }
```

## [crates/design-system/src/theme/index.rs](../../crates/design-system/src/theme/index.rs)

```diff
--- before/crates/design-system/src/theme/index.rs
+++ after/crates/design-system/src/theme/index.rs
@@ -40,7 +40,7 @@
     /// User `@utility` definitions in authored order.
     pub utilities: Vec<CustomUtility>,
-    /// User `@variant` and `@custom-variant` definitions in authored order.
+    /// User variant definitions grouped by syntax form; source order within each form.
     pub variants: Vec<CustomVariant>,
-    /// Authored `@property` definitions in authored order.
+    /// Compiler-owned `@stillwind-property` definitions in authored order.
     pub properties: Vec<CustomPropertyDefinition>,
     /// Utility names whose declarations provide compatibility marker classes.
@@ -48,5 +48,5 @@
     /// Generic serialization formats attached to CSS declarations.
     pub declaration_formats: BTreeMap<String, String>,
-    /// Property-name patterns whose values relate to each other.
+    /// Candidate patterns identifying compatibility relationships, such as `group-*`.
     pub relationship_patterns: BTreeSet<String>,
     /// Property names declared by authored `@property` rules.
@@ -69,5 +69,5 @@
     }

-    /// Every active named token in `namespace`, in authored order.
+    /// Every active named token in `namespace`, ordered by custom-property name.
     #[must_use]
     pub fn tokens_in(&self, namespace: ThemeNamespace) -> Vec<&ThemeToken> {
@@ -139,6 +139,6 @@
     /// Returns the effective CSS custom properties after v4 namespace resets.
     ///
-    /// The returned order is authored order with superseded entries removed, so
-    /// consumers may rely on it for stable output; it is not sorted.
+    /// Resets and replacements are applied in authored order. The result is sorted
+    /// by custom-property name because the effective values are stored in a `BTreeMap`.
     #[must_use]
     pub fn active_tokens(&self) -> Vec<&ThemeToken> {
@@ -168,9 +168,9 @@
     }

-    /// Selects the effective tokens required by emitted declarations. Project tokens
-    /// are retained as authored; vendored defaults are tree-shaken and their `var()`
-    /// dependencies are followed transitively.
-    ///
-    /// Static tokens are always required, inline tokens are never emitted, and the
+    /// Selects effective tokens by lexical custom-property references and follows
+    /// their dependencies transitively. Ordinary project and built-in tokens are
+    /// both omitted when unused; static mode requests retention.
+    ///
+    /// Static tokens seed the required set, inline tokens are never emitted, and the
     /// dependency walk terminates on cyclic aliases.
     #[must_use]
```

## [crates/design-system/src/theme/parser.rs](../../crates/design-system/src/theme/parser.rs)

```diff
--- before/crates/design-system/src/theme/parser.rs
+++ after/crates/design-system/src/theme/parser.rs
@@ -5,5 +5,5 @@
 //! accepts. It reads declared semantics only: no package name, default theme, reset, or
 //! built-in registry is injected, so the definitions a snapshot ends up with are
-//! exactly the ones the host supplied, in the order they arrived.
+//! exactly the ones the host supplied, expanded in authored import order.
 //!
 //! Because directives are found textually, the buffer is first checked for an
```

## [crates/ir/src/analysis.rs](../../crates/ir/src/analysis.rs)

```diff
--- before/crates/ir/src/analysis.rs
+++ after/crates/ir/src/analysis.rs
@@ -54,5 +54,6 @@
     /// Suppression directives whose scope may still cover later findings.
     pub lint_directives: Vec<LintDirective>,
-    /// Errors that are already final; they survive [`SourceAnalyzer::finalize`].
+    /// Scan-time errors. Finalization with accepted facts may replace these with
+    /// diagnostics from a new parse.
     pub diagnostics: Vec<CompileError>,
     /// Findings that do not depend on cross-module facts.
```

## [crates/ir/src/model.rs](../../crates/ir/src/model.rs)

```diff
--- before/crates/ir/src/model.rs
+++ after/crates/ir/src/model.rs
@@ -32,5 +32,5 @@
     #[serde(default)]
     pub imports: ModuleImports,
-    /// In source order. The compiler emits one rule unit per call.
+    /// In source order. A call can emit a base unit plus conditional units, or no CSS.
     pub style_calls: Vec<StyleCall>,
     /// In source order. Each entry is a `style.ref()` declaration.
@@ -131,10 +131,10 @@
     /// `style.ref().in(scope)`.
     pub scope: Option<ScopeDefinition>,
-    /// Span of the declaration call, part of the ref's identity.
+    /// Span of the producing call, used for replacement; identity uses `name.span`.
     pub call_span: Span,
 }

 /// A `style.scope()` declaration. Scopes are pure identities: they produce CSS
-/// selectors and attachment classes, never props.
+/// selectors and class-bearing runtime fragments rather than arbitrary attribute props.
 #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
 pub struct ScopeDefinition {
@@ -145,5 +145,5 @@
     /// Optional user label that participates in the generated class name.
     pub label: Option<Spanned<String>>,
-    /// Span of the declaration call, part of the scope's identity.
+    /// Span of the producing call, used for replacement; identity uses `name.span`.
     pub call_span: Span,
 }
@@ -152,7 +152,7 @@
 #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
 pub struct AnimationDefinition {
-    /// Exported declaration name, used as the identity's binding name.
-    pub name: Spanned<String>,
-    /// Span of the declaration call, part of the animation's identity.
+    /// Local binding name and its declaration span, used for identity.
+    pub name: Spanned<String>,
+    /// Span of the producing call, used for replacement; identity uses `name.span`.
     pub call_span: Span,
     /// In source order. The declaration order is the keyframe order.
@@ -173,13 +173,13 @@
 #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
 pub struct ViewTransitionDefinition {
-    /// Exported declaration name. Every use shares one generated class.
-    pub name: Spanned<String>,
-    /// Span of the declaration call, part of the transition's identity.
+    /// Local binding name and its declaration span. Uses share the declaration identity.
+    pub name: Spanned<String>,
+    /// Span of the producing call, used for replacement; identity uses `name.span`.
     pub call_span: Span,
-    /// Group name that ties transitions sharing a view-transition group.
+    /// Utilities for the `::view-transition-group` pseudo-element.
     pub group: Option<Spanned<String>>,
-    /// Old-state selector or class list.
+    /// Utilities for the `::view-transition-old` pseudo-element.
     pub old: Option<Spanned<String>>,
-    /// New-state selector or class list.
+    /// Utilities for the `::view-transition-new` pseudo-element.
     pub new: Option<Spanned<String>>,
 }
@@ -200,9 +200,9 @@
 #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
 pub enum HandleProjection {
-    /// `anchor()`: the element is the anchor for positioning.
+    /// `ref.anchor`: the element is the anchor for positioning.
     Anchor,
-    /// `position()`: the element is positioned against an anchor.
+    /// `ref.position`: the element is positioned against an anchor.
     Position,
-    /// `anchorScope()`: the element is the containing block for scoped refs.
+    /// `ref.anchorScope`: limits anchor-name visibility with `anchor-scope`.
     Scope,
     /// `container()`: the element is a query container of the named type.
@@ -234,10 +234,10 @@
         /// The referenced base expression, emitted verbatim.
         value: Span,
-        /// Candidate identities of the base call, in source order.
+        /// Candidate identities of the base call, deduplicated in identity order.
         candidates: Vec<IdentityId>,
     },
     /// Class contributions that keep the source expression structure.
     Class(ClassExpression),
-    /// `at(name)`: attaches a condition to a container query declaration.
+    /// `ref.at("query", "utilities")`: applies utilities inside a container query.
     ContainerQuery {
         /// The container ref this query applies to.
@@ -245,5 +245,5 @@
         /// Query condition text, such as `sm` or a size feature list.
         query: Spanned<String>,
-        /// The value the query is attached to.
+        /// Utility string to emit inside the query.
         value: Spanned<String>,
     },
@@ -336,5 +336,5 @@
     /// anything the framework runtime accepts.
     External(Span),
-    /// `ref.anchor()` and the other ref projections.
+    /// `ref.anchor` and the other ref projections.
     HandleProjection {
         /// The ref this projection applies to.
@@ -352,9 +352,9 @@
         projection: HandleProjection,
     },
-    /// `style(animation)` with a statically known animation.
+    /// `style(animation("options"))` with a statically known animation.
     AnimationUse {
         /// The declared animation being used.
         animation: AssetReference,
-        /// Animation options text, passed through verbatim.
+        /// Literal option candidates resolved into animation declarations by the compiler.
         options: Spanned<String>,
     },
@@ -365,5 +365,5 @@
         /// Every statically reachable alternative, used for CSS planning.
         animations: Vec<AssetReference>,
-        /// Animation options text, passed through verbatim.
+        /// Literal option candidates resolved into animation declarations by the compiler.
         options: Spanned<String>,
     },
```

## [crates/js-oxc/src/analyzer.rs](../../crates/js-oxc/src/analyzer.rs)

```diff
--- before/crates/js-oxc/src/analyzer.rs
+++ after/crates/js-oxc/src/analyzer.rs
@@ -93,5 +93,5 @@
     /// every fact span slices the scanned source; otherwise the scan's own IR is kept
     /// and an [`IntegrationDiagnostic`] records why. Facts for projected sources are
-    /// rejected entirely, because their spans describe the generated module.
+    /// rejected entirely: applying checker facts after projection is not implemented.
     /// [`ModuleAnalysis::semantic_revision`] is copied from accepted facts and is
     /// `None` when they were rejected.
```

## [crates/js-oxc/src/ast.rs](../../crates/js-oxc/src/ast.rs)

```diff
--- before/crates/js-oxc/src/ast.rs
+++ after/crates/js-oxc/src/ast.rs
@@ -1,8 +1,7 @@
 //! Small bridges between Oxc spans and editable-source spans.
 //!
-//! Every helper re-tags an Oxc span with the file id of the source being analysed, so
-//! callers never mix coordinates from a generated shadow with the original. The
-//! extractor works on original text, so these spans are the ones that end up in the IR
-//! and in diagnostics.
+//! Oxc spans are tagged with the file id of the buffer being analysed. That buffer
+//! may be a normalized projection; the projection pipeline subsequently maps its
+//! local spans to the editable original. Re-tagging alone does not map coordinates.

 use oxc::ast::ast::{PropertyKey, StringLiteral};
@@ -37,6 +36,7 @@
     /// Builds an IR asset reference from an identity declared in another module.
     ///
-    /// The name span is the identity's declaration span, so a consumer that reports the
-    /// reference points at the declaration rather than at this use.
+    /// The name carries declaration offsets but is tagged with the current buffer
+    /// id. It is identity metadata paired with `module_path`, not a directly sliceable
+    /// diagnostic span in the importing module.
     pub(crate) fn asset_reference(&self, identity: &IdentityId) -> AssetReference {
         AssetReference {
```

## [crates/js-oxc/src/entry.rs](../../crates/js-oxc/src/entry.rs)

```diff
--- before/crates/js-oxc/src/entry.rs
+++ after/crates/js-oxc/src/entry.rs
@@ -52,5 +52,5 @@
 }

-/// Parses a module without checker facts.
+/// Parses a module with optional checker facts and extension-based language detection.
 pub fn parse(source: &SourceFile) -> ParsedJavaScript {
     parse_with_type_facts(source, &[], &BTreeMap::new())
@@ -60,5 +60,5 @@
 ///
 /// Facts are matched by expression span and are only as trustworthy as their enclosing
-/// [`TypeFact`]s revisions; [`OxcSourceAnalyzer::finalize`] validates those before
+/// `TypeFacts` revisions; [`OxcSourceAnalyzer::finalize`] validates those before
 /// calling here.
 pub fn parse_with_type_facts(
```

## [crates/js-oxc/src/motion/view_transition.rs](../../crates/js-oxc/src/motion/view_transition.rs)

```diff
--- before/crates/js-oxc/src/motion/view_transition.rs
+++ after/crates/js-oxc/src/motion/view_transition.rs
@@ -55,5 +55,6 @@
     /// A `.for(...)` argument is kept as a user span, never evaluated. A use that the
     /// checker already described through a type fact becomes dynamic even when only one
-    /// candidate remains, because the fact proves the value can vary across instances.
+    /// candidate remains. A singleton fact does not prove runtime variation; this is
+    /// the lowering policy for fact-backed uses.
     pub(crate) fn view_transition_use(
         &mut self,
```

## [crates/js-oxc/src/projected_scan.rs](../../crates/js-oxc/src/projected_scan.rs)

```diff
--- before/crates/js-oxc/src/projected_scan.rs
+++ after/crates/js-oxc/src/projected_scan.rs
@@ -7,5 +7,5 @@
 //! reported rather than left in generated coordinates.
 //!
-//! Checker facts are not used here: their spans describe the generated module, so
+//! Checker-fact application for projected inputs is not implemented, so
 //! [`ParsedJavaScript::type_probes`] is cleared and the analysis is intentionally
 //! fact-free.
@@ -120,6 +120,6 @@
 /// Rewrites an identity's declaration span into the original file.
 ///
-/// The identity key itself is unchanged, so the same asset resolves to the same
-/// declaration before and after projection.
+/// The module path and declaration offsets in the identity are rewritten together.
+/// The resulting key names the original declaration rather than the generated copy.
 fn map_identity(
     identity: &mut IdentityId,
```

## [crates/js-oxc/src/refs/projections.rs](../../crates/js-oxc/src/refs/projections.rs)

```diff
--- before/crates/js-oxc/src/refs/projections.rs
+++ after/crates/js-oxc/src/refs/projections.rs
@@ -3,6 +3,6 @@
 //! A projection turns a ref identity into something a style call can attach to an
 //! element or use as a container query source. Projections need the ref to be
-//! statically known, so a ref that only the checker can resolve is reported as
-//! `SHTS001` here rather than guessed.
+//! resolvable to finite candidates, from local analysis or checker facts. Unresolved
+//! refs are reported as `SHTS001` instead of being guessed.
 //!
 //! Every use is also recorded with the JSX path it appeared on, which is what the
```

## [crates/napi/src/api.rs](../../crates/napi/src/api.rs)

```diff
--- before/crates/napi/src/api.rs
+++ after/crates/napi/src/api.rs
@@ -175,6 +175,6 @@
 /// the analysis buffer. The original of module `i` and the code of a projected module
 /// are different files even when their text matches, and only the original may appear
-/// in a published span. Module ids are allocated after the source ids so a diagnostic
-/// can name its file without a lookup table.
+/// in a published local span. Original file ids occupy the first range; generated
+/// and analysis ids follow it. Module identities themselves are host path strings.
 ///
 /// Rules shared by two modules are emitted once: the first module in input order keeps
```

## [crates/napi/src/output.rs](../../crates/napi/src/output.rs)

```diff
--- before/crates/napi/src/output.rs
+++ after/crates/napi/src/output.rs
@@ -28,5 +28,5 @@
     /// Assets and resources the module references.
     pub resources: Vec<NativeModuleResource>,
-    /// Generated CSS for this module alone.
+    /// Rendered CSS for this compilation; standalone entry points include document CSS.
     pub css: String,
     /// Completely transformed source, or `None` when compilation failed.
@@ -146,18 +146,18 @@
 }

-/// One mapping from generated source to original source, in byte offsets.
+/// One mapping from generated source to original source, in UTF-16 code-unit offsets.
 #[napi(object, object_from_js = false)]
 #[derive(Serialize)]
 pub struct NativeTransformMapping {
-    /// Byte start in the original source.
+    /// UTF-16 start in the original source.
     #[napi(js_name = "original_start")]
     pub original_start: u32,
-    /// Byte end in the original source.
+    /// UTF-16 end in the original source.
     #[napi(js_name = "original_end")]
     pub original_end: u32,
-    /// Byte start in the generated source.
+    /// UTF-16 start in the generated source.
     #[napi(js_name = "generated_start")]
     pub generated_start: u32,
-    /// Byte end in the generated source.
+    /// UTF-16 end in the generated source.
     #[napi(js_name = "generated_end")]
     pub generated_end: u32,
```

## [crates/napi/src/semantic.rs](../../crates/napi/src/semantic.rs)

```diff
--- before/crates/napi/src/semantic.rs
+++ after/crates/napi/src/semantic.rs
@@ -4,6 +4,7 @@
 //! Every type here mirrors a [`stillwind_semantic`] type with the JSON shape the
 //! JavaScript host already consumes. Conversions validate what the domain type would
-//! reject anyway: a finite candidate set whose elements disagree with the value's asset
-//! kind, or a ref alternative without its permitted scopes.
+//! reject through their checked constructors: inconsistent candidate kinds or a ref
+//! alternative carrying a non-ref identity or a non-scope value. Missing scopes mean
+//! an unscoped ref and are accepted.
 //!
 //! Export and metadata collections cross the boundary as arrays of named entries rather
@@ -69,7 +70,8 @@
     #[napi(js_name = "module_path")]
     pub module_path: String,
-    /// Byte start of the declaration call.
+    /// Byte start of the declaration identity: binding-name span for named assets,
+    /// call span for styles.
     pub start: u32,
-    /// Byte end of the declaration call.
+    /// Exclusive byte end of the declaration identity span.
     pub end: u32,
     /// Lexical binding name, or the empty string for anonymous calls.
@@ -314,8 +316,8 @@
     /// Metadata carried with the asset.
     pub metadata: NativeAssetMetadata,
-    /// Byte start of the declaration call.
+    /// Byte start of the producing call.
     #[napi(js_name = "call_start")]
     pub call_start: u32,
-    /// Byte end of the declaration call.
+    /// Exclusive byte end of the producing call.
     #[napi(js_name = "call_end")]
     pub call_end: u32,
```

## [crates/napi/src/session.rs](../../crates/napi/src/session.rs)

```diff
--- before/crates/napi/src/session.rs
+++ after/crates/napi/src/session.rs
@@ -243,6 +243,7 @@
     /// Publishes a reloaded design system and reports what changed.
     ///
-    /// Recompiling retained modules can fail; the error then lists the failing modules
-    /// and the previous revision stays active.
+    /// Recompiling retained modules can fail; the error then lists the failing modules.
+    /// Watch mode retains the previous revision. Build mode has already published the
+    /// new revision with failed modules omitted when this error is returned.
     #[napi]
     pub fn reload(&mut self, loader: &NativeDesignSystemLoader) -> napi::Result<NativeReload> {
@@ -329,5 +330,5 @@
     }

-    /// Stable document CSS independent of which JS modules have been visited.
+    /// Stable document CSS without a source map, independent of visited JS modules.
     #[napi]
     pub fn base_css(&self) -> napi::Result<String> {
```

## [crates/napi/src/source_input.rs](../../crates/napi/src/source_input.rs)

```diff
--- before/crates/napi/src/source_input.rs
+++ after/crates/napi/src/source_input.rs
@@ -9,6 +9,6 @@
 //! The conversion also enforces the pairing: a projected input must carry its map,
 //! converter identity, language, and framework, and a plain input must not carry
-//! projection metadata at all. Rejecting the mismatch here keeps the domain types free
-//! of "sometimes present" combinations.
+//! projection metadata at all. This validation applies to transport conversion;
+//! domain records remain publicly constructible and need validation at use sites.

 use std::sync::Arc;
@@ -144,5 +144,5 @@
         /// Source-map JSON text.
         json: String,
-        /// Name of the generated file the map describes.
+        /// Source name identifying the original file within the source map.
         source: String,
     },
```

## [crates/napi/src/stylesheet.rs](../../crates/napi/src/stylesheet.rs)

```diff
--- before/crates/napi/src/stylesheet.rs
+++ after/crates/napi/src/stylesheet.rs
@@ -48,5 +48,5 @@
     /// Opaque name of the source that contains the import.
     pub importer: String,
-    /// Authored import specifier, exactly as written.
+    /// Decoded import specifier, without CSS quoting or escapes.
     pub specifier: String,
 }
@@ -61,7 +61,7 @@
     /// Authored url() specifier.
     pub specifier: String,
-    /// Byte start of the specifier in the owning source.
+    /// Byte start of the complete authored resource expression in the owning source.
     pub start: u32,
-    /// Byte end of the specifier in the owning source.
+    /// Exclusive byte end of the complete authored resource expression.
     pub end: u32,
 }
```

## [crates/project/src/dependencies.rs](../../crates/project/src/dependencies.rs)

```diff
--- before/crates/project/src/dependencies.rs
+++ after/crates/project/src/dependencies.rs
@@ -5,10 +5,13 @@
 //! [`CompilationDependencies`] and describe the last published artifact.
 //!
-//! Candidates, theme tokens, and assets are exact as of that artifact, so this index
-//! supports genuinely selective recompilation for them. Utilities, variants, and
-//! properties are conservative: the compiler records every definition in those three
+//! Entries reflect the compiler's recorded sets, not a proof of exact use. Candidates
+//! are syntactic mentions, tokens include static and transitive dependencies, and
+//! assets include declarations as well as external facts. Utilities, variants, and
+//! properties are especially conservative: the compiler records every definition in those three
 //! maps, because a candidate's generated content depends on which definition it resolves
 //! against and that cannot be narrowed without tracking usage during resolution. A
-//! definition change therefore reports every module as affected.
+//! change to an already indexed definition therefore includes every module. New names
+//! have no reverse entry; hosts must handle additions separately. Session reloads
+//! currently rebuild all retained analyses instead of selecting through this index.
 //!
 //! The index is not append-only. A module that stops using a definition is removed from
@@ -31,11 +34,11 @@
     /// Theme token name to the modules that reference it.
     pub theme_tokens: BTreeMap<String, BTreeSet<ModuleId>>,
-    /// Utility definition name to the modules that use it.
+    /// Utility definition name to the modules conservatively depending on it.
     pub utilities: BTreeMap<String, BTreeSet<ModuleId>>,
-    /// Variant definition name to the modules that use it.
+    /// Variant definition name to the modules conservatively depending on it.
     pub variants: BTreeMap<String, BTreeSet<ModuleId>>,
-    /// Property definition name to the modules that require it.
+    /// Property definition name to the modules conservatively depending on it.
     pub properties: BTreeMap<String, BTreeSet<ModuleId>>,
-    /// Produced asset identity to the modules that consume it.
+    /// Asset identity to modules declaring it or receiving it in external facts.
     pub assets: BTreeMap<IdentityId, BTreeSet<ModuleId>>,
 }
```

## [crates/project/src/lib.rs](../../crates/project/src/lib.rs)

```diff
--- before/crates/project/src/lib.rs
+++ after/crates/project/src/lib.rs
@@ -10,6 +10,7 @@
 //! Two state rules define the behaviour of the whole layer. A revision is published as
 //! one coherent set, so a failed watch reload retains the previous snapshot, artifacts,
-//! and CSS rather than mixing new registries with stale output; a failed initial load
-//! or build has no last-known-good value and clears the artifacts instead. And coverage
+//! and CSS rather than mixing new registries with stale output. A failed design-system
+//! load in build mode clears artifacts even if a previous revision existed. A build
+//! reload with module errors publishes the new revision with successful modules only. And coverage
 //! is only as good as the host says: observing a transform never proves that the module
 //! graph is complete, so a selection must carry an explicit [`StylesheetCoverage`].
```

## [crates/project/src/session.rs](../../crates/project/src/session.rs)

```diff
--- before/crates/project/src/session.rs
+++ after/crates/project/src/session.rs
@@ -263,6 +263,6 @@
     /// artifact, which makes repeated updates during a watch session cheap; the
     /// analysis is still replaced so later queries see the newest one. A module whose
-    /// artifact key changed is recompiled, and its previous stylesheet contribution and
-    /// dependency entries are retracted first.
+    /// artifact key changed is recompiled. Previous contributions are replaced only
+    /// after successful compilation; build-mode failure removes the old artifact.
     ///
     /// Generated identifiers are checked against every other module's symbols, not only
```

## [crates/semantic/src/module.rs](../../crates/semantic/src/module.rs)

```diff
--- before/crates/semantic/src/module.rs
+++ after/crates/semantic/src/module.rs
@@ -77,7 +77,7 @@
     /// Fingerprint of the exact source text the facts were computed from.
     pub source_revision: Fingerprint,
-    /// Fingerprint of the checker payload itself, including checker version and
-    /// configuration. Two modules with identical source but different settings must not
-    /// share cache entries, so this travels beside the source revision.
+    /// Host-provided revision of the semantic payload. Current Rust adapters hash the
+    /// facts and asset metadata; checker version and configuration are not separately
+    /// included. Hosts must refresh the payload when their checker inputs change.
     pub semantic_revision: Fingerprint,
     /// Expression facts keyed by byte span. Each span must satisfy
@@ -97,6 +97,7 @@
 /// A problem the host hit while producing type facts for a module.
 ///
-/// Integration problems do not fail the build: the module is analysed without facts and
-/// the diagnostic explains why the facts were ignored.
+/// The frontend can fall back to local analysis and record why facts were ignored.
+/// Module compilation promotes these records to `SHFACT001` errors, so they prevent
+/// publication rather than acting as non-fatal warnings.
 #[derive(Clone, Debug, Eq, PartialEq)]
 pub struct IntegrationDiagnostic {
```

## [crates/semantic/src/module_summary.rs](../../crates/semantic/src/module_summary.rs)

```diff
--- before/crates/semantic/src/module_summary.rs
+++ after/crates/semantic/src/module_summary.rs
@@ -26,5 +26,5 @@
     /// Local name in the importing module.
     pub local: String,
-    /// Module specifier text exactly as written; never resolved here.
+    /// Decoded module specifier value; never resolved here.
     pub source: String,
     /// Name imported from that module: the exported name, `default` for a default
@@ -38,5 +38,5 @@
     /// Name visible to importers of this module.
     pub exported: String,
-    /// Module specifier text exactly as written; never resolved here.
+    /// Decoded module specifier value; never resolved here.
     pub source: String,
     /// Name imported from that module (`default` or `*` as in [`ImportBinding`]).
@@ -51,5 +51,5 @@
 #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
 pub struct ModuleSpecifier {
-    /// Specifier text exactly as written.
+    /// Decoded string-literal value, without quotes or source escape spelling.
     pub source: String,
     /// Byte offset of the literal start in the original source.
@@ -61,6 +61,7 @@
 /// Everything one module publishes about itself.
 ///
-/// The fields are independent lists in source order; consumers must not sort them,
-/// because diagnostics and shadow rewriting report the authored order.
+/// Ordering is field-specific: module specifiers, imports and re-exports retain
+/// source order; assets are grouped by kind; exports are keyed by name. Demand import
+/// lists are sorted by `(source, imported, local)` by the Oxc frontend.
 #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
 pub struct ModuleSummary {
```

## [crates/semantic/src/symbolic.rs](../../crates/semantic/src/symbolic.rs)

```diff
--- before/crates/semantic/src/symbolic.rs
+++ after/crates/semantic/src/symbolic.rs
@@ -46,6 +46,7 @@
 /// A sound abstract value containing identities of exactly one asset kind.
 ///
-/// The [`Self::kind`] and the identities' kinds agree by construction, so consumers may
-/// rely on a value never mixing asset kinds.
+/// [`Self::finite`] checks candidate kinds and [`Self::exact`] derives the kind from
+/// its identity. Public fields and derived deserialization can bypass those checks;
+/// consumers of externally constructed values must validate them.
 #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
 pub struct SymbolicValue {
@@ -133,6 +134,6 @@
     /// Builds one correlated alternative, rejecting mismatched kinds.
     ///
-    /// Kinds are checked here rather than at use sites so a bad correlation cannot
-    /// travel between modules.
+    /// Checks the reference kind and the scope value's outer kind. It does not
+    /// revalidate the identities inside a publicly constructed scope value.
     ///
     /// # Errors
```

## [crates/source/src/coordinates.rs](../../crates/source/src/coordinates.rs)

```diff
--- before/crates/source/src/coordinates.rs
+++ after/crates/source/src/coordinates.rs
@@ -103,7 +103,7 @@
     /// that position.
     ///
-    /// A column that lands on or past the line's terminating newline is invalid, so a
-    /// successful result always names a character on the requested line. The empty line
-    /// after a trailing newline is addressable at column zero.
+    /// The terminating newline itself is addressable; a column reaching the next
+    /// line's start is rejected. EOF is also addressable, including column zero on
+    /// the empty line after a trailing newline.
     ///
     /// # Errors
```

## [crates/source/src/diagnostics.rs](../../crates/source/src/diagnostics.rs)

```diff
--- before/crates/source/src/diagnostics.rs
+++ after/crates/source/src/diagnostics.rs
@@ -2,6 +2,6 @@
 //!
 //! Both types carry a stable machine-readable name (`code` for errors, `rule` for
-//! findings) as `&'static str`, so the set of codes is closed at compile time and hosts
-//! can switch on them instead of parsing the message. Each diagnostic additionally
+//! findings) as `&'static str`, so hosts can inspect codes instead of parsing messages.
+//! These strings are not a closed enum; callers can construct additional codes. Each diagnostic additionally
 //! carries ordered `labels` that point into the `SourceFile` which produced it, plus
 //! free-form `notes`.
```

## [crates/source/src/fingerprint.rs](../../crates/source/src/fingerprint.rs)

```diff
--- before/crates/source/src/fingerprint.rs
+++ after/crates/source/src/fingerprint.rs
@@ -12,5 +12,5 @@
 ///
 /// A fingerprint says nothing about where the content came from or how it is named;
-/// equal bytes give equal fingerprints and nothing else does. The ordering is
+/// equal bytes give equal fingerprints; collision resistance is not a proof of equality. The ordering is
 /// lexicographic over the hex text so fingerprints can key a `BTreeMap`
 /// deterministically.
```

## [crates/source/src/lint.rs](../../crates/source/src/lint.rs)

```diff
--- before/crates/source/src/lint.rs
+++ after/crates/source/src/lint.rs
@@ -6,6 +6,6 @@
 //! of its keyword, and the rule names it names; an empty rule list means every rule.
 //!
-//! [`filter_lint_findings`] runs once, after all analysis is complete, over the
-//! findings of every pass together; suppression therefore cannot depend on which pass
+//! The compiler filters analysis findings before lowering and filters the combined
+//! findings again after emission; suppression therefore cannot depend on which pass
 //! noticed a problem. A finding is judged by its first label, and a finding without
 //! labels is treated as covering the whole file.
@@ -122,6 +122,6 @@
 /// A comment yields at most one directive, and only from its first `swlint-`
 /// occurrence; if the text after that occurrence names no keyword, the comment
-/// contributes nothing. Prose that merely mentions a directive is therefore ignored
-/// rather than reported.
+/// contributes nothing. There is no requirement that the keyword start the comment:
+/// prose containing a recognized directive spelling is parsed too.
 pub fn collect_lint_directives(source: &str) -> Vec<LintDirective> {
     let bytes = source.as_bytes();
```

## [crates/source/src/projection.rs](../../crates/source/src/projection.rs)

```diff
--- before/crates/source/src/projection.rs
+++ after/crates/source/src/projection.rs
@@ -7,6 +7,6 @@
 //! them.
 //!
-//! Mapping here is verified, not assumed: a query succeeds only when the mapped
-//! original text equals the projected text. Converters rewrite code, so a
+//! Exact-span verification succeeds only when the mapped original text equals the
+//! retained projected text. Coordinate lookup alone does not verify text equality. Converters rewrite code, so a
 //! correspondence that merely looks plausible must fail rather than produce an edit
 //! against the wrong characters.
@@ -87,7 +87,7 @@
 ///
 /// Both halves are half-open spans in their own [`SourceFile`]; the original span
-/// already carries verified, byte-for-byte identical text, so callers do not re-check
-/// the correspondence. Provenance is never merged with a neighboring one: adjacent
-/// spans may show that a converter reordered code.
+/// returned by exact verification contains the same text. Fragment verification may
+/// omit synthetic gaps inside the projected envelope. Public construction and
+/// deserialization do not validate this correspondence.
 #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
 pub struct SourceProvenance {
@@ -102,5 +102,5 @@
 /// Only pairs verified at construction are stored, so interior byte offsets can be
 /// translated by simple addition. Ranges are sorted by generated start and may be
-/// adjacent, but never overlap after verification.
+/// adjacent or overlapping. Queries reject disagreement between covering mappings.
 #[derive(Clone, Debug)]
 struct LinearRange {
@@ -128,7 +128,7 @@
     /// Loads `map` and validates it against both sources.
     ///
-    /// Stale `sourcesContent`, Volar arrays of differing lengths, coordinates that do
-    /// not land on scalar boundaries and truncated surrogate tokens are all rejected
-    /// here, so later queries can be cheap.
+    /// Rejects stale `sourcesContent`, mismatched Volar arrays, and malformed
+    /// coordinates. Matching v3 tokens inside the same surrogate scalar are accepted
+    /// but skipped: they cannot serve as byte-oriented edit anchors.
     ///
     /// # Errors
@@ -329,6 +329,6 @@
     /// Translates one generated byte offset to its original byte offset.
     ///
-    /// Returns `None` unless the offset is a character boundary, is covered by exactly
-    /// one point or interval, and every mapping that covers it agrees. `None` means
+    /// Returns `None` unless the offset is a character boundary, is covered by
+    /// at least one point or interval, and every mapping that covers it agrees. `None` means
     /// unmapped and must not be treated as the start of the file or as the nearest
     /// mapped neighbor.
```

## [crates/source/src/text.rs](../../crates/source/src/text.rs)

```diff
--- before/crates/source/src/text.rs
+++ after/crates/source/src/text.rs
@@ -81,5 +81,6 @@
 ///
 /// Ids are assigned per run and are not stable across processes, so they must never
-/// appear in generated output, cache keys or diagnostics that outlive the run. Anything
+/// be used alone as durable identity. In-process artifact keys may include them to
+/// distinguish source buffers; persisted records need a durable source name. Anything
 /// that needs a durable name should use [`normalized_module_path`].
 #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
@@ -97,5 +98,6 @@
 /// `start == end` is a valid empty span that addresses a position rather than a
 /// character. Construction validates nothing, so a span may be reversed or out of
-/// range; [`SourceFile::slice`] and [`SourceFile::position`] are what reject it.
+/// range; [`SourceFile::slice`] validates the whole span. [`SourceFile::position`]
+/// validates only the byte offset supplied to it.
 #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
 pub struct Span {
@@ -134,6 +136,6 @@
 /// A value together with the source range it was parsed from.
 ///
-/// The span is provenance, not part of the value's identity: two equal values parsed
-/// from different ranges compare equal.
+/// Derived equality compares both the value and the span. Equal values from different
+/// source ranges therefore compare unequal.
 #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
 pub struct Spanned<T> {
@@ -251,7 +253,6 @@
     /// Converts a byte offset to the UTF-16 position expected by editor clients.
     ///
-    /// Columns count UTF-16 code units from the start of the line, so a position can
-    /// name a byte offset that is not a character boundary for the editor as long as
-    /// the offset itself is one.
+    /// Columns count UTF-16 code units from the start of the line. Only UTF-8 scalar
+    /// boundaries are accepted; supplementary characters advance the column by two.
     ///
     /// # Errors
```

## [crates/syntax/src/lexer.rs](../../crates/syntax/src/lexer.rs)

```diff
--- before/crates/syntax/src/lexer.rs
+++ after/crates/syntax/src/lexer.rs
@@ -2,8 +2,7 @@
 //!
 //! Input is a [`Spanned<String>`] the host already decoded from JavaScript: the span
-//! covers the string literal in the original source and the value is its decoded text.
-//! Tokens come back in source order, split on Unicode whitespace, and each token span
-//! is the literal span shifted by the token offset inside the value, so a token span
-//! can be sliced out of the original source again.
+//! normally covers the literal contents and the value is decoded text. Tokens are
+//! split on Unicode whitespace in value order. Spans add decoded byte offsets to the
+//! supplied start; escapes are not mapped back to their original source spelling.
 //!
 //! The lexer performs no trimming, unescaping or normalization: [`UtilityToken::raw`]
@@ -20,5 +19,5 @@
 #[derive(Clone, Debug, Eq, PartialEq)]
 pub struct UtilityToken {
-    /// Exact text of the token, with escapes still in place.
+    /// Exact substring of the supplied value; this lexer performs no unescaping.
     pub raw: String,
 }
@@ -27,7 +26,7 @@
 /// span for every token.
 ///
-/// Spans are relative to the input literal, so all of them stay inside `input.span`,
-/// and concatenating the returned tokens in order reproduces every non-whitespace
-/// character of the value in order.
+/// Spans use value-relative offsets from `input.span.start`. The supplied end is not
+/// used to bound tokens, so callers must provide a sufficiently long span. Exact
+/// original-source correspondence additionally requires unchanged literal spelling.
 ///
 /// # Errors
```

## [crates/syntax/src/syntax.rs](../../crates/syntax/src/syntax.rs)

```diff
--- before/crates/syntax/src/syntax.rs
+++ after/crates/syntax/src/syntax.rs
@@ -9,6 +9,7 @@
 //! The parse is lossless in both directions: `raw` keeps the exact source spelling,
 //! escapes included, for hashing and diagnostics, while arbitrary values keep their
-//! decoded text for evaluation. Unbalanced delimiters, empty segments and dangling
-//! separators are rejected as a whole; no input is partially accepted.
+//! decoded text for evaluation. Unbalanced delimiters and empty variant segments
+//! are rejected. Arbitrary-property names and values receive no semantic validation
+//! here, and may be empty.

 /// How a variant prefix was written.
@@ -159,8 +160,8 @@
 }

-/// Splits the candidate into the utility and the variant prefixes before it.
-///
-/// A `[...]`-wrapped utility is an arbitrary property only when it contains a top-level
-/// `:`; otherwise it is treated as a named utility whose name happens to be bracketed,
+/// Parses the utility after variant prefixes have been removed.
+///
+/// A `[...]`-wrapped utility is an arbitrary property when its inner text contains a
+/// `:`; the first colon is used without checking its nesting depth; otherwise it is treated as a named utility whose name happens to be bracketed,
 /// leaving the registry to resolve it.
 fn parse_utility(raw: &str) -> Option<Utility> {
```

## [crates/test-support/css.rs](../../crates/test-support/css.rs)

```diff
--- before/crates/test-support/css.rs
+++ after/crates/test-support/css.rs
@@ -1,4 +1,4 @@
 //! Explicit CSS fixtures for compiler tests that exercise the standard package. This
-//! module is included only by tests, never by a production host.
+//! module is used by test and example targets rather than production hosts.
 //!
 //! The helpers read the real `packages/core` CSS from disk instead of embedding copies,
@@ -8,6 +8,6 @@
 //! ignored.
 //!
-//! Nothing here is available to production code; the module is pulled in through
-//! `#[path]` under `#[cfg(test)]`.
+//! This is a fixture helper included through `#[path]` by test and example targets;
+//! it is not part of the library API.

 use std::fmt::Write;
```

## [experiments/source-map-research/native/src/main.rs](../../experiments/source-map-research/native/src/main.rs)

```diff
--- before/experiments/source-map-research/native/src/main.rs
+++ after/experiments/source-map-research/native/src/main.rs
@@ -11,6 +11,6 @@
 // the same text is written as regular comments on purpose.
 //
-// Coordinates here are UTF-16 units, matching the JavaScript consumer, and the
-// emitter is replacement-only: it never invents text that has no range.
+// Source-map coordinates count UTF-16 units; edit ranges count UTF-8 bytes. The
+// emitter also accepts an unmapped prefix before the edited source.

 use std::{
```
