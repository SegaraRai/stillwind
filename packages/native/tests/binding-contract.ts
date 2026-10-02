import { pathToFileURL } from "node:url";

import { expect, test } from "vite-plus/test";

/**
 * Registers the output and validation contracts for a public binding entry.
 * The caller supplies a module specifier that resolves from this file to the native or WASM binding.
 * Tests import that entry when they run, and import failures fail the tests.
 */
export function registerBindingContract(bindingSpecifier: string) {
  test("minifyCss exposes final CSS processing and separate source maps", async () => {
    const { minifyCss } = await import(bindingSpecifier);
    const result = minifyCss(".a { color: red; &:hover { color: blue } } .unused { width: 1px }", {
      filename: "asset.css",
      targets: { chrome: 100 << 16 },
      unusedSymbols: ["unused"],
      sourceMap: true,
      includeSourcesContent: false,
    });
    expect(result.css).not.toContain("&");
    expect(result.css).not.toContain("unused");
    expect(result.css).not.toContain("sourceMappingURL");
    const map = JSON.parse(result.sourceMap);
    expect(map.version).toBe(3);
    expect(map.sources).toContain("asset.css");
    expect(map.sourcesContent).toBe(undefined);
    expect(result.warnings).toStrictEqual([]);
    expect(minifyCss(".a { color: red } ").css).toContain(".a{color:red}");
  });

  test("minifyCss forwards input maps, features, recovery, and option errors", async () => {
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
    expect(result.css).not.toContain("&");
    expect(JSON.parse(result.sourceMap).sources).toContain("original.tsx");
    const preserved = minifyCss(".a { &:hover { color: blue } }", {
      targets: { chrome: 100 << 16 },
      exclude: ["Nesting"],
    });
    expect(preserved.css).toContain("&");
    expect(() => minifyCss(".a { color red }")).toThrow(/CSS parse failed/);
    expect(minifyCss(".a { color red }", { errorRecovery: true }).warnings.length).toBeGreaterThan(
      0,
    );
    expect(() => minifyCss("", { include: ["Typo"] })).toThrow(/unknown CSS feature/);
    expect(() => minifyCss("", { inputSourceMap })).toThrow(/requires source_map/);
  });
  const sourceUri = (path: string) => pathToFileURL(path).href;
  const cssInput = (content: string) => ({
    content,
    name: "root.css",
    uri: sourceUri("root.css"),
    sourceMapSource: sourceUri("root.css"),
  });

  /**
   */
  const oneShotInput = (source: string, moduleId: string, theme: string) => ({
    source,
    moduleId: moduleId,
    uri: sourceUri(moduleId),
    sourceMapSource: sourceUri(moduleId),
    language: moduleId.endsWith(".tsx") ? "tsx" : "ts",
    moduleKind: "unambiguous",
    css: cssInput(theme),
  });

  test("native properties use camelCase and enum values follow their declared cases", async () => {
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
      expect(summary.moduleId).toBe("contract.js");
      expect(Object.hasOwn(summary, "typeProbes")).toBeTruthy();
      expect(Object.hasOwn(summary, "module_id")).toBe(false);
      expect(Object.hasOwn(summary, "type_probes")).toBe(false);
    }
    const loader = new NativeDesignSystemLoader();
    const step = loader.provide(0, "root.css", '@import "child.css";');
    expect(step.kind).toBe("needFile");
    expect(step.requests[0].specifier).toBe("child.css");
  });

  test("lint settings preserve rule names and reporting levels across the native boundary", async () => {
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
      expect(output.diagnostics).toStrictEqual([]);
      expect(output.lintSettings["unknown-utility"]).toBe(severity);
      expect(output.lintSettings["macro-argument-mutation"]).toBe("deny");
      expect(Object.hasOwn(output.lintSettings, "typo")).toBe(false);
      expect(output.lintFindings.length).toBe(severity === "off" ? 0 : 1);
      if (severity !== "off") {
        expect(output.lintFindings[0].rule).toBe("unknown-utility");
        expect(output.reports[0].severity).toBe(severity === "deny" ? "error" : "warning");
      }
      expect(output.code).toBeTruthy();
    }
  });

  test("prepared documents require formats and keep document identity separate from module identity", async () => {
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
    expect(scanSource(input).moduleId).toBe(input.moduleId);
    for (const compile of [
      (source: unknown) =>
        compileProject([source], cssInput("@stillwind-slot utilities;")).modules[0],
      (source: unknown) => session.compile([source]).modules[0],
    ]) {
      const output = compile(input);
      expect(output.diagnostics).toStrictEqual([]);
      expect(output.lintFindings[0].uri).toBe(input.document.uri);
      expect(output.lintFindings[0].startLine).toBe(1);
      expect(output.moduleId).toBe(input.moduleId);
    }
    for (const consume of [
      scanSource,
      (source: unknown) => compileProject([source], cssInput("@stillwind-slot utilities;")),
      (source: unknown) => session.compile([source]),
    ]) {
      for (const format of ["vue", "svelte", "astro"]) {
        expect(() => consume({ ...input, document: { ...input.document, format } })).toThrow(
          /direct source requires script or JSX format/,
        );
      }
      expect(() =>
        consume({ ...input, document: { ...input.document, format: undefined } }),
      ).toThrow(/format/);
    }
  });

  test("failed compile transactions retain watch CSS but reject current plans", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "root.css",
      "@utility red{color:red}@utility blue{color:blue}@stillwind-slot utilities;",
      sourceUri("root.css"),
    );
    const source = (path: string, utility: string) => ({
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
      expect(
        result.modules.some(
          (module: import("../binding.js").ProjectModuleOutput) => module.diagnostics.length,
        ),
      ).toBeTruthy();
      const attempted = session
        .diagnosticBatches()
        .find((batch: import("../binding.js").NativeDiagnosticBatch) => batch.module === "b.ts");
      expect(attempted.reports[0].primary.location.uri).toBe(sourceUri("b.ts"));
      expect(attempted.reports[0].primary.location.startLine).toBe(0);
      if (watch) {
        expect(session.css()).toBe(css);
      }
      expect(() =>
        session.planStylesheet({
          modules: ["a.ts", "b.ts"],
          coverage: { kind: "observed" },
          placement: "document",
        }),
      ).toThrow(/stale|unregistered/);
      session.compile([source("a.ts", "'blue'"), source("b.ts", "'red'")]);
      expect(
        session
          .diagnosticBatches()
          .find((batch: import("../binding.js").NativeDiagnosticBatch) => batch.module === "b.ts")
          .reports,
      ).toStrictEqual([]);
      expect(
        session.planStylesheet({
          modules: ["a.ts", "b.ts"],
          coverage: { kind: "observed" },
          placement: "document",
        }).css,
      ).toMatch(/color:blue/);
    }
  });

  test("native object transport returns typed values and rejects the former JSON protocol", async () => {
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
    expect(typeof output).toBe("object");
    expect(Object.getPrototypeOf(output)).toBe(Object.prototype);
    expect(output.diagnostics).toStrictEqual([]);
    expect(output.code).toMatch(/className/);
    expect(typeof output.mappings[0].originalStart).toBe("number");
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
    expect(
      compileProject(sources, cssInput(css)).modules.map(
        (module: import("../binding.js").ProjectModuleOutput) => module.moduleId,
      ),
    ).toStrictEqual(["🧭.tsx"]);
    const summary = summarizeSource(
      "import {style} from 'stillwind/react'; export const handle=style.ref();",
      "🧭.tsx",
      "tsx",
      "unambiguous",
    );
    expect(summary.schema).toBe(4);
    const handle = summary.exports.find(
      (entry: import("../binding.js").NativeExport) => entry.name === "handle",
    ).value;
    expect(handle.kind).toBe("refs");
    expect(handle.value.kind).toBe("ref");
    expect(handle.value.candidates.kind).toBe("finite");
    expect(handle.value.candidates.values[0].moduleId).toBe("🧭.tsx");
    const reserved = summarizeSource(
      "import {style} from 'stillwind/react'; export const __proto__=style.ref();",
      "reserved.ts",
      "ts",
      "unambiguous",
    );
    expect(reserved.exports[0].name).toBe("__proto__");
    expect(reserved.exportImports[0].name).toBe("__proto__");
    expect(() => compileProject("[]", cssInput(css))).toThrow(/array/i);
    expect(() => compileSource(oneShotInput(source, "x.ts", css), "{}")).toThrow();
    expect(() => new NativeDesignSystemLoader('{"prefix":"--x-"}')).toThrow();
    const loader = new NativeDesignSystemLoader();
    expect(loader.provide(0, "root", css, sourceUri("root")).kind).toBe("complete");
    const session = new NativeProjectSession(loader, false);
    expect(session.compile(sources).modules[0].diagnostics).toStrictEqual([]);
    expect(() => session.compile("[]")).toThrow(/array/i);
    expect(() =>
      session.compile([{ ...sources[0], syntax: { ...sources[0].syntax, language: "unknown" } }]),
    ).toThrow(/language/);
    expect(() =>
      session.planStylesheet({
        modules: [],
        coverage: { kind: "unknown" },
        placement: "document",
      }),
    ).toThrow(/kind|variant/);
    expect(() =>
      session.planStylesheet({
        modules: [],
        coverage: { kind: "complete" },
        placement: "document",
      }),
    ).toThrow();
    expect(() =>
      session.planStylesheet({
        modules: [],
        coverage: { kind: "observed" },
        placement: "unknown",
      }),
    ).toThrow(/placement/);
    const plan = session.planStylesheet({
      modules: ["🧭.tsx"],
      coverage: { kind: "observed" },
      placement: "document",
    });
    expect(typeof plan).toBe("object");
    expect(plan.css).toMatch(/display:flex/);
    expect(plan.producers[0].module).toBe("🧭.tsx");
    if (!Array.isArray(session.baseCssWithMap().sourceMap.entries)) {
      expect.fail("Unexpected value: Array.isArray(session.baseCssWithMap().sourceMap.entries)");
    }
  });

  test("removal reports producer presence independently of shared CSS", async () => {
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
    expect(session.remove("a.ts")).toBe(true);
    expect(
      session
        .diagnosticBatches()
        .some((batch: import("../binding.js").NativeDiagnosticBatch) => batch.module === "a.ts"),
    ).toBe(false);
    expect(session.css()).not.toBe(shared);
    expect(session.remove("a.ts")).toBe(false);
    expect(session.remove("b.ts")).toBe(true);
    expect(session.css()).not.toBe(shared);
  });

  test("native semantic unions validate nested asset kinds and candidate payloads", async () => {
    const { compileProject } = await import(bindingSpecifier);
    const compile = (value: unknown) =>
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
    const symbolic = (kind: string) => ({ kind, candidates: { kind: "universe" } });
    const finite = (kind: string, values: unknown[]) => ({
      kind,
      candidates: { kind: "finite", values },
    });
    for (const value of [
      { kind: "refs", value: finite("ref", [reference]) },
      { kind: "refs", value: finite("ref", []) },
      { kind: "refs", value: symbolic("ref") },
      { kind: "keyframes", value: symbolic("keyframes") },
      { kind: "viewTransitions", value: symbolic("viewTransition") },
    ]) {
      expect(compile(value).modules[0].diagnostics).toStrictEqual([]);
    }
    for (const value of [
      { kind: "refs", value: finite("ref", [{ ...reference, kind: "keyframes" }]) },
      { kind: "refs", value: symbolic("keyframes") },
      { kind: "refs", value: { kind: "ref", candidates: { kind: "finite" } } },
      { kind: "unknown", value: symbolic("ref") },
    ]) {
      expect(() => compile(value)).toThrow();
    }
  });

  test("stylesheet assembly errors belong to rendering APIs", async () => {
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
    expect(result.modules[0].diagnostics).toStrictEqual([]);
    expect(result.modules[0].css).toMatch(/display:flex/);
    expect(result.modules[0].code).toMatch(/className/);
    expect(
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
    ).toStrictEqual([]);
    expect(() => session.css()).toThrow(/@stillwind-slot/);
    expect(() => session.moduleCss("button.tsx")).toThrow(/@stillwind-slot/);
    expect(() =>
      session.planStylesheet({
        modules: ["button.tsx"],
        coverage: { kind: "observed" },
        placement: "document",
      }),
    ).toThrow(/@stillwind-slot/);
    expect(() => renderStylesheet(result.modules[0].css, css)).toThrow(/@stillwind-slot/);
    const standalone = compileSource(oneShotInput(source, "button.tsx", css));
    expect(standalone.diagnostics[0].code).toBe("SW08002");
    expect(standalone.diagnostics[0].start).toBe(0);
    expect(standalone.diagnostics[0].end).toBe(Buffer.byteLength(source));
    expect(standalone.code).toBe(null);
    expect(standalone.css).toBe("");
    const repaired = session.newLoader();
    repaired.provide(0, "root.css", css + " @stillwind-slot utilities;", sourceUri("root.css"));
    expect(session.reload(repaired).cssChanged).toBe(true);
    const foreign = new NativeDesignSystemLoader();
    foreign.provide(0, "root.css", css + " @stillwind-slot utilities;", sourceUri("root.css"));
    expect(() => session.reload(foreign)).toThrow(/another source scope/);
    const valid = session.css();
    expect(valid).toMatch(/display:flex/);
    expect(() => session.reload(loader)).toThrow(/@stillwind-slot/);
    expect(session.css()).toBe(valid);
  });

  test("projected session edits original SFC text and preserves watch recovery", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession, scanSource, compileProject } =
      await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader();
    expect(
      loader.provide(
        0,
        "root",
        "@utility flex { display:flex; } @stillwind-slot utilities;",
        sourceUri("root"),
      ).kind,
    ).toBe("complete");
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
    expect(before.diagnostics).toStrictEqual([]);
    expect(before.code.startsWith(prefix)).toBeTruthy();
    expect(before.code.endsWith(suffix)).toBeTruthy();
    expect(before.code).toContain("{class:");
    expect(
      before.mappings.some(
        (mapping: import("../binding.js").NativeExpressionMapping) =>
          mapping.originalStart === prefix.length + source.indexOf("style('"),
      ),
    ).toBeTruthy();
    expect(before.code).not.toContain("stillwind/svelte");
    const css = session.css();
    for (const consume of [
      scanSource,
      (source: unknown) =>
        compileProject(
          [source],
          cssInput("@utility flex { display:flex; } @stillwind-slot utilities;"),
        ),
      (source: unknown) => session.compile([source]),
    ]) {
      expect(() =>
        consume({ ...input, projection: { ...input.projection, importRealms: [] } }),
      ).toThrow(/editable region 0 references missing import realm 0/);
      expect(() =>
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
      ).toThrow(/editable regions 0 and 1 overlap/);
    }
    const outsideRegions = session.compile([
      { ...input, projection: { ...input.projection, editableRegions: [] } },
    ]).modules[0];
    expect(outsideRegions.diagnostics[0].code).toBe("SW11007");
    expect(session.css()).toBe(css);
    const invalid = {
      ...input,
      projection: { ...input.projection, map: { kind: "volar", mappings: [] } },
    };
    const failed = session.compile([invalid]).modules[0];
    expect(failed.diagnostics[0].code).toBe("SW10002");
    expect(session.css()).toBe(css);
    const recovered = session.compile([input]).modules[0];
    expect(recovered.diagnostics).toStrictEqual([]);
    expect(recovered.code).toBe(before.code);
    for (const field of ["code", "syntax", "map", "converter", "importRealms", "editableRegions"]) {
      expect(() =>
        session.compile([{ ...input, projection: { ...input.projection, [field]: undefined } }]),
      ).toThrow(new RegExp(field));
    }
    expect(() =>
      session.compile([{ ...input, document: { ...input.document, format: undefined } }]),
    ).toThrow(/format/);
    expect(() =>
      session.compile([{ ...input, document: { ...input.document, format: "jsx" } }]),
    ).toThrow(/component|format/);
    const inferredAdapter = session.compile([{ ...input, framework: undefined }]).modules[0];
    expect(inferredAdapter.diagnostics).toStrictEqual([]);
    expect(inferredAdapter.code).toBe(before.code);
  });

  test("loads the selected package through its public entry", async () => {
    const binding = await import(bindingSpecifier);
    expect(typeof binding.compileSource).toBe("function");
    expect(typeof binding.compileProject).toBe("function");
  });

  test("session reload applies code generation options independently of CSS definitions", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const load = (
      classNamespace: string,
      session?: import("../binding.js").NativeProjectSession,
    ) => {
      const loader = session
        ? session.newLoader({ classNamespace })
        : new NativeDesignSystemLoader({ classNamespace });
      const result = loader.provide(
        0,
        "root",
        "@utility flex { display:flex; } @stillwind-slot utilities;",
        sourceUri("root"),
      );
      expect(result.kind).toBe("complete");
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
    expect(update.cssChanged).toBe(true);
    expect(update.transformedModules).toStrictEqual(["button.tsx"]);
    expect(session.css()).not.toBe(beforeCss);
    const after = session.compile(sources);
    expect(before.modules[0].code).not.toBe(after.modules[0].code);
    expect(after.modules[0].diagnostics).toStrictEqual([]);
  });

  test("native sessions retain independent producer rules and replace CSS revisions", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader();
    expect(
      loader.provide(
        0,
        "opaque root",
        "@import 'tokens'; :root,:host { @stillwind-slot theme; } @utility bg-brand {background-color:var(--color-brand)} @stillwind-slot utilities;",
        sourceUri("opaque root"),
      ),
    ).toStrictEqual({
      kind: "needFile",
      requests: [{ id: 1, importer: "opaque root", specifier: "tokens" }],
    });
    expect(
      loader.provide(1, "opaque child", "@theme { --color-brand: red; }", sourceUri("opaque child"))
        .kind,
    ).toBe("complete");
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
    expect(result.modules.length).toBe(2);
    expect(
      result.modules.every(
        (module: import("../binding.js").ProjectModuleOutput) => module.diagnostics.length === 0,
      ),
    ).toBeTruthy();
    const before = session.css();
    expect(before.length).toBeGreaterThan(0);
    expect(session.remove("a.tsx")).toBe(true);
    expect(session.css()).not.toBe(before);
    expect((session.css().match(/background-color:/g) ?? []).length).toBe(1);
    const next = session.newLoader();
    next.provide(
      0,
      "root",
      "@theme { --color-brand: blue; } :root,:host { @stillwind-slot theme; } @utility bg-brand {background-color:var(--color-brand)} @stillwind-slot utilities;",
      sourceUri("root"),
    );
    expect(session.reload(next).cssChanged).toBe(true);
    expect(session.css()).not.toBe(before);
    expect(session.remove("b.tsx")).toBe(true);
  });

  test("native loader accepts partial and reversed batches", async () => {
    const { NativeDesignSystemLoader } = await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader();
    const first = loader.provideMany([
      { id: 0, name: "root", uri: sourceUri("root"), content: "@import 'a'; @import 'b';" },
    ]);
    expect(
      first.requests.map((request: import("../binding.js").NativeCssRequest) => request.id),
    ).toStrictEqual([1, 2]);
    const next = loader.provide(2, "b", "@import 'c';", sourceUri("b"));
    expect(
      next.requests.map((request: import("../binding.js").NativeCssRequest) => request.id),
    ).toStrictEqual([1, 3]);
    const done = loader.provideMany([
      { id: 3, name: "c", uri: sourceUri("c"), content: "@theme { --color-child: green; }" },
      {
        id: 1,
        name: "a",
        uri: sourceUri("a"),
        content: "@theme { --color-brand: red; } @utility brand {color:var(--color-brand)}",
      },
    ]);
    expect(done.kind).toBe("complete");
    expect(new Set(done.sources)).toStrictEqual(new Set(["root", "a", "b", "c"]));
    expect(loader.generateVocabularyTypes()).toMatch(/brand/);
    expect(() => loader.provideMany([])).toThrow(/after completion/);
  });

  test("CSS load errors include the accepted document range", async () => {
    const { NativeDesignSystemLoader } = await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader();
    expect(() => loader.provide(0, "broken.css", "@theme {", "untitled:broken.css")).toThrow(
      /untitled:broken\.css:1:1-1:\d+: SW08026:/,
    );
  });

  test("CSS without a document URI remains an internal source", async () => {
    const { NativeDesignSystemLoader } = await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader();
    expect(() => loader.provide(0, "internal", "@theme {", null)).toThrow(
      /unterminated CSS block at byte 7/,
    );
  });

  test("native watch reload reports compilation errors without publishing a partial revision", async () => {
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
    expect(output.modules[0].diagnostics).toStrictEqual([]);
    const before = session.css();
    expect(before.length).toBeGreaterThan(0);
    const invalid = session.newLoader();
    invalid.provide(0, "theme", "", sourceUri("theme"));
    expect(() => session.reload(invalid)).toThrow(
      /CSS reload compilation failed.*motion\.tsx.*SW02003.*unknown or unavailable utility `brand`/s,
    );
    expect(session.diagnosticBatches()[0].reports[0].primary.location.uri).toBe(
      sourceUri("motion.tsx"),
    );
    expect(session.css()).toBe(before);
    const recovered = session.newLoader();
    recovered.provide(
      0,
      "theme",
      "@utility brand { color:blue; } @stillwind-slot utilities; @stillwind-slot keyframes;",
      sourceUri("theme"),
    );
    expect(session.reload(recovered).cssChanged).toBe(true);
    expect(session.diagnosticBatches()[0].reports).toStrictEqual([]);
    expect(session.css()).not.toBe(before);
  });

  test("native sessions join supplied type facts with another module's asset metadata", async () => {
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
    const summary = summarizeSource(
      producer.document.code,
      producer.moduleId,
      "tsx",
      "unambiguous",
    );
    const start = consumer.document.code.lastIndexOf("marker");
    const session = new NativeProjectSession(loader, true);
    const without = session.compile([consumer, producer]);
    expect(without.modules[0].diagnostics.length).toBeGreaterThan(0);
    const withFacts = session.compile([
      {
        ...consumer,
        typeFacts: [
          {
            start,
            end: start + "marker".length,
            value: summary.exports.find(
              (entry: import("../binding.js").NativeExport) => entry.name === "marker",
            ).value,
          },
        ],
      },
      producer,
    ]);
    expect(
      withFacts.modules.map(
        (module: import("../binding.js").ProjectModuleOutput) => module.moduleId,
      ),
    ).toStrictEqual([consumer.moduleId, producer.moduleId]);
    expect(
      withFacts.modules.every(
        (module: import("../binding.js").ProjectModuleOutput) => module.diagnostics.length === 0,
      ),
      JSON.stringify(withFacts),
    ).toBeTruthy();
    expect(withFacts.modules[0].code).toMatch(/sw_shared_/);
    expect(withFacts.modules[0].code).not.toMatch(/style\(marker\)/);
  });
}
