import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import { pathToFileURL } from "node:url";

const bindingSpecifier =
  process.env.STILLWIND_TEST_WASM === "1"
    ? pathToFileURL(createRequire(import.meta.url).resolve("../npm/wasm32-wasi")).href
    : "../dist/index.js";

void test("minifyCss exposes final CSS processing and separate source maps", async () => {
  const { minifyCss } = await import(bindingSpecifier);
  const result = minifyCss(".a { color: red; &:hover { color: blue } } .unused { width: 1px }", {
    filename: "asset.css",
    targets: { chrome: 100 << 16 },
    unusedSymbols: ["unused"],
    sourceMap: true,
    includeSourcesContent: false,
  });
  assert.ok(!result.css.includes("&"));
  assert.ok(!result.css.includes("unused"));
  assert.ok(!result.css.includes("sourceMappingURL"));
  const map = JSON.parse(result.sourceMap);
  assert.equal(map.version, 3);
  assert.ok(map.sources.includes("asset.css"));
  assert.equal(map.sourcesContent, undefined);
  assert.deepEqual(result.warnings, []);
  assert.ok(minifyCss(".a { color: red } ").css.includes(".a{color:red}"));
});

void test("minifyCss forwards input maps, features, recovery, and option errors", async () => {
  const { minifyCss } = await import(bindingSpecifier);
  const inputSourceMap = JSON.stringify({
    version: 3,
    sources: ["original.tsx"],
    sourcesContent: ["original source"],
    names: [],
    mappings: "AAEE",
  });
  const result = minifyCss(".a { color: red; &:hover { color: blue } }", {
    include: ["Nesting"],
    sourceMap: true,
    inputSourceMap,
  });
  assert.ok(!result.css.includes("&"));
  assert.ok(JSON.parse(result.sourceMap).sources.includes("original.tsx"));
  const preserved = minifyCss(".a { &:hover { color: blue } }", {
    targets: { chrome: 100 << 16 },
    exclude: ["Nesting"],
  });
  assert.ok(preserved.css.includes("&"));
  assert.throws(() => minifyCss(".a { color red }"), /CSS parse failed/);
  assert.ok(minifyCss(".a { color red }", { errorRecovery: true }).warnings.length > 0);
  assert.throws(() => minifyCss("", { include: ["Typo"] }), /unknown CSS feature/);
  assert.throws(() => minifyCss("", { inputSourceMap }), /requires source_map/);
});

const sourceUri = (path) => pathToFileURL(path).href;
const cssInput = (content) => ({
  content,
  name: "root.css",
  uri: sourceUri("root.css"),
  source_map_source: sourceUri("root.css"),
});

const oneShotInput = (source, moduleId, theme) => ({
  source,
  module_id: moduleId,
  uri: sourceUri(moduleId),
  source_map_source: sourceUri(moduleId),
  language: moduleId.endsWith(".tsx") ? "tsx" : "ts",
  module_kind: "unambiguous",
  css: cssInput(theme),
});

void test("failed compile transactions retain watch CSS but reject current plans", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
  const loader = new NativeDesignSystemLoader();
  loader.provide(
    0,
    "root.css",
    "@utility red{color:red}@utility blue{color:blue}@stillwind-slot utilities;",
    sourceUri("root.css"),
  );
  const source = (path, utility) => ({
    module_id: path,
    uri: sourceUri(path),
    language: "tsx",
    module_kind: "unambiguous",
    source_map_source: sourceUri(path),
    source: `import {style} from 'stillwind/react';export const props=style(${utility});`,
  });
  for (const watch of [true, false]) {
    const session = new NativeProjectSession(loader, watch);
    session.compile([source("a.ts", "'red'"), source("b.ts", "'red'")]);
    const css = session.css();
    const result = session.compile([source("a.ts", "'blue'"), source("b.ts", "unknown")]);
    assert.ok(result.modules.some((module) => module.diagnostics.length));
    const attempted = session.diagnosticBatches().find((batch) => batch.module === "b.ts");
    assert.equal(attempted.reports[0].primary.location.uri, sourceUri("b.ts"));
    assert.equal(attempted.reports[0].primary.location.start_line, 0);
    if (watch) {
      assert.equal(session.css(), css);
    }
    assert.throws(
      () =>
        session.planStylesheet({
          modules: ["a.ts", "b.ts"],
          coverage: { kind: "observed" },
          placement: "document",
        }),
      /stale|unregistered/,
    );
    session.compile([source("a.ts", "'blue'"), source("b.ts", "'red'")]);
    assert.deepEqual(
      session.diagnosticBatches().find((batch) => batch.module === "b.ts").reports,
      [],
    );
    assert.match(
      session.planStylesheet({
        modules: ["a.ts", "b.ts"],
        coverage: { kind: "observed" },
        placement: "document",
      }).css,
      /color:blue/,
    );
  }
});

void test("native object transport returns typed values and rejects the former JSON protocol", async () => {
  const {
    compileSource,
    compileProject,
    summarizeSource,
    NativeDesignSystemLoader,
    NativeProjectSession,
  } = await import(bindingSpecifier);
  const source = "import {style} from 'stillwind/react'; export const props=style('flex');";
  const css = "@utility flex {display:flex;} @stillwind-slot utilities;";
  const output = compileSource(oneShotInput(source, "🧭.tsx", css), {});
  assert.equal(typeof output, "object");
  assert.equal(Object.getPrototypeOf(output), Object.prototype);
  assert.deepEqual(output.diagnostics, []);
  assert.match(output.code, /className/);
  assert.equal(typeof output.mappings[0].original_start, "number");
  const sources = [
    {
      module_id: "🧭.tsx",
      uri: sourceUri("🧭.tsx"),
      language: "tsx",
      module_kind: "unambiguous",
      source_map_source: sourceUri("🧭.tsx"),
      source,
    },
  ];
  assert.deepEqual(
    compileProject(sources, cssInput(css)).modules.map((module) => module.module_id),
    ["🧭.tsx"],
  );
  const summary = summarizeSource(
    "import {style} from 'stillwind/react'; export const handle=style.ref();",
    "🧭.tsx",
    "tsx",
    "unambiguous",
  );
  const handle = summary.exports.find((entry) => entry.name === "handle").value;
  assert.equal(handle.kind, "refs");
  assert.equal(handle.value[0].reference.module_id, "🧭.tsx");
  assert.equal(handle.value[0].permitted_boundaries, null);
  const reserved = summarizeSource(
    "import {style} from 'stillwind/react'; export const __proto__=style.ref();",
    "reserved.ts",
    "ts",
    "unambiguous",
  );
  assert.equal(reserved.exports[0].name, "__proto__");
  assert.equal(reserved.export_imports[0].name, "__proto__");
  assert.throws(() => compileProject("[]", cssInput(css)), /array/i);
  assert.throws(() => compileSource(oneShotInput(source, "x.ts", css), "{}"));
  assert.throws(() => new NativeDesignSystemLoader('{"prefix":"--x-"}'));
  const loader = new NativeDesignSystemLoader();
  assert.equal(loader.provide(0, "root", css, sourceUri("root")).kind, "complete");
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
  const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
  const loader = new NativeDesignSystemLoader({});
  loader.provide(
    0,
    "root.css",
    "@utility flex {display:flex;} @stillwind-slot utilities;",
    sourceUri("root.css"),
  );
  const session = new NativeProjectSession(loader, true);
  const source = "import {style} from 'stillwind/react'; export const props=style('flex');";
  session.compile([
    {
      module_id: "a.ts",
      uri: sourceUri("a.ts"),
      language: "tsx",
      module_kind: "unambiguous",
      source_map_source: sourceUri("a.ts"),
      source,
    },
    {
      module_id: "b.ts",
      uri: sourceUri("b.ts"),
      language: "tsx",
      module_kind: "unambiguous",
      source_map_source: sourceUri("b.ts"),
      source,
    },
  ]);
  const shared = session.css();
  assert.equal(session.remove("a.ts"), true);
  assert.equal(
    session.diagnosticBatches().some((batch) => batch.module === "a.ts"),
    false,
  );
  assert.notEqual(session.css(), shared);
  assert.equal(session.remove("a.ts"), false);
  assert.equal(session.remove("b.ts"), true);
  assert.notEqual(session.css(), shared);
});

void test("native semantic unions validate nested asset kinds and candidate payloads", async () => {
  const { compileProject } = await import(bindingSpecifier);
  const compile = (value) =>
    compileProject(
      [
        {
          module_id: "facts.ts",
          uri: sourceUri("facts.ts"),
          language: "tsx",
          module_kind: "unambiguous",
          source_map_source: sourceUri("facts.ts"),
          source: "export {};",
          type_facts: [{ start: 0, end: 0, value }],
        },
      ],
      cssInput(""),
    );
  const reference = {
    kind: "ref",
    module_id: "facts.ts",
    start: 0,
    end: 1,
    binding_name: "handle",
  };
  const boundary = {
    kind: "ref",
    module_id: "facts.ts",
    start: 2,
    end: 3,
    binding_name: "menu",
  };
  const symbolic = (kind) => ({ kind, candidates: { kind: "universe" } });
  const finite = (kind, values) => ({ kind, candidates: { kind: "finite", values } });
  for (const value of [
    { kind: "refs", value: [{ reference, permitted_boundaries: null }] },
    { kind: "refs", value: [{ reference, permitted_boundaries: symbolic("ref") }] },
    {
      kind: "refs",
      value: [{ reference, permitted_boundaries: finite("ref", [boundary]) }],
    },
    { kind: "keyframes", value: symbolic("keyframes") },
    { kind: "view-transitions", value: symbolic("view-transition") },
  ]) {
    assert.deepEqual(compile(value).modules[0].diagnostics, []);
  }
  for (const value of [
    {
      kind: "refs",
      value: [{ reference: { ...reference, kind: "keyframes" }, permitted_boundaries: null }],
    },
    {
      kind: "refs",
      value: [{ reference, permitted_boundaries: symbolic("keyframes") }],
    },
    {
      kind: "refs",
      value: [
        {
          reference,
          permitted_boundaries: finite("ref", [{ ...boundary, kind: "keyframes" }]),
        },
      ],
    },
    {
      kind: "refs",
      value: [{ reference, permitted_boundaries: { kind: "ref", candidates: { kind: "finite" } } }],
    },
    { kind: "unknown", value: symbolic("ref") },
  ]) {
    assert.throws(() => compile(value));
  }
});

void test("stylesheet assembly errors belong to rendering APIs", async () => {
  const {
    NativeDesignSystemLoader,
    NativeProjectSession,
    compileSource,
    compileProject,
    renderStylesheet,
  } = await import(bindingSpecifier);
  const source = "import {style} from 'stillwind/react'; export const props=style('flex');";
  const css = "@utility flex {display:flex;}";
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "root.css", css, sourceUri("root.css"));
  const session = new NativeProjectSession(loader, true);
  const result = session.compile([
    {
      module_id: "button.tsx",
      uri: sourceUri("button.tsx"),
      language: "tsx",
      module_kind: "unambiguous",
      source_map_source: sourceUri("button.tsx"),
      source,
    },
  ]);
  assert.deepEqual(result.modules[0].diagnostics, []);
  assert.match(result.modules[0].css, /display:flex/);
  assert.match(result.modules[0].code, /className/);
  assert.deepEqual(
    compileProject(
      [
        {
          module_id: "button.tsx",
          uri: sourceUri("button.tsx"),
          language: "tsx",
          module_kind: "unambiguous",
          source_map_source: sourceUri("button.tsx"),
          source,
        },
      ],
      cssInput(css),
    ).modules[0].diagnostics,
    [],
  );
  assert.throws(() => session.css(), /@stillwind-slot/);
  assert.throws(() => session.moduleCss("button.tsx"), /@stillwind-slot/);
  assert.throws(
    () =>
      session.planStylesheet({
        modules: ["button.tsx"],
        coverage: { kind: "observed" },
        placement: "document",
      }),
    /@stillwind-slot/,
  );
  assert.throws(() => renderStylesheet(result.modules[0].css, css), /@stillwind-slot/);
  const standalone = compileSource(oneShotInput(source, "button.tsx", css));
  assert.equal(standalone.diagnostics[0].code, "SHCSS001");
  assert.equal(standalone.diagnostics[0].start, 0);
  assert.equal(standalone.diagnostics[0].end, Buffer.byteLength(source));
  assert.equal(standalone.code, null);
  assert.equal(standalone.css, "");
  const repaired = session.newLoader();
  repaired.provide(0, "root.css", css + " @stillwind-slot utilities;", sourceUri("root.css"));
  assert.equal(session.reload(repaired).cssChanged, true);
  const foreign = new NativeDesignSystemLoader();
  foreign.provide(0, "root.css", css + " @stillwind-slot utilities;", sourceUri("root.css"));
  assert.throws(() => session.reload(foreign), /another source scope/);
  const valid = session.css();
  assert.match(valid, /display:flex/);
  assert.throws(() => session.reload(loader), /@stillwind-slot/);
  assert.equal(session.css(), valid);
});

void test("projected session edits original SFC text and preserves watch recovery", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
  const loader = new NativeDesignSystemLoader();
  assert.equal(
    loader.provide(
      0,
      "root",
      "@utility flex { display:flex; } @stillwind-slot utilities;",
      sourceUri("root"),
    ).kind,
    "complete",
  );
  const session = new NativeProjectSession(loader, true);
  const source = "import {style} from 'stillwind/svelte'; const props = style('flex');";
  const prefix = "<!-- 🧭 -->\n<script>";
  const suffix = "</script><div {...props}></div><style>/* preserved */</style>";
  const input = {
    module_id: "Component.svelte",
    uri: sourceUri("Component.svelte"),
    module_kind: "unambiguous",
    source_map_source: sourceUri("Component.svelte"),
    source,
    language: "ts",
    framework: "Svelte",
    source_format: "svelte",
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
  assert.throws(
    () => session.compile([{ ...input, source_format: undefined }]),
    /component source_format/,
  );
  assert.throws(
    () => session.compile([{ ...input, source_format: "jsx" }]),
    /component source_format/,
  );
  const inferredAdapter = session.compile([{ ...input, framework: undefined }]).modules[0];
  assert.deepEqual(inferredAdapter.diagnostics, []);
  assert.equal(inferredAdapter.code, before.code);
});

void test("loads the selected package through its public entry", async () => {
  const binding = await import(bindingSpecifier);
  assert.equal(typeof binding.compileSource, "function");
  assert.equal(typeof binding.compileProject, "function");
});

void test("session reload applies code generation options independently of CSS definitions", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
  const load = (classNamespace, session) => {
    const loader = session
      ? session.newLoader({ classNamespace })
      : new NativeDesignSystemLoader({ classNamespace });
    const result = loader.provide(
      0,
      "root",
      "@utility flex { display:flex; } @stillwind-slot utilities;",
      sourceUri("root"),
    );
    assert.equal(result.kind, "complete");
    return loader;
  };
  const session = new NativeProjectSession(load("first"), true);
  const sources = [
    {
      module_id: "button.tsx",
      uri: sourceUri("button.tsx"),
      language: "tsx",
      module_kind: "unambiguous",
      source_map_source: sourceUri("button.tsx"),
      source: "import { style } from 'stillwind/react'; const x = style('flex');",
    },
  ];
  const before = session.compile(sources);
  const beforeCss = session.css();
  const update = session.reload(load("second", session));
  assert.equal(update.cssChanged, true);
  assert.deepEqual(update.transformedModules, ["button.tsx"]);
  assert.notEqual(session.css(), beforeCss);
  const after = session.compile(sources);
  assert.notEqual(before.modules[0].code, after.modules[0].code);
  assert.deepEqual(after.modules[0].diagnostics, []);
});

void test("native sessions retain independent producer rules and replace CSS revisions", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
  const loader = new NativeDesignSystemLoader();
  assert.deepEqual(
    loader.provide(
      0,
      "opaque root",
      "@import 'tokens'; :root,:host { @stillwind-slot theme; } @utility bg-brand {background-color:var(--color-brand)} @stillwind-slot utilities;",
      sourceUri("opaque root"),
    ),
    {
      kind: "needFile",
      requests: [{ id: 1, importer: "opaque root", specifier: "tokens" }],
    },
  );
  assert.equal(
    loader.provide(1, "opaque child", "@theme { --color-brand: red; }", sourceUri("opaque child"))
      .kind,
    "complete",
  );
  const session = new NativeProjectSession(loader, true);
  const source = "import { style } from 'stillwind/react'; const x = style('bg-brand');";
  const result = session.compile([
    {
      module_id: "a.tsx",
      uri: sourceUri("a.tsx"),
      language: "tsx",
      module_kind: "unambiguous",
      source_map_source: sourceUri("a.tsx"),
      source,
    },
    {
      module_id: "b.tsx",
      uri: sourceUri("b.tsx"),
      language: "tsx",
      module_kind: "unambiguous",
      source_map_source: sourceUri("b.tsx"),
      source,
    },
  ]);
  assert.equal(result.modules.length, 2);
  assert.ok(result.modules.every((module) => module.diagnostics.length === 0));
  const before = session.css();
  assert.ok(before.length > 0);
  assert.equal(session.remove("a.tsx"), true);
  assert.notEqual(session.css(), before);
  assert.equal((session.css().match(/background-color:/g) ?? []).length, 1);
  const next = session.newLoader();
  next.provide(
    0,
    "root",
    "@theme { --color-brand: blue; } :root,:host { @stillwind-slot theme; } @utility bg-brand {background-color:var(--color-brand)} @stillwind-slot utilities;",
    sourceUri("root"),
  );
  assert.equal(session.reload(next).cssChanged, true);
  assert.notEqual(session.css(), before);
  assert.equal(session.remove("b.tsx"), true);
});

void test("native loader accepts partial and reversed batches", async () => {
  const { NativeDesignSystemLoader } = await import(bindingSpecifier);
  const loader = new NativeDesignSystemLoader();
  const first = loader.provideMany([
    { id: 0, name: "root", uri: sourceUri("root"), content: "@import 'a'; @import 'b';" },
  ]);
  assert.deepEqual(
    first.requests.map((request) => request.id),
    [1, 2],
  );
  const next = loader.provide(2, "b", "@import 'c';", sourceUri("b"));
  assert.deepEqual(
    next.requests.map((request) => request.id),
    [1, 3],
  );
  const done = loader.provideMany([
    { id: 3, name: "c", uri: sourceUri("c"), content: "@theme { --color-child: green; }" },
    {
      id: 1,
      name: "a",
      uri: sourceUri("a"),
      content: "@theme { --color-brand: red; } @utility brand {color:var(--color-brand)}",
    },
  ]);
  assert.equal(done.kind, "complete");
  assert.deepEqual(new Set(done.sources), new Set(["root", "a", "b", "c"]));
  assert.match(loader.generateVocabularyTypes(), /brand/);
  assert.throws(() => loader.provideMany([]), /after completion/);
});

void test("CSS load errors include the accepted document range", async () => {
  const { NativeDesignSystemLoader } = await import(bindingSpecifier);
  const loader = new NativeDesignSystemLoader();
  assert.throws(
    () => loader.provide(0, "broken.css", "@theme {", "untitled:broken.css"),
    /untitled:broken\.css:1:1-1:\d+: SHCSS\d+:/,
  );
});

void test("CSS without a document URI remains an internal source", async () => {
  const { NativeDesignSystemLoader } = await import(bindingSpecifier);
  const loader = new NativeDesignSystemLoader();
  assert.throws(
    () => loader.provide(0, "internal", "@theme {", null),
    /unterminated CSS block at byte 7/,
  );
});

void test("native watch reload reports compilation errors without publishing a partial revision", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
  const initial = new NativeDesignSystemLoader();
  initial.provide(
    0,
    "theme",
    "@utility brand { color:red; } @stillwind-slot utilities; @stillwind-slot keyframes;",
    sourceUri("theme"),
  );
  const session = new NativeProjectSession(initial, true);
  const source =
    "import { style } from 'stillwind/react'; const motion = style.keyframes({ from: 'brand' }); const x = style(motion(''));";
  const output = session.compile([
    {
      module_id: "motion.tsx",
      uri: sourceUri("motion.tsx"),
      language: "tsx",
      module_kind: "unambiguous",
      source_map_source: sourceUri("motion.tsx"),
      source,
    },
  ]);
  assert.deepEqual(output.modules[0].diagnostics, []);
  const before = session.css();
  assert.ok(before.length > 0);
  const invalid = session.newLoader();
  invalid.provide(0, "theme", "", sourceUri("theme"));
  assert.throws(
    () => session.reload(invalid),
    /CSS reload compilation failed.*motion\.tsx.*SH001.*unknown or unavailable utility `brand`/s,
  );
  assert.equal(
    session.diagnosticBatches()[0].reports[0].primary.location.uri,
    sourceUri("motion.tsx"),
  );
  assert.equal(session.css(), before);
  const recovered = session.newLoader();
  recovered.provide(
    0,
    "theme",
    "@utility brand { color:blue; } @stillwind-slot utilities; @stillwind-slot keyframes;",
    sourceUri("theme"),
  );
  assert.equal(session.reload(recovered).cssChanged, true);
  assert.deepEqual(session.diagnosticBatches()[0].reports, []);
  assert.notEqual(session.css(), before);
});

void test("native sessions join supplied type facts with another module's asset metadata", async () => {
  const { NativeDesignSystemLoader, NativeProjectSession, summarizeSource } = await import(
    bindingSpecifier
  );
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "theme", "", sourceUri("theme"));
  const producer = {
    module_id: "producer.tsx",
    uri: sourceUri("producer.tsx"),
    language: "tsx",
    module_kind: "unambiguous",
    source_map_source: sourceUri("producer.tsx"),
    source: "import { style } from 'stillwind/react'; export const marker = style.ref('shared');",
  };
  const consumer = {
    module_id: "consumer.tsx",
    uri: sourceUri("consumer.tsx"),
    language: "tsx",
    module_kind: "unambiguous",
    source_map_source: sourceUri("consumer.tsx"),
    source:
      "import { style } from 'stillwind/react'; import { marker } from './producer'; const props = style(marker);",
  };
  const summary = summarizeSource(producer.source, producer.module_id, "tsx", "unambiguous");
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
    withFacts.modules.map((module) => module.module_id),
    [consumer.module_id, producer.module_id],
  );
  assert.ok(
    withFacts.modules.every((module) => module.diagnostics.length === 0),
    JSON.stringify(withFacts),
  );
  assert.match(withFacts.modules[0].code, /sw_shared_/);
  assert.doesNotMatch(withFacts.modules[0].code, /style\(marker\)/);
});
