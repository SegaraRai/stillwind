# Experimental runtime CSS conditions

`--if(--condition, yes, no)` chooses a utility value at computed-value time.
`if(--condition):utility` conditionally applies a utility. Neither changes the
element's class list when the condition changes.

```tsx
import { style } from "stillwind/react";

const props = style("grid if(--enabled):flex w-[--if(--enabled,20rem,10rem)]", {
  "--enabled": enabled,
});
```

Boolean conditions are **unregistered custom-property names**. Stillwind's boolean
serialization supplies one space for true and `initial` for false. A missing
property is false; inherited values follow ordinary CSS inheritance. Strings
such as `"true"`, numeric flags and media queries are not boolean
conditions. Native value comparisons use the separate `--name:value` form below. `@property` registration changes the initial-value contract and must
not be used for these flags.

One leading `!` negates a condition: `--if(!--enabled,20px,10px)` and
`if(!--enabled):flex`. Two or more leading exclamation marks are rejected.
Negated value conditions swap the branches in both strategies. **Negated variants
require `useCssIf: true`**: swapping their branches would store `revert-rule` in
a fallback helper, which cannot preserve that keyword's meaning.

Native mode also supports value comparisons: `if(--mode:compact):flex` and
`--if(--mode:compact,20px,10px)` lower to `style(--mode:compact)` tests. Set
`useCssIf: true`; fallback mode reports a compilation error. Comparison values
must be nonempty, and commas must be inside a CSS function or quoted string.
Matching follows native CSS style-query semantics, including any `@property`
registration. `if(!--mode:compact):flex` negates the comparison; comparisons can
also be combined with boolean conditions using repeated variants.

Repeated variants form an AND: `if(--a):if(--b):flex`. Ordinary variants compose
with them, for example `hover:if(--enabled):flex`. The condition is evaluated on
the styled element (or pseudo-element), not a referenced ancestor. A condition
inside a `ref` wrapper is rejected; put `if(...)` outside that wrapper to test
the target instead.

Value conditionals can nest and appear inside expressions:

```tsx
style("w-[calc(--if(--wide,20,10)*1px)]");
style("bg-[--if(--enabled,red,blue)]");
style("w-[--if(--a,--if(--b,30px,20px),10px)]");
```

Both branches participate in typed utility selection. Use the usual explicit
hint when ambiguous, such as `bg-[image:--if(--enabled,var(--image),none)]`.
Wrap a branch containing top-level commas in braces: `--if(--a,{Arial,sans-serif},serif)`.
Strings, URLs, nested function arguments, and comments keep their lexical boundaries.
Authored declaration values and `@variant if(--enabled) { ... }` use the same
lowering. `@apply grid if(--enabled):flex;` keeps the unconditional declaration
before its conditional rule.

## Output strategy

```ts
stillwind({ css: "./src/root.css", useCssIf: true });
```

`useCssIf` defaults to `false`. It is also available in native compiler options;
Rust uses `CompilerOptions::use_css_if`, and the CLI accepts `--use-css-if`.
The option applies to both values and variants and participates in module cache
identity.

| Input                    | Default fallback strategy                                            | `useCssIf: true`                                  |
| ------------------------ | -------------------------------------------------------------------- | ------------------------------------------------- |
| `--if(--flag,20px,10px)` | Local helper `--sw-if-…:var(--flag) 20px;` and `var(--sw-if-…,10px)` | `if(style(--flag):20px;else:10px)`                |
| `if(--flag):flex`        | Same helper pattern, with `revert-rule` as the false branch          | `display:if(style(--flag):flex;else:revert-rule)` |

Helper names are deterministic and honor the configured Stillwind prefix.
They are defined next to their consumers to avoid inheriting a helper value
already computed on another element. Conditions must not depend on the
declaration they guard; normal CSS variable cycle rules still apply.

Fallback mode requires the branch used when the underlying property is true
(the false branch for `!--condition`) to resolve to a valid custom
property token value. An invalid `var()` in that branch can select the false
branch instead. CSS-wide values in that branch (`initial`, `inherit`, `unset`, `revert`,
`revert-layer`, `revert-rule`) require native mode and literal occurrences are
reported as compilation errors. Indirect values that become CSS-wide keywords
have the same limitation. Fallback substitution is not a general replacement
for native `if()`'s branch evaluation semantics.

## Browser and minifier requirements

Conditional **variants require `revert-rule` in both strategies**. Native mode
additionally requires CSS `if()` and its style conditions. This feature is
experimental and is not supported by every browser. The compiler adds a
`@supports (x:revert-rule)` gate around conditional rules; browsers without
that feature retain the unconditional styling.

Conditional rules share a support-query name within each nonconflicting interval
of their owner's emission order: `@supports (x:revert-rule) and (--<hash>:)`.
A property conflict starts a new interval, including shorthand/longhand and
logical/physical overlap. Each `style()` call owns its intervals; authored CSS
uses the outer rule, sharing state across expanded `@apply`, `@variant`, and nested
rules. The default name hashes the owner and interval without a prefix. These
names use a separate `rule-boundary` collision pool and may share spelling with
actual custom properties.

`revert-rule` discards the current rule's declarations for the affected property.
Merging conflicting rules would also discard a declaration the false branch
should fall back to. Custom minifier pipelines must preserve distinct support
queries. Value conditionals alone do not need `revert-rule`
unless a branch explicitly requests it.

Run `vp -C e2e/frameworks run browser:runtime-if` from the repository. The task builds
its compiler dependencies. The
acceptance script checks both strategies after a minified Vite production build
in installed Chrome and reports the browser version and required feature support.

## CSS registration

The package registers the public variant name and its ordering position in CSS:

```css
@stillwind-variant if\(*\) (value: [if-condition]) {
  @stillwind-intrinsic builtin-if-variant;
}
```

`[if-condition]` validates the captured presence test, negation, or value comparison.
The compiler intrinsic lowers it to a computed-value condition. The complete body
must be that intrinsic statement; extra declarations and unknown intrinsics reject.
The capture preserves CSS text, including slashes in comparison values.

Names belong to the CSS registry. The same definition can register `when\(*\)`
instead, and an absent registration does not make `if(...)` a built-in name in
Rust. Literal prefix and suffix text surround the single unescaped `*`; escaped
asterisks are literal. Ordinary CSS overrides keep their normal registry precedence.
`--if(...)` remains a compiler-defined value function, independent of variant registration.
