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
  sourceMapSource: sourceUri("root.css"),
});

const oneShotInput = (source, moduleId, theme) => ({
  source,
  moduleId: moduleId,
  uri: sourceUri(moduleId),
  sourceMapSource: sourceUri(moduleId),
  language: moduleId.endsWith(".tsx") ? "tsx" : "ts",
  moduleKind: "unambiguous",
  css: cssInput(theme),
});

void test("native properties use camelCase and enum values follow their declared cases", async () => {
  const { scanSource, NativeDesignSystemLoader } = await import(bindingSpecifier);
  for (const framework of ["react", "preact", "solid", "vue", "svelte", "astro"]) {
    const summary = scanSource({
      kind: "direct",
      moduleId: "contract.js",
      document: {
        uri: sourceUri("contract.js"),
        code: "module.exports = 1;",
        format: "script",
      },
      syntax: { language: "js", moduleKind: "commonjs" },
      framework,
    });
    assert.equal(summary.moduleId, "contract.js");
    assert.ok(Object.hasOwn(summary, "typeProbes"));
    assert.equal(Object.hasOwn(summary, "module_id"), false);
    assert.equal(Object.hasOwn(summary, "type_probes"), false);
  }
  const loader = new NativeDesignSystemLoader();
  const step = loader.provide(0, "root.css", '@import "child.css";');
  assert.equal(step.kind, "needFile");
  assert.equal(step.requests[0].specifier, "child.css");
});

void test("lint settings preserve rule names and reporting levels across the native boundary", async () => {
  const { compileSource } = await import(bindingSpecifier);
  const source = "import {style} from 'stillwind/react'; export const props=style('missing');";
  for (const severity of ["warn", "deny", "off"]) {
    const output = compileSource(
      oneShotInput(
        source,
        "lint.tsx",
        `@stillwind-slot utilities; @stillwind {unknown-utility: ${severity}; typo: deny;}`,
      ),
    );
    assert.deepEqual(output.diagnostics, []);
    assert.equal(output.lintSettings["unknown-utility"], severity);
    assert.equal(output.lintSettings["macro-argument-mutation"], "deny");
    assert.equal(Object.hasOwn(output.lintSettings, "typo"), false);
    assert.equal(output.lintFindings.length, severity === "off" ? 0 : 1);
    if (severity !== "off") {
      assert.equal(output.lintFindings[0].rule, "unknown-utility");
      assert.equal(output.reports[0].severity, severity === "deny" ? "error" : "warning");
    }
    assert.ok(output.code);
  }
});

void test("prepared documents require formats and keep document identity separate from module identity", async () => {
  const { scanSource, compileProject, NativeDesignSystemLoader, NativeProjectSession } =
    await import(bindingSpecifier);
  const loader = new NativeDesignSystemLoader();
  loader.provide(0, "root.css", "@stillwind-slot utilities;", sourceUri("root.css"));
  const session = new NativeProjectSession(loader, true);
  const input = {
    kind: "direct",
    moduleId: "opaque-module",
    document: {
      uri: "untitled:document.tsx",
      code: "import {style} from 'stillwind/react';\nconst props = style('missing');",
      format: "jsx",
      sourceMapSource: "original/document.tsx",
    },
    syntax: { language: "tsx", moduleKind: "unambiguous" },
  };
  assert.equal(scanSource(input).moduleId, input.moduleId);
  for (const compile of [
    (source) => compileProject([source], cssInput("@stillwind-slot utilities;")).modules[0],
    (source) => session.compile([source]).modules[0],
  ]) {
    const output = compile(input);
    assert.deepEqual(output.diagnostics, []);
    assert.equal(output.lintFindings[0].uri, input.document.uri);
    assert.equal(output.lintFindings[0].startLine, 1);
    assert.equal(output.moduleId, input.moduleId);
  }
  for (const consume of [
    scanSource,
    (source) => compileProject([source], cssInput("@stillwind-slot utilities;")),
    (source) => session.compile([source]),
  ]) {
    for (const format of ["vue", "svelte", "astro"]) {
      assert.throws(
        () => consume({ ...input, document: { ...input.document, format } }),
        /direct source requires script or JSX format/,
      );
    }
    assert.throws(
      () => consume({ ...input, document: { ...input.document, format: undefined } }),
      /format/,
    );
  }
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
    kind: "direct",
    moduleId: path,
    document: {
      uri: sourceUri(path),
      code: `import {style} from 'stillwind/react';export const props=style(${utility});`,
      format: "jsx",
      sourceMapSource: sourceUri(path),
    },
    syntax: { language: "tsx", moduleKind: "unambiguous" },
  });
  for (const watch of [true, false]) {
    const session = new NativeProjectSession(loader, watch);
    session.compile([source("a.ts", "'red'"), source("b.ts", "'red'")]);
    const css = session.css();
    const result = session.compile([source("a.ts", "'blue'"), source("b.ts", "unknown")]);
    assert.ok(result.modules.some((module) => module.diagnostics.length));
    const attempted = session.diagnosticBatches().find((batch) => batch.module === "b.ts");
    assert.equal(attempted.reports[0].primary.location.uri, sourceUri("b.ts"));
    assert.equal(attempted.reports[0].primary.location.startLine, 0);
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
  assert.equal(typeof output.mappings[0].originalStart, "number");
  const sources = [
    {
      kind: "direct",
      moduleId: "🧭.tsx",
      document: {
        uri: sourceUri("🧭.tsx"),
        code: source,
        format: "jsx",
        sourceMapSource: sourceUri("🧭.tsx"),
      },
      syntax: { language: "tsx", moduleKind: "unambiguous" },
    },
  ];
  assert.deepEqual(
    compileProject(sources, cssInput(css)).modules.map((module) => module.moduleId),
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
  assert.equal(handle.value[0].reference.moduleId, "🧭.tsx");
  assert.equal(handle.value[0].permittedBoundaries, null);
  const reserved = summarizeSource(
    "import {style} from 'stillwind/react'; export const __proto__=style.ref();",
    "reserved.ts",
    "ts",
    "unambiguous",
  );
  assert.equal(reserved.exports[0].name, "__proto__");
  assert.equal(reserved.exportImports[0].name, "__proto__");
  assert.throws(() => compileProject("[]", cssInput(css)), /array/i);
  assert.throws(() => compileSource(oneShotInput(source, "x.ts", css), "{}"));
  assert.throws(() => new NativeDesignSystemLoader('{"prefix":"--x-"}'));
  const loader = new NativeDesignSystemLoader();
  assert.equal(loader.provide(0, "root", css, sourceUri("root")).kind, "complete");
  const session = new NativeProjectSession(loader, false);
  assert.deepEqual(session.compile(sources).modules[0].diagnostics, []);
  assert.throws(() => session.compile("[]"), /array/i);
  assert.throws(
    () =>
      session.compile([{ ...sources[0], syntax: { ...sources[0].syntax, language: "unknown" } }]),
    /language/,
  );
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
      kind: "direct",
      moduleId: "a.ts",
      document: {
        uri: sourceUri("a.ts"),
        code: source,
        format: "jsx",
        sourceMapSource: sourceUri("a.ts"),
      },
      syntax: { language: "tsx", moduleKind: "unambiguous" },
    },
    {
      kind: "direct",
      moduleId: "b.ts",
      document: {
        uri: sourceUri("b.ts"),
        code: source,
        format: "jsx",
        sourceMapSource: sourceUri("b.ts"),
      },
      syntax: { language: "tsx", moduleKind: "unambiguous" },
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
          kind: "direct",
          moduleId: "facts.ts",
          document: {
            uri: sourceUri("facts.ts"),
            code: "export {};",
            format: "jsx",
            sourceMapSource: sourceUri("facts.ts"),
          },
          syntax: { language: "tsx", moduleKind: "unambiguous" },
          typeFacts: [{ start: 0, end: 0, value }],
        },
      ],
      cssInput(""),
    );
  const reference = {
    kind: "ref",
    moduleId: "facts.ts",
    start: 0,
    end: 1,
    bindingName: "handle",
  };
  const boundary = {
    kind: "ref",
    moduleId: "facts.ts",
    start: 2,
    end: 3,
    bindingName: "menu",
  };
  const symbolic = (kind) => ({ kind, candidates: { kind: "universe" } });
  const finite = (kind, values) => ({ kind, candidates: { kind: "finite", values } });
  for (const value of [
    { kind: "refs", value: [{ reference, permittedBoundaries: null }] },
    { kind: "refs", value: [{ reference, permittedBoundaries: symbolic("ref") }] },
    {
      kind: "refs",
      value: [{ reference, permittedBoundaries: finite("ref", [boundary]) }],
    },
    { kind: "keyframes", value: symbolic("keyframes") },
    { kind: "viewTransitions", value: symbolic("viewTransition") },
  ]) {
    assert.deepEqual(compile(value).modules[0].diagnostics, []);
  }
  for (const value of [
    {
      kind: "refs",
      value: [{ reference: { ...reference, kind: "keyframes" }, permittedBoundaries: null }],
    },
    {
      kind: "refs",
      value: [{ reference, permittedBoundaries: symbolic("keyframes") }],
    },
    {
      kind: "refs",
      value: [
        {
          reference,
          permittedBoundaries: finite("ref", [{ ...boundary, kind: "keyframes" }]),
        },
      ],
    },
    {
      kind: "refs",
      value: [{ reference, permittedBoundaries: { kind: "ref", candidates: { kind: "finite" } } }],
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
      kind: "direct",
      moduleId: "button.tsx",
      document: {
        uri: sourceUri("button.tsx"),
        code: source,
        format: "jsx",
        sourceMapSource: sourceUri("button.tsx"),
      },
      syntax: { language: "tsx", moduleKind: "unambiguous" },
    },
  ]);
  assert.deepEqual(result.modules[0].diagnostics, []);
  assert.match(result.modules[0].css, /display:flex/);
  assert.match(result.modules[0].code, /className/);
  assert.deepEqual(
    compileProject(
      [
        {
          kind: "direct",
          moduleId: "button.tsx",
          document: {
            uri: sourceUri("button.tsx"),
            code: source,
            format: "jsx",
            sourceMapSource: sourceUri("button.tsx"),
          },
          syntax: { language: "tsx", moduleKind: "unambiguous" },
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
  assert.equal(standalone.diagnostics[0].code, "SW08002");
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
  const { NativeDesignSystemLoader, NativeProjectSession, scanSource, compileProject } =
    await import(bindingSpecifier);
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
    kind: "projected",
    moduleId: "Component.svelte",
    document: {
      uri: sourceUri("Component.svelte"),
      code: prefix + source + suffix,
      format: "svelte",
      sourceMapSource: sourceUri("Component.svelte"),
    },
    framework: "svelte",
    projection: {
      code: source,
      syntax: { language: "ts", moduleKind: "unambiguous" },
      map: {
        kind: "volar",
        mappings: [
          { sourceOffsets: [prefix.length], generatedOffsets: [0], lengths: [source.length] },
        ],
      },
      converter: { name: "svelte2tsx", version: "0.7.61", optionsFingerprint: "test" },
      importRealms: [{ id: 0, start: prefix.length, end: prefix.length + source.length }],
      editableRegions: [
        { start: prefix.length, end: prefix.length + source.length, importRealm: 0 },
      ],
    },
  };
  const before = session.compile([input]).modules[0];
  assert.deepEqual(before.diagnostics, []);
  assert(before.code.startsWith(prefix));
  assert(before.code.endsWith(suffix));
  assert(before.code.includes("{class:"));
  assert(
    before.mappings.some(
      (mapping) => mapping.originalStart === prefix.length + source.indexOf("style('"),
    ),
  );
  assert(!before.code.includes("stillwind/svelte"));
  const css = session.css();
  for (const consume of [
    scanSource,
    (source) =>
      compileProject(
        [source],
        cssInput("@utility flex { display:flex; } @stillwind-slot utilities;"),
      ),
    (source) => session.compile([source]),
  ]) {
    assert.throws(
      () => consume({ ...input, projection: { ...input.projection, importRealms: [] } }),
      /editable region 0 references missing import realm 0/,
    );
    assert.throws(
      () =>
        consume({
          ...input,
          projection: {
            ...input.projection,
            editableRegions: [
              ...input.projection.editableRegions,
              input.projection.editableRegions[0],
            ],
          },
        }),
      /editable regions 0 and 1 overlap/,
    );
  }
  const outsideRegions = session.compile([
    { ...input, projection: { ...input.projection, editableRegions: [] } },
  ]).modules[0];
  assert.equal(outsideRegions.diagnostics[0].code, "SW11007");
  assert.equal(session.css(), css);
  const invalid = {
    ...input,
    projection: { ...input.projection, map: { kind: "volar", mappings: [] } },
  };
  const failed = session.compile([invalid]).modules[0];
  assert.equal(failed.diagnostics[0].code, "SW10002");
  assert.equal(session.css(), css);
  const recovered = session.compile([input]).modules[0];
  assert.deepEqual(recovered.diagnostics, []);
  assert.equal(recovered.code, before.code);
  for (const field of ["code", "syntax", "map", "converter", "importRealms", "editableRegions"]) {
    assert.throws(
      () =>
        session.compile([{ ...input, projection: { ...input.projection, [field]: undefined } }]),
      new RegExp(field),
    );
  }
  assert.throws(
    () => session.compile([{ ...input, document: { ...input.document, format: undefined } }]),
    /format/,
  );
  assert.throws(
    () => session.compile([{ ...input, document: { ...input.document, format: "jsx" } }]),
    /component|format/,
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
      kind: "direct",
      moduleId: "button.tsx",
      document: {
        uri: sourceUri("button.tsx"),
        code: "import { style } from 'stillwind/react'; const x = style('flex');",
        format: "jsx",
        sourceMapSource: sourceUri("button.tsx"),
      },
      syntax: { language: "tsx", moduleKind: "unambiguous" },
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
      kind: "direct",
      moduleId: "a.tsx",
      document: {
        uri: sourceUri("a.tsx"),
        code: source,
        format: "jsx",
        sourceMapSource: sourceUri("a.tsx"),
      },
      syntax: { language: "tsx", moduleKind: "unambiguous" },
    },
    {
      kind: "direct",
      moduleId: "b.tsx",
      document: {
        uri: sourceUri("b.tsx"),
        code: source,
        format: "jsx",
        sourceMapSource: sourceUri("b.tsx"),
      },
      syntax: { language: "tsx", moduleKind: "unambiguous" },
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
    /untitled:broken\.css:1:1-1:\d+: SW08026:/,
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
      kind: "direct",
      moduleId: "motion.tsx",
      document: {
        uri: sourceUri("motion.tsx"),
        code: source,
        format: "jsx",
        sourceMapSource: sourceUri("motion.tsx"),
      },
      syntax: { language: "tsx", moduleKind: "unambiguous" },
    },
  ]);
  assert.deepEqual(output.modules[0].diagnostics, []);
  const before = session.css();
  assert.ok(before.length > 0);
  const invalid = session.newLoader();
  invalid.provide(0, "theme", "", sourceUri("theme"));
  assert.throws(
    () => session.reload(invalid),
    /CSS reload compilation failed.*motion\.tsx.*SW02003.*unknown or unavailable utility `brand`/s,
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
    kind: "direct",
    moduleId: "producer.tsx",
    document: {
      uri: sourceUri("producer.tsx"),
      code: "import { style } from 'stillwind/react'; export const marker = style.ref('shared');",
      format: "jsx",
      sourceMapSource: sourceUri("producer.tsx"),
    },
    syntax: { language: "tsx", moduleKind: "unambiguous" },
  };
  const consumer = {
    kind: "direct",
    moduleId: "consumer.tsx",
    document: {
      uri: sourceUri("consumer.tsx"),
      code: "import { style } from 'stillwind/react'; import { marker } from './producer'; const props = style(marker);",
      format: "jsx",
      sourceMapSource: sourceUri("consumer.tsx"),
    },
    syntax: { language: "tsx", moduleKind: "unambiguous" },
  };
  const summary = summarizeSource(producer.document.code, producer.moduleId, "tsx", "unambiguous");
  const start = consumer.document.code.lastIndexOf("marker");
  const session = new NativeProjectSession(loader, true);
  const without = session.compile([consumer, producer]);
  assert.ok(without.modules[0].diagnostics.length > 0);
  const withFacts = session.compile([
    {
      ...consumer,
      typeFacts: [
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
    withFacts.modules.map((module) => module.moduleId),
    [consumer.moduleId, producer.moduleId],
  );
  assert.ok(
    withFacts.modules.every((module) => module.diagnostics.length === 0),
    JSON.stringify(withFacts),
  );
  assert.match(withFacts.modules[0].code, /sw_shared_/);
  assert.doesNotMatch(withFacts.modules[0].code, /style\(marker\)/);
});
