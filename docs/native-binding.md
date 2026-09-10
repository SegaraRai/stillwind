# Native Node API

The Node binding accepts JavaScript objects and arrays and returns typed objects. Compiler options, source inputs, diagnostics, CSS loading steps, stylesheet plans, and semantic values use napi-rs objects and enums. There is no JSON-string transport or compatibility overload. CSS, source text, and serialized v3 source maps remain strings because those are their actual data formats.

```ts
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

const loader = new NativeDesignSystemLoader({ classIdentity: "module" });
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

The binding types live in `crates/napi`; Rust domain crates remain independent of napi-rs. The build generates [binding.d.ts](../packages/native/binding.d.ts) using `--no-const-enum`, so native string enums become TypeScript literal unions. [index.d.ts](../packages/native/index.d.ts) re-exports those declarations and adds the JavaScript loader's `nativeBindingPath`. The host reuses the generated types instead of asserting the shape of parsed JSON.

Semantic candidates are discriminated unions: `{kind: "universe"}` or `{kind: "finite", values: [...]}`. Summary `exports` and `export_imports` are named entry arrays. This preserves all source-level names, including `__proto__`, which napi-rs's ordinary object-map conversion otherwise treats as a prototype setter. Domain summary serialization is unchanged; these are Node transport types.

Run these tasks from the repository root:

- `vp run @stillwind/native#check`: native build, generated declaration typechecks, and public JavaScript API tests. Invalid former JSON inputs, enum values, and malformed nested records must fail.
- `vp run @stillwind/host#test` and `vp run @stillwind/vite#test`: host source analysis, type facts, CSS maps, and Vite integration.
- `vp run @stillwind/bundler#test` and `vp run @stillwind/vite#test:frameworks`: webpack/Rspack and official SFC builds.

The native build regenerates `binding.d.ts`; do not edit it by hand. Only generated declarations are excluded from formatting. Authored binding code and the declaration typecheck remain checked.

Session compilation updates producers without rendering the project stylesheet. `remove(module)` returns whether that producer was registered, including when another producer keeps the same shared CSS alive. Query `css()` or `planStylesheet()` to render CSS; `reload()` still reports the exact project-level `cssChanged` result across the revision boundary.

`compileReactProject()` and session `compile()` return unwrapped module CSS. They do not assemble document CSS. Assembly errors (such as a missing utilities slot or an unresolved authored theme reference) are reported by stylesheet rendering APIs. Standalone `compileReact()` still assembles the document stylesheet and returns assembly failures as `SHCSS001` diagnostics with no generated code. The Rust equivalents are `CompiledModule::to_module_output` for fragments and `StylesheetRenderer::render_compilation` for standalone output.

The implementation follows napi-rs's [object](https://napi.rs/docs/concepts/object) and [enum](https://napi.rs/docs/concepts/enum) conversion contracts.
