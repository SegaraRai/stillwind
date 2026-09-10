import assert from "node:assert/strict";
import test from "node:test";

void test("native object transport returns typed values and rejects the former JSON protocol", async () => {
  const {
    compileReact,
    compileReactProject,
    summarizeReact,
    NativeDesignSystemLoader,
    NativeProjectSession,
  } = await import("../index.js");
  const source = "import {style} from 'stillwind/react'; export const props=style('flex');";
  const css = "@utility flex {display:flex;} @stillwind-slot utilities;";
  const output = compileReact(source, "🧭.tsx", css, { classIdentity: "module" });
  assert.equal(typeof output, "object");
  assert.equal(Object.getPrototypeOf(output), Object.prototype);
  assert.deepEqual(output.diagnostics, []);
  assert.match(output.code, /className/);
  assert.equal(typeof output.mappings[0].original_start, "number");
  const sources = [{ path: "🧭.tsx", source }];
  assert.deepEqual(
    compileReactProject(sources, css).modules.map((module) => module.path),
    ["🧭.tsx"],
  );
  const summary = summarizeReact(
    "import {style} from 'stillwind/react'; export const scope=style.scope();",
    "🧭.tsx",
  );
  const scope = summary.exports.find((entry) => entry.name === "scope").value;
  assert.equal(scope.kind, "scopes");
  assert.equal(scope.value.candidates.kind, "finite");
  assert.equal(scope.value.candidates.values[0].module_path, "🧭.tsx");
  const reserved = summarizeReact(
    "import {style} from 'stillwind/react'; export const __proto__=style.scope();",
    "reserved.ts",
  );
  assert.equal(reserved.exports[0].name, "__proto__");
  assert.equal(reserved.export_imports[0].name, "__proto__");
  assert.throws(() => compileReactProject("[]", css), /array/i);
  assert.throws(() => compileReact(source, "x.ts", css, "{}"));
  assert.throws(() => new NativeDesignSystemLoader('{"prefix":"--x-"}'));
  assert.throws(() => new NativeDesignSystemLoader({ classIdentity: "invalid" }), /classIdentity/);
  const loader = new NativeDesignSystemLoader();
  assert.equal(loader.provide(0, "root", css).kind, "complete");
  const session = new NativeProjectSession(loader, false);
  assert.deepEqual(session.compile(sources).modules[0].diagnostics, []);
  assert.throws(() => session.compile("[]"), /array/i);
  assert.throws(() => session.compile([{ ...sources[0], language: "unknown" }]), /language/);
  assert.throws(
    () =>
      session.planStylesheet({ modules: [], coverage: { kind: "unknown" }, placement: "document" }),
    /kind|variant/,
  );
  assert.throws(() =>
    session.planStylesheet({ modules: [], coverage: { kind: "complete" }, placement: "document" }),
  );
  assert.throws(
    () =>
      session.planStylesheet({ modules: [], coverage: { kind: "observed" }, placement: "unknown" }),
    /placement/,
  );
  const plan = session.planStylesheet({
    modules: ["🧭.tsx"],
    coverage: { kind: "observed" },
    placement: "document",
  });
  assert.equal(typeof plan, "object");
  assert.match(plan.css, /display:flex/);
  assert.equal(plan.producers[0].module, "🧭.tsx");
  assert(Array.isArray(session.baseCssWithMap().sourceMap.entries));
});

void test("removal reports producer presence independently of shared CSS", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import("../index.js");
  const loader = new NativeDesignSystemLoader({ classIdentity: "content" });
  loader.provide(0, "root.css", "@utility flex {display:flex;} @stillwind-slot utilities;");
  const session = new NativeProjectSession(loader, true);
  const source = "import {style} from 'stillwind/react'; export const props=style('flex');";
  session.compile([
    { path: "a.ts", source },
    { path: "b.ts", source },
  ]);
  const shared = session.css();
  assert.equal(session.remove("a.ts"), true);
  assert.equal(session.css(), shared);
  assert.equal(session.remove("a.ts"), false);
  assert.equal(session.remove("b.ts"), true);
  assert.notEqual(session.css(), shared);
});

void test("native semantic unions validate nested asset kinds and candidate payloads", async () => {
  const { compileReactProject } = await import("../index.js");
  const compile = (value) =>
    compileReactProject(
      [{ path: "facts.ts", source: "export {};", type_facts: [{ start: 0, end: 0, value }] }],
      "",
    );
  const scope = { kind: "scope", module_path: "facts.ts", start: 0, end: 1, binding_name: "scope" };
  const symbolic = (kind) => ({ kind, candidates: { kind: "universe" } });
  for (const value of [
    { kind: "scopes", value: { kind: "scope", candidates: { kind: "finite", values: [scope] } } },
    { kind: "scopes", value: symbolic("scope") },
    { kind: "animations", value: symbolic("animation") },
    { kind: "view-transitions", value: symbolic("view-transition") },
    {
      kind: "refs",
      value: [{ reference: { ...scope, kind: "ref" }, permitted_scopes: symbolic("scope") }],
    },
  ])
    assert.deepEqual(compile(value).modules[0].diagnostics, []);
  for (const value of [
    { kind: "scopes", value: symbolic("ref") },
    {
      kind: "scopes",
      value: {
        kind: "scope",
        candidates: { kind: "finite", values: [{ ...scope, kind: "animation" }] },
      },
    },
    { kind: "refs", value: [{ reference: scope, permitted_scopes: null }] },
    {
      kind: "refs",
      value: [{ reference: { ...scope, kind: "ref" }, permitted_scopes: symbolic("animation") }],
    },
    { kind: "scopes", value: { kind: "scope", candidates: { kind: "finite" } } },
    { kind: "unknown", value: symbolic("scope") },
  ])
    assert.throws(() => compile(value));
});

void test("projected session edits original SFC text and preserves watch recovery", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import("../index.js");
  const loader = new NativeDesignSystemLoader();
  assert.equal(
    loader.provide(0, "root", "@utility flex { display:flex; } @stillwind-slot utilities;").kind,
    "complete",
  );
  const session = new NativeProjectSession(loader, true);
  const source = "import {style} from 'stillwind/svelte'; const props = style('flex');";
  const prefix = "<!-- 🧭 -->\n<script>";
  const suffix = "</script><div {...props}></div><style>/* preserved */</style>";
  const input = {
    path: "Component.svelte",
    source,
    language: "ts",
    framework: "Svelte",
    fw_code: prefix + source + suffix,
    fw2js_map: {
      kind: "volar",
      mappings: [
        { sourceOffsets: [prefix.length], generatedOffsets: [0], lengths: [source.length] },
      ],
    },
    converter: { name: "svelte2tsx", version: "0.7.61", options_fingerprint: "test" },
    import_realms: [{ id: 0, start: prefix.length, end: prefix.length + source.length }],
    regions: [{ start: prefix.length, end: prefix.length + source.length, import_realm: 0 }],
  };
  const before = session.compile([input]).modules[0];
  assert.deepEqual(before.diagnostics, []);
  assert(before.code.startsWith(prefix));
  assert(before.code.endsWith(suffix));
  assert(before.code.includes("{class:"));
  assert(
    before.mappings.some(
      (mapping) => mapping.original_start === prefix.length + source.indexOf("style('"),
    ),
  );
  assert(!before.code.includes("stillwind/svelte"));
  const css = session.css();
  const missingRealm = session.compile([{ ...input, import_realms: [], regions: [] }]).modules[0];
  assert.equal(missingRealm.diagnostics[0].code, "SHIMPORT001");
  assert.equal(session.css(), css);
  const invalid = { ...input, fw2js_map: { kind: "volar", mappings: [] } };
  const failed = session.compile([invalid]).modules[0];
  assert.equal(failed.diagnostics[0].code, "SHPRJ001");
  assert.equal(session.css(), css);
  const recovered = session.compile([input]).modules[0];
  assert.deepEqual(recovered.diagnostics, []);
  assert.equal(recovered.code, before.code);
  assert.throws(() => session.compile([{ ...input, fw2js_map: undefined }]), /fw2js_map/);
});

void test("loads the current platform package through the public loader", async () => {
  const binding = await import("../index.js");
  assert.equal(typeof binding.compileReact, "function");
  assert.equal(typeof binding.compileReactProject, "function");
});

void test("session reload applies code generation options independently of CSS definitions", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import("../index.js");
  const load = (classNamespace) => {
    const loader = new NativeDesignSystemLoader({ classIdentity: "module", classNamespace });
    const result = loader.provide(
      0,
      "root",
      "@utility flex { display:flex; } @stillwind-slot utilities;",
    );
    assert.equal(result.kind, "complete");
    return loader;
  };
  const session = new NativeProjectSession(load("first"), true);
  const sources = [
    {
      path: "button.tsx",
      source: "import { style } from 'stillwind/react'; const x = style('flex');",
    },
  ];
  const before = session.compile(sources);
  const beforeCss = session.css();
  const update = session.reload(load("second"));
  assert.equal(update.cssChanged, true);
  assert.deepEqual(update.transformedModules, ["button.tsx"]);
  assert.notEqual(session.css(), beforeCss);
  const after = session.compile(sources);
  assert.notEqual(before.modules[0].code, after.modules[0].code);
  assert.deepEqual(after.modules[0].diagnostics, []);
});

void test("native sessions retain shared rules and replace CSS revisions", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import("../index.js");
  const loader = new NativeDesignSystemLoader();
  assert.deepEqual(
    loader.provide(
      0,
      "opaque root",
      "@import 'tokens'; @utility bg-brand {background-color:var(--color-brand)} @stillwind-slot utilities;",
    ),
    {
      kind: "needFile",
      requests: [{ id: 1, importer: "opaque root", specifier: "tokens" }],
    },
  );
  assert.equal(
    loader.provide(1, "opaque child", "@theme { --color-brand: red; }").kind,
    "complete",
  );
  const session = new NativeProjectSession(loader, true);
  const source = "import { style } from 'stillwind/react'; const x = style('bg-brand');";
  const result = session.compile([
    { path: "a.tsx", source },
    { path: "b.tsx", source },
  ]);
  assert.equal(result.modules.length, 2);
  assert.ok(result.modules.every((module) => module.diagnostics.length === 0));
  const before = session.css();
  assert.ok(before.length > 0);
  assert.equal(session.remove("a.tsx"), true);
  assert.equal(session.css(), before);
  const next = new NativeDesignSystemLoader();
  next.provide(
    0,
    "root",
    "@theme { --color-brand: blue; } @utility bg-brand {background-color:var(--color-brand)} @stillwind-slot utilities;",
  );
  assert.equal(session.reload(next).cssChanged, true);
  assert.notEqual(session.css(), before);
  assert.equal(session.remove("b.tsx"), true);
});

void test("native loader accepts partial and reversed batches", async () => {
  const { NativeDesignSystemLoader } = await import("../index.js");
  const loader = new NativeDesignSystemLoader();
  const first = loader.provideMany([{ id: 0, name: "root", content: "@import 'a'; @import 'b';" }]);
  assert.deepEqual(
    first.requests.map((request) => request.id),
    [1, 2],
  );
  const next = loader.provide(2, "b", "@import 'c';");
  assert.deepEqual(
    next.requests.map((request) => request.id),
    [1, 3],
  );
  const done = loader.provideMany([
    { id: 3, name: "c", content: "@theme { --color-child: green; }" },
    {
      id: 1,
      name: "a",
      content: "@theme { --color-brand: red; } @utility brand {color:var(--color-brand)}",
    },
  ]);
  assert.equal(done.kind, "complete");
  assert.deepEqual(new Set(done.sources), new Set(["root", "a", "b", "c"]));
  assert.match(loader.generateReactTypes(), /brand/);
  assert.throws(() => loader.provideMany([]), /after completion/);
});

void test("native watch reload reports compilation errors without publishing a partial revision", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import("../index.js");
  const initial = new NativeDesignSystemLoader();
  initial.provide(0, "theme", "@utility brand { color:red; } @stillwind-slot utilities;");
  const session = new NativeProjectSession(initial, true);
  const source =
    "import { style } from 'stillwind/react'; const motion = style.animation({ from: 'brand' }); const x = style(motion(''));";
  const output = session.compile([{ path: "motion.tsx", source }]);
  assert.deepEqual(output.modules[0].diagnostics, []);
  const before = session.css();
  assert.ok(before.length > 0);
  const invalid = new NativeDesignSystemLoader();
  invalid.provide(0, "theme", "");
  assert.throws(() => session.reload(invalid), /CSS reload compilation failed.*SH001/s);
  assert.equal(session.css(), before);
  const recovered = new NativeDesignSystemLoader();
  recovered.provide(0, "theme", "@utility brand { color:blue; } @stillwind-slot utilities;");
  assert.equal(session.reload(recovered).cssChanged, true);
  assert.notEqual(session.css(), before);
});

void test("native sessions join supplied type facts with another module's asset metadata", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession, summarizeReact } =
    await import("../index.js");
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "theme", "");
  const producer = {
    path: "producer.tsx",
    source: "import { style } from 'stillwind/react'; export const marker = style.ref('shared');",
  };
  const consumer = {
    path: "consumer.tsx",
    source:
      "import { style } from 'stillwind/react'; import { marker } from './producer'; const props = style(marker);",
  };
  const summary = summarizeReact(producer.source, producer.path);
  const start = consumer.source.lastIndexOf("marker");
  const session = new NativeProjectSession(loader, true);
  const without = session.compile([consumer, producer]);
  assert.ok(without.modules[0].diagnostics.length > 0);
  const withFacts = session.compile([
    {
      ...consumer,
      type_facts: [
        {
          start,
          end: start + "marker".length,
          value: summary.exports.find((entry) => entry.name === "marker").value,
        },
      ],
    },
    producer,
  ]);
  assert.deepEqual(
    withFacts.modules.map((module) => module.path),
    [consumer.path, producer.path],
  );
  assert.ok(
    withFacts.modules.every((module) => module.diagnostics.length === 0),
    JSON.stringify(withFacts),
  );
  assert.match(withFacts.modules[0].code, /s_shared_/);
  assert.doesNotMatch(withFacts.modules[0].code, /style\(marker\)/);
});
