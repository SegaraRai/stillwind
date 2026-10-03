# Native Node API

The Node binding accepts JavaScript objects and arrays and returns typed objects. Compiler options, source inputs, diagnostics, CSS loading steps, stylesheet plans, and semantic values use napi-rs objects and enums. There is no JSON-string transport or compatibility overload. CSS, source text, and serialized v3 source maps remain strings because those are their actual data formats.

The stateless entrypoints are `compileSource`, `compileProject`, `summarizeSource`,
and `renderStylesheet`. They use the framework imports in each source to determine
its output format. `generateVocabularyTypes` augments `stillwind/macro` with the
theme's shared utility and variant vocabulary.

```ts
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

const loader = new NativeDesignSystemLoader({});
const step = loader.provide(
  0,
  "root.css",
  "@utility flex { display:flex; } @stillwind-slot utilities;",
);
if (step.kind !== "complete") throw new Error("Resolve step.requests before creating a session");
const session = new NativeProjectSession(loader, true);
const result = session.compile([
  {
    path: "button.ts",
    source: "import {style} from 'stillwind/react'; export const props=style('flex');",
  },
]);
const stylesheet = session.planStylesheet({
  modules: ["button.ts"],
  coverage: { kind: "observed" },
  placement: "document",
});
console.log(result.modules[0].code, stylesheet.css);
```

The binding types live in `crates/napi`; Rust domain crates remain independent of napi-rs. The build generates [binding.d.ts](../packages/native/binding.d.ts) using `--no-const-enum`, so native string enums become TypeScript literal unions. The package exposes these declarations directly and uses napi-rs's generated ESM loader. The loader prefers native addons and falls back to WASI. `NAPI_RS_FORCE_WASI=error` requires WASI. Browser imports use the generated browser entry and the separate WASI package. The host reuses the generated types instead of asserting the shape of parsed JSON.

Semantic candidates are discriminated unions: `{kind: "universe"}` or `{kind: "finite", values: [...]}`. Summary `exports` and `export_imports` are named entry arrays. This preserves all source-level names, including `__proto__`, which napi-rs's ordinary object-map conversion otherwise treats as a prototype setter. Domain summary serialization is unchanged; these are Node transport types.

Run these tasks from the repository root:

- `vp run --filter @stillwind/native test`: native build, generated declaration typechecks, and public JavaScript API tests. Invalid former JSON inputs, enum values, and malformed nested records must fail.
- `vp run test:wasm`: shared API contracts through the root loader with WASI required, fallback with packed packages, production bundling, and browser compilation with Vitest Browser Mode.
- `vp run @stillwind/host#test` and `vp run @stillwind/integration#test:vite`: host source analysis, type facts, CSS maps, and Vite integration.
- `vp run @stillwind/integration#test:bundlers` and `vp run @stillwind/integration#test:frameworks`: webpack/Rspack and official SFC builds.

The native build regenerates `binding.d.ts`; do not edit it by hand. Only generated declarations are excluded from formatting. Authored binding code and the declaration typecheck remain checked.

`build:binding` generates the Rust Webref and Tailwind property tables before
compiling into `.build/native`. `build` then installs changed artifacts into
`dist` and the package's `binding.d.ts`. The WASM build has the same Rust data
prerequisites and writes to `dist/wasm`.

Run `vp run @stillwind/native#generate:platform-packages` after changing the
configured targets or the napi-rs CLI's runtime dependencies. This task generates
package metadata independently of compilation. `collect:local` builds and
collects the host native and WASM artifacts. Release jobs build every configured
target before selecting `collect:release`, which collects the full target set.

Session compilation updates producers without rendering the project stylesheet. `remove(module)` returns whether that producer was registered, including when another producer keeps the same shared CSS alive. Query `css()` or `planStylesheet()` to render CSS; `reload()` still reports the exact project-level `cssChanged` result across the revision boundary.

`compileProject()` and session `compile()` return unwrapped module CSS. They do not assemble document CSS. Assembly errors (such as a missing utilities slot or an unresolved authored theme reference) are reported by stylesheet rendering APIs. Standalone `compileSource()` still assembles the document stylesheet and returns assembly failures as distinct CSS diagnostics with no generated code. The Rust equivalents are `CompiledModule::to_module_output` for fragments and `StylesheetRenderer::render_compilation` for standalone output.

The implementation follows napi-rs's [object](https://napi.rs/docs/concepts/object) and [enum](https://napi.rs/docs/concepts/enum) conversion contracts.
