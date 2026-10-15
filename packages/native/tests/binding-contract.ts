import { pathToFileURL } from "node:url";
import { runInNewContext } from "node:vm";

import { expect, test } from "vite-plus/test";

import { stillwindCva } from "../../core/src/runtime/react.ts";

/**
 * Registers the output and validation contracts for a public binding entry.
 * The caller supplies a module specifier that resolves from this file to the native or WASM binding.
 * Tests import that entry when they run, and import failures fail the tests.
 */
export function registerBindingContract(bindingSpecifier: string) {
  test("browser targets retain color fallbacks unless every target supports the value", async () => {
    const { NativeDesignSystemLoader, compileSource } = await import(bindingSpecifier);
    const css =
      "@theme{--color-brand:red;}:root,:host{@stillwind-slot theme;}@utility tone{color:--compat-color(var(--color-brand),50%);}@utility current{color:--compat-color(currentcolor,50%);}@utility alpha{color:--compat-color(red,var(--opacity));}@utility authored{@supports (color:color-mix(in oklab,red,red)){color:color-mix(in oklab,var(--external) 50%,transparent);}}@stillwind-slot utilities;";
    for (const [target, fallback, alphaFallback] of [
      [undefined, true, true],
      [[], true, true],
      [["chrome110"], true, true],
      [["safari16.1"], true, true],
      [["safari16.2"], true, false],
      [["chrome111", "safari16.5"], true, false],
      [["chrome111", "firefox113", "safari17"], false, false],
    ] as const) {
      const options = {
        target: target && [...target],
      };
      const loader = new NativeDesignSystemLoader(options);
      loader.provide(0, "root.css", css);
      const query = loader.editorQuery();
      for (const candidate of ["tone", "current"]) {
        const preview = query.previewCandidate(candidate);
        expect(preview).toContain("color-mix(in oklab");
        expect(preview?.includes("@supports")).toBe(fallback);
      }
      expect(query.previewCandidate("alpha")?.includes("@supports")).toBe(alphaFallback);
      expect(query.previewCandidate("authored")).toContain("@supports");
      const output = compileSource(
        oneShotInput(
          'import {style} from "stillwind/react";export const tone=style("tone");export const current=style("current");',
          "targets.tsx",
          css,
        ),
        options,
      );
      expect(output.diagnostics).toEqual([]);
      expect(output.css.includes("@supports")).toBe(fallback);
    }
    expect(
      () =>
        new NativeDesignSystemLoader({
          target: ["unknown1"],
        }),
    ).toThrow("invalid browser target");
  });

  test("module summaries preserve UTF-8 module identities across the binding", async () => {
    const { summarizeSource } = await import(bindingSpecifier);
    for (const moduleId of ["short.ts", "a".repeat(16), "a".repeat(17), "日本/😀\0entry.ts"]) {
      const summary = summarizeSource("export const value = 1;", moduleId, "ts", "module");
      expect(summary.moduleId).toBe(moduleId);
    }
  });

  test("all stylesheet outputs retain development naming fallbacks and fatal render reports", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      "@utility flex {display:flex;} @layer { .authored { color:red; } @stillwind-slot utilities; }",
      sourceUri("theme.css"),
      "theme.css",
    );
    const naming = {
      layerName: () => {
        throw new Error("layer rejected");
      },
    };
    const session = new NativeProjectSession(loader, false, "development", naming);
    session.compile([
      {
        kind: "direct",
        moduleId: "entry.tsx",
        document: {
          uri: sourceUri("entry.tsx"),
          sourceMapSource: "entry.tsx",
          format: "jsx",
          code: 'import {style} from "stillwind/react"; export const card=style("flex");',
        },
        syntax: {
          language: "tsx",
          moduleKind: "module",
        },
      },
    ]);
    const base = session.baseCss();
    expect(base.rejectedNames).toHaveLength(1);
    expect(base.rejectedNames[0]).toMatchObject({
      reason: expect.stringContaining("layer rejected"),
      substituted: true,
    });
    const selection = {
      modules: ["entry.tsx"],
      coverage: { kind: "observed" },
      placement: "document",
    };
    for (const output of [
      session.css(),
      session.baseCssWithMap(),
      session.moduleCss("entry.tsx"),
      session.planStylesheet(selection),
      session.planStylesheet({
        ...selection,
        placement: "carrier",
      }),
      session.planStylesheet({
        ...selection,
        coverage: {
          kind: "complete",
          target: "app",
          revision: "1",
        },
      }),
    ]) {
      expect(output.css).toContain("@layer");
      expect(output.rejectedNames).toEqual(base.rejectedNames);
    }
    const layer = /@layer (sw-anon-[\w-]+)\{/.exec(base.css)?.[1];
    expect(layer).toBeTruthy();
    session.reserveAuthoredCss("authored.css", `@layer ${layer};`);
    for (const render of [() => session.css(), () => session.planStylesheet(selection)]) {
      try {
        render();
        expect.fail("Expected a generated layer collision");
      } catch (error) {
        expect(error).toMatchObject({
          rejectedNames: base.rejectedNames,
        });
        expect(String(error)).toMatch(/collision|collid/i);
      }
    }
    const release = new NativeProjectSession(loader, false, "release", naming);
    expect(() => release.baseCss()).toThrow(/layer rejected/);
  });

  test("fingerprints accept UTF-8 text and bytes and match diagnostic source revisions", async () => {
    const { fingerprint, compileSource } = await import(bindingSpecifier);
    expect(fingerprint("")).toBe(
      "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
    );
    const source = `/*${"日本語 🧭\r\n".repeat(1024)}*/\r\nimport {style} from "stillwind/react";export const props=style("missing");`;
    const revision = fingerprint(source);
    expect(fingerprint(new TextEncoder().encode(source))).toBe(revision);
    expect(fingerprint(source.replaceAll("\r\n", "\n"))).not.toBe(revision);
    const output = compileSource(
      oneShotInput(source, "revision.tsx", "@stillwind-slot utilities;"),
    );
    expect(output.reports[0].primary.location.revision).toBe(revision);
  });

  test("diagnostic collection keeps local utilities when external declarations cannot be lowered", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader({
      lint: {
        noImportant: "warn",
        unusedLintSuppression: "warn",
      },
    });
    loader.provide(0, "root.css", "@stillwind-slot utilities;", sourceUri("root.css"));
    const session = new NativeProjectSession(loader, false);
    const source = {
      kind: "direct",
      moduleId: "partial.tsx",
      document: {
        uri: sourceUri("partial.tsx"),
        format: "jsx",
        code: `import {style} from "stillwind/react";import {external} from "./missing";export const a=style(external);export const local=style("missing");
// swlint-disable-next-line no-important
export const clean=style('[display:block]');
// swlint-disable-next-line unknown-rule
export const valid=style('[display:block]');`,
      },
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
      typeFacts: [],
    };
    const reports = session.diagnose([source])[0].reports;
    expect(reports.map((report: { code: string }) => report.code)).toEqual(
      expect.arrayContaining(["SW30001", "unknown-utility"]),
    );
    expect(reports.map((report: { code: string }) => report.code)).toContain(
      "invalid-lint-suppression",
    );
    expect(reports.map((report: { code: string }) => report.code)).not.toContain(
      "unused-lint-suppression",
    );
    expect(session.diagnosticBatches()).toEqual([]);
    const output = session.compile([source]).modules[0];
    expect(output.code).toBeNull();
    expect(output.css).toBe("");
  });

  test("scan failures preserve original reports without exposing a partial summary", async () => {
    const { scanSource } = await import(bindingSpecifier);
    const original =
      "<!-- 日本語 🧭 -->\r\n<script>import {style} from 'stillwind/vue';const props=style('pad');</script>";
    const input = {
      kind: "projected",
      moduleId: "broken.vue",
      document: {
        uri: sourceUri("broken.vue"),
        format: "vue",
        code: original,
      },
      projection: {
        code: "const broken = (",
        syntax: {
          language: "ts",
          moduleKind: "module",
        },
        map: {
          kind: "volar",
          mappings: [],
        },
        converter: {
          name: "test-converter",
          version: "1.0.0",
          optionsFingerprint: "test",
        },
        importRealms: [],
        editableRegions: [],
      },
    };
    const syntax = scanSource(input);
    expect(syntax).toMatchObject({
      kind: "failed",
      batch: {
        module: "broken.vue",
        reports: [
          {
            code: "SW31004",
            primary: {
              scope: "document",
              location: {
                uri: sourceUri("broken.vue"),
                start: 0,
                end: Buffer.byteLength(original),
                endUtf16: original.length,
              },
            },
            notes: expect.arrayContaining(["Converter: test-converter@1.0.0"]),
          },
        ],
      },
    });
    expect("summary" in syntax).toBe(false);
    const invalidMap = scanSource({
      ...input,
      projection: {
        ...input.projection,
        code: "const okay=1;",
        map: {
          kind: "v3",
          source: "broken.vue",
          json: "{}",
        },
      },
    });
    expect(invalidMap).toMatchObject({
      kind: "failed",
      batch: {
        reports: [
          {
            code: "SW31001",
            primary: {
              scope: "document",
            },
          },
        ],
      },
    });
    const validGenerated = "import {style} from 'stillwind/vue';const props=style('pad');";
    expect(
      scanSource({
        ...input,
        projection: {
          ...input.projection,
          code: validGenerated,
        },
      }),
    ).toMatchObject({
      kind: "failed",
      batch: {
        reports: [
          {
            code: "SW31002",
            primary: {
              scope: "document",
            },
          },
        ],
      },
    });
  });

  test("source failures share native reporting and reject stale semantic artifacts until recovery", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader();
    const css = "@utility pad {padding:1px;} @stillwind-slot utilities;";
    loader.provide(0, "root", css, sourceUri("root"));
    const session = new NativeProjectSession(loader, true);
    const source =
      "// 日本語 🧭\r\nimport {style} from 'stillwind/react';const props=style('pad');";
    const input = {
      kind: "direct",
      moduleId: "attempt.tsx",
      document: {
        uri: sourceUri("attempt.tsx"),
        code: source,
        format: "jsx",
      },
      syntax: {
        language: "tsx",
        moduleKind: "module",
      },
    };
    expect(session.compile([input]).modules[0].diagnostics).toEqual([]);
    const before = session.css().css;
    const start = source.indexOf("props");
    const batch = session.reportSourceFailure({
      moduleId: input.moduleId,
      document: input.document,
      issue: {
        kind: "checker",
        detail: "required AST expression is missing",
      },
      range: {
        start,
        end: start + 5,
      },
    });
    expect(batch.reports[0]).toMatchObject({
      code: "SW30004",
      primary: {
        scope: "range",
        location: {
          startUtf16: start,
          endUtf16: start + 5,
          start: Buffer.byteLength(source.slice(0, start)),
          startLine: 1,
        },
      },
    });
    expect(session.diagnosticBatches()).toEqual([batch]);
    expect(session.css().css).toBe(before);
    const replacement = session.newLoader();
    replacement.provide(0, "root", css, sourceUri("root"));
    expect(() => session.reload(replacement)).toThrow(/SW30004/);
    expect(session.diagnosticBatches()).toEqual([batch]);
    expect(session.css().css).toBe(before);
    const surrogate = source.indexOf("🧭") + 1;
    expect(() =>
      session.reportSourceFailure({
        moduleId: input.moduleId,
        document: input.document,
        issue: {
          kind: "checker",
          detail: "invalid host range",
        },
        range: {
          start: surrogate,
          end: surrogate + 1,
        },
      }),
    ).toThrow(/UTF-16 offset/);
    expect(session.diagnosticBatches()).toEqual([batch]);
    expect(session.compile([input]).modules[0].reports).toEqual([]);
    expect(session.diagnosticBatches()[0].reports).toEqual([]);
    const failed = session.scanSource({
      ...input,
      document: {
        ...input.document,
        code: "const broken = (",
      },
    });
    expect(failed).toMatchObject({
      kind: "failed",
      batch: {
        reports: [
          {
            code: "SW10001",
          },
        ],
      },
    });
    expect(session.css().css).toBe(before);
    expect(session.diagnosticBatches()).toEqual([failed.batch]);
  });

  test("composition analysis agrees across bindings and refreshes transitive dependents", async () => {
    const {
      compileSource,
      compileProject,
      NativeDesignSystemLoader,
      NativeProjectSession,
      summarizeSource,
    } = await import(bindingSpecifier);
    const css =
      "@utility pad { padding:1px; } @utility alt { padding:2px; } @utility ink { color:red; } @stillwind-slot utilities;";
    const prepared = (code: string, moduleId: string) => ({
      kind: "direct",
      moduleId,
      document: {
        uri: sourceUri(moduleId),
        code,
        format: "jsx",
        sourceMapSource: sourceUri(moduleId),
      },
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
    });
    const directCode =
      "import {style} from 'stillwind/react'; const a=style('pad',flag&&'ink'); const b=style('alt'); export const x=style(a,b);";
    const loader = new NativeDesignSystemLoader();
    expect(loader.provide(0, "root", css, sourceUri("root")).kind).toBe("complete");
    const session = new NativeProjectSession(loader, true);
    const codes = (output: import("../binding.js").BindingOutput) =>
      output.lintFindings.map((finding) => finding.rule);
    const direct = prepared(directCode, "direct.tsx");
    const outputs = [
      compileSource(oneShotInput(directCode, direct.moduleId, css)),
      compileProject([direct], cssInput(css)).modules[0],
      session.compile([direct]).modules[0],
    ];
    for (const output of outputs) {
      expect(output.diagnostics).toStrictEqual([]);
      expect(codes(output)).toStrictEqual(["ambiguous-style-base-order"]);
    }
    const baseCode = "import {style} from 'stillwind/react'; export const base=style('pad');";
    const middleCode =
      "import {style} from 'stillwind/react'; import {base} from './base'; export const middle=style(base);";
    const appCode =
      "import {style} from 'stillwind/react'; import {middle} from './middle'; export const x=style(middle,'alt');";
    const exported = (code: string, module: string, name: string) => {
      const entry = summarizeSource(code, module, "tsx", "unambiguous").exports.find(
        (entry: import("../binding.js").NativeExport) => entry.name === name,
      );
      expect(entry).toBeDefined();
      return entry.value;
    };
    const withBase = (code: string, module: string, binding: string, value: unknown) => ({
      ...prepared(code, module),
      typeFacts: [
        {
          start: code.lastIndexOf(binding),
          end: code.lastIndexOf(binding) + binding.length,
          value,
        },
      ],
    });
    const base = prepared(baseCode, "base.tsx");
    const middle = withBase(
      middleCode,
      "middle.tsx",
      "base",
      exported(baseCode, base.moduleId, "base"),
    );
    const app = withBase(
      appCode,
      "app.tsx",
      "middle",
      exported(middleCode, middle.moduleId, "middle"),
    );
    for (const output of [
      compileProject([base, middle, app], cssInput(css)),
      compileProject([app, middle, base], cssInput(css)),
      session.compile([base, middle, app]),
      new NativeProjectSession(loader, true).compile([app, middle, base]),
    ]) {
      for (const module of output.modules) {
        expect(module.diagnostics).toStrictEqual([]);
      }
      expect(
        codes(
          output.modules.find(
            (module: import("../binding.js").ProjectModuleOutput) =>
              module.moduleId === app.moduleId,
          ),
        ),
      ).toStrictEqual(["interfering-imported-style-base"]);
    }
    expect(
      JSON.stringify(
        session
          .diagnosticBatches()
          .find(
            (batch: import("../binding.js").NativeDiagnosticBatch) => batch.module === app.moduleId,
          ),
      ),
    ).toContain("interfering-imported-style-base");
    expect(
      session.compile([prepared(`${baseCode}}`, base.moduleId)]).modules[0].diagnostics,
    ).not.toStrictEqual([]);
    expect(
      JSON.stringify(
        session
          .diagnosticBatches()
          .find(
            (batch: import("../binding.js").NativeDiagnosticBatch) => batch.module === app.moduleId,
          ),
      ),
    ).not.toContain("interfering-imported-style-base");
    expect(session.compile([base]).modules[0].diagnostics).toStrictEqual([]);
    expect(
      JSON.stringify(
        session
          .diagnosticBatches()
          .find(
            (batch: import("../binding.js").NativeDiagnosticBatch) => batch.module === app.moduleId,
          ),
      ),
    ).toContain("interfering-imported-style-base");
    expect(
      session.compile([prepared(baseCode.replace("'pad'", "'ink'"), base.moduleId)]).modules[0]
        .diagnostics,
    ).toStrictEqual([]);
    const batches = session.diagnosticBatches();
    const appBatch = batches.find(
      (batch: import("../binding.js").NativeDiagnosticBatch) => batch.module === app.moduleId,
    );
    expect(appBatch).toBeDefined();
    expect(JSON.stringify(appBatch)).not.toContain("interfering-imported-style-base");
    expect(session.remove(base.moduleId)).toBe(true);
    expect(
      JSON.stringify(
        session
          .diagnosticBatches()
          .find(
            (batch: import("../binding.js").NativeDiagnosticBatch) => batch.module === app.moduleId,
          ),
      ),
    ).not.toContain("interfering-imported-style-base");
  });

  test("minifyCss exposes final CSS processing and separate source maps", async () => {
    const { minifyCss } = await import(bindingSpecifier);
    const result = minifyCss(".a { color: red; &:hover { color: blue } } .unused { width: 1px }", {
      filename: "asset.css",
      targets: {
        chrome: 100 << 16,
      },
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
      targets: {
        chrome: 100 << 16,
      },
      exclude: ["Nesting"],
    });
    expect(preserved.css).toContain("&");
    expect(() => minifyCss(".a { color red }")).toThrow(/CSS parse failed/);
    expect(
      minifyCss(".a { color red }", {
        errorRecovery: true,
      }).warnings.length,
    ).toBeGreaterThan(0);
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

  test("naming callbacks preserve Unicode identities and names across session calls", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const moduleId = "部品/🧭.tsx";
    for (const name of ["小", "abcdefgh", "abcdefghijklmnop", "名称".repeat(20)]) {
      const loader = new NativeDesignSystemLoader();
      loader.provide(0, "theme.css", "@utility flex {display:flex;} @stillwind-slot utilities;");
      const seen: string[] = [];
      const naming: import("../binding.js").NativeIdentifierNaming = {
        utilityClass: (context) => {
          seen.push(context.module);
          return name;
        },
      };
      const session = new NativeProjectSession(loader, false, undefined, naming);
      for (const suffix of ["", "\n// edited source"]) {
        const output = session.compile([
          {
            kind: "direct",
            moduleId,
            document: {
              uri: sourceUri(moduleId),
              format: "jsx",
              code: `import {style} from 'stillwind/react'; export const card=style('flex');${suffix}`,
            },
            syntax: {
              language: "tsx",
              moduleKind: "module",
            },
          },
        ]);
        expect(output.modules[0].diagnostics).toEqual([]);
        expect(
          output.modules[0].symbols.find(
            (symbol: import("../binding.js").NativeGeneratedSymbol) => symbol.kind === "class",
          ),
        ).toMatchObject({ name, sourceKind: "module", sourceId: moduleId });
        expect(output.modules[0].code).toContain(name);
      }
      expect(seen).toEqual([moduleId, moduleId]);
    }
  });

  test("native properties use camelCase and enum values follow their declared cases", async () => {
    const { scanSource, NativeDesignSystemLoader } = await import(bindingSpecifier);
    for (const framework of ["react", "preact", "solid", "vue", "svelte", "astro"]) {
      const scan = scanSource({
        kind: "direct",
        moduleId: "contract.js",
        document: {
          uri: sourceUri("contract.js"),
          code: "module.exports = 1;",
          format: "script",
        },
        syntax: {
          language: "js",
          moduleKind: "commonjs",
        },
        framework,
      });
      expect(scan.kind).toBe("ready");
      const summary = scan.summary;
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
    const source =
      "import {style} from 'stillwind/react'; export const props=style('missing bad-[');";
    for (const severity of ["warn", "deny", "off"]) {
      const output = compileSource(oneShotInput(source, "lint.tsx", "@stillwind-slot utilities;"), {
        lint: {
          unknownUtility: severity,
          invalidUtility: severity,
        },
      });
      expect(output.diagnostics).toStrictEqual([]);
      expect(output.lintSettings["unknown-utility"]).toBe(severity);
      expect(output.lintSettings["invalid-utility"]).toBe(severity);
      expect(output.lintSettings["mutating-macro-argument"]).toBe("deny");
      expect(Object.hasOwn(output.lintSettings, "typo")).toBe(false);
      expect(output.lintFindings.length).toBe(severity === "off" ? 0 : 2);
      if (severity !== "off") {
        expect(output.lintFindings[0].rule).toBe("unknown-utility");
        expect(output.reports[0].severity).toBe(severity === "deny" ? "error" : "warning");
      }
      expect(output.code).toBeTruthy();
      expect(output.code).not.toContain("missing");
      expect(output.code).not.toContain("bad-[");
    }
  });

  test("CVA recovery preserves selection and omits complete invalid compounds", async () => {
    const { compileSource } = await import(bindingSpecifier);
    const css =
      "@utility a {color:red;} @utility b {color:blue;} @utility c {color:green;} @utility d {color:yellow;} @stillwind-slot utilities;";
    const evaluate = (code: string): ReturnType<typeof stillwindCva> =>
      runInNewContext(`${code.replace(/^import[^;]+;/gm, "")}\nchoose;`, {
        stillwindCva,
      });
    const selectedColors = (
      code: string,
      css: string,
      selection: Readonly<Record<string, unknown>>,
    ) => {
      const colors = new Map(
        [...css.matchAll(/\.([\w-]+)\{color:([^;]+);\}/g)].map((match) => [match[1], match[2]]),
      );
      const props = evaluate(code)(selection);
      if (typeof props.className !== "string") {
        expect.fail("Expected classes from supported CVA values");
      }
      return props.className.split(" ").map((name) => colors.get(name));
    };
    const source = `import {style} from 'stillwind/react';
      const fail=()=>{throw new Error('omitted input was evaluated');};
      const choose=style.cva('a', {
        [fail()]: style('c'), ...fail(),
        variants: {tone: {on: style('c'), off: 'b'}},
        defaultVariants: {tone: 'off', missing: fail()},
        compoundVariants: [,
          {missing: 'on', class: 'c'},
          {...fail(), class: 'c'},
          {tone: 'absent', class: 'c'},
          {tone: 'on', class: 'd'}
        ]
      });`;
    let baselineCss: string | undefined;
    for (const severity of ["deny", "warn", "off"]) {
      const output = compileSource(oneShotInput(source, "cva-recovery.ts", css), {
        lint: { invalidCvaInput: severity, invalidCvaConfig: severity },
      });
      expect(output.diagnostics).toStrictEqual([]);
      baselineCss ??= output.css;
      expect(output.css).toBe(baselineCss);
      expect(output.css).not.toContain("color:green");
      for (const [selection, colors] of [
        [{}, ["red", "blue"]],
        [{ tone: "on" }, ["red", "yellow"]],
        [{ tone: "off" }, ["red", "blue"]],
        [{ tone: null }, ["red"]],
      ] as const) {
        expect(selectedColors(output.code, output.css, selection)).toStrictEqual(colors);
      }
      expect(
        output.reports.every(
          (report: { severity: string }) =>
            report.severity === (severity === "deny" ? "error" : "warning"),
        ),
      ).toBe(true);
      if (severity === "off") {
        expect(output.reports).toStrictEqual([]);
      } else {
        expect(
          new Set(output.reports.map((report: { code: string }) => report.code)),
        ).toStrictEqual(new Set(["invalid-cva-config", "invalid-cva-input"]));
      }
    }
    const suppressed = compileSource(
      oneShotInput(
        `// swlint-disable invalid-cva-config, invalid-cva-input\n${source}`,
        "cva-recovery.ts",
        css,
      ),
    );
    expect(suppressed.reports).toStrictEqual([]);
    expect(selectedColors(suppressed.code, suppressed.css, { tone: "on" })).toStrictEqual([
      "red",
      "yellow",
    ]);
  });

  test("source restrictions and suppression usage agree across bindings and project APIs", async () => {
    const { compileSource, NativeDesignSystemLoader, NativeProjectSession } = await import(
      bindingSpecifier
    );
    const css = `@theme { --color-brand:blue; } @utility paint-* {color:--value(--color-*,[color]);}
@utility floating {position:fixed;} :root,:host { @stillwind-slot theme; } @stillwind-slot utilities;`;
    const source = `import {style} from 'stillwind/react';
export const color=style('paint-[#abc]');
export const spacing=style('[margin:13px]');
export const external=style({class:'legacy'});
export const restricted=style('floating');
export const important=style('[display:flex]!');
// swlint-disable-next-line no-important
export const used=style('[display:block]!');
// swlint-disable-next-line no-important
export const unused=style('[display:grid]');
// swlint-disable-next-line invalid-rule
export const invalidSuppression=style('paint-brand');`;
    const options: import("../binding.js").NativeCompilerOptions = {
      lint: {
        noArbitraryColors: "deny",
        noArbitrarySpacing: {
          severity: "warn",
        },
        noClassProperty: "warn",
        noImportant: "warn",
        noRestrictedUtilities: {
          utilities: [
            {
              pattern: "floating",
              message: "Use a layer",
            },
          ],
        },
        noRestrictedDeclarations: {
          rules: [
            {
              properties: ["position"],
              values: ["fixed"],
            },
          ],
        },
        unusedLintSuppression: "warn",
        preferCanonicalUtilities: {
          severity: "off",
        },
      },
    };
    const expected = [
      "no-arbitrary-colors",
      "no-arbitrary-spacing",
      "no-class-property",
      "no-restricted-utilities",
      "no-restricted-declarations",
      "no-important",
      "unused-lint-suppression",
      "invalid-lint-suppression",
    ].sort();
    const output = compileSource(oneShotInput(source, "source-policies.tsx", css), options);
    expect(output.diagnostics).toEqual([]);
    expect(output.reports.map((report: { code: string }) => report.code).sort()).toEqual(expected);
    expect(
      output.reports.find((report: { code: string }) => report.code === "no-arbitrary-colors")
        .severity,
    ).toBe("error");
    const loader = new NativeDesignSystemLoader(options);
    loader.provide(0, "root.css", css, sourceUri("root.css"));
    const session = new NativeProjectSession(loader, false);
    const prepared = {
      kind: "direct",
      moduleId: "source-policies.tsx",
      document: {
        code: source,
        uri: sourceUri("source-policies.tsx"),
        format: "jsx",
      },
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
    };
    expect(
      session
        .diagnose([prepared])[0]
        .reports.map((report: { code: string }) => report.code)
        .sort(),
    ).toEqual(expected);
    expect(
      session
        .compile([prepared])
        .modules[0].reports.map((report: { code: string }) => report.code)
        .sort(),
    ).toEqual(expected);
    for (const lint of [
      { noArbitrarySpacing: { properties: ["padding left"] } },
      { noRestrictedUtilities: { utilities: [{ pattern: "" }] } },
      { noRestrictedDeclarations: { rules: [{ properties: ["position"], values: [] }] } },
      { noImportant: "error" },
    ]) {
      expect(() => new NativeDesignSystemLoader({ lint })).toThrow();
    }
  });

  test("utility policies share configuration and diagnostics across one-shot and session APIs", async () => {
    const { compileSource, NativeDesignSystemLoader, NativeProjectSession } = await import(
      bindingSpecifier
    );
    const css = `:root { @stillwind-slot theme; } @theme inline { --color-red-500:red; --color-danger:var(--color-red-500); }
@utility p2 {padding:0.5rem} @utility px4 {padding-inline:1rem} @utility text-* {color:--value(--color-*,[color]);}
@custom-variant child (&>span); @custom-variant before (&::before);
@custom-variant part (&::part(label)); @custom-variant child-before (&>span::before);
@utility internal {&>span {color:red;}} @stillwind-slot utilities;`;
    const source = `import {style} from 'stillwind/react';
export const duplicate=style('p2 p2'); export const conflict=style('p2 px4');
export const policies=style('child:[padding-left:2px] text-red-500');`;
    const options: import("../binding.js").NativeCompilerOptions = {
      lint: {
        preferDirectStyling: "warn",
        duplicateUtility: "deny",
        preferLogicalProperties: "warn",
        noRestrictedLengthUnits: {
          rules: [
            {
              properties: ["padding*"],
              disallow: ["px"],
            },
          ],
        },
        noRestrictedThemeTokens: {
          tokens: ["--color-red-*"],
        },
      },
    };
    const expected = [
      "duplicate-utility",
      "conflicting-utilities",
      "prefer-direct-styling",
      "prefer-logical-properties",
      "no-restricted-length-units",
      "no-restricted-theme-tokens",
    ].sort();
    const output = compileSource(oneShotInput(source, "policies.tsx", css), options);
    expect(output.diagnostics).toEqual([]);
    expect(output.reports.map((report: { code: string }) => report.code).sort()).toEqual(expected);
    expect(
      output.reports.find((report: { code: string }) => report.code === "duplicate-utility")
        .severity,
    ).toBe("error");
    const loader = new NativeDesignSystemLoader(options);
    loader.provide(0, "root.css", css, sourceUri("root.css"));
    const session = new NativeProjectSession(loader, false);
    const prepared = {
      kind: "direct",
      moduleId: "policies.tsx",
      document: {
        code: source,
        uri: sourceUri("policies.tsx"),
        format: "jsx",
      },
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
    };
    expect(
      session
        .diagnose([prepared])[0]
        .reports.map((report: { code: string }) => report.code)
        .sort(),
    ).toEqual(expected);
    expect(
      session
        .compile([prepared])
        .modules[0].reports.map((report: { code: string }) => report.code)
        .sort(),
    ).toEqual(expected);
    const allowed = compileSource(
      oneShotInput(
        "import {style} from 'stillwind/react';const x=style('p2 px4 text-danger before:p2 part:p2 internal');",
        "allowed.tsx",
        css,
      ),
      {
        ...options,
        lint: {
          ...options.lint,
          conflictingUtilities: {
            allowShorthandOverrides: true,
          },
        },
      },
    );
    expect(allowed.reports).toEqual([]);
    const indirect = compileSource(
      oneShotInput(
        "import {style} from 'stillwind/react';const x=style('child-before:p2');",
        "indirect.tsx",
        css,
      ),
      {
        lint: {
          preferDirectStyling: "deny",
        },
      },
    );
    expect(indirect.diagnostics).toEqual([]);
    expect(
      indirect.reports.map((report: { code: string; severity: string }) => ({
        code: report.code,
        severity: report.severity,
      })),
    ).toEqual([{ code: "prefer-direct-styling", severity: "error" }]);
  });

  test("theme token restrictions cover variant selections and arbitrary breakpoints exclude containers", async () => {
    const { compileSource, NativeDesignSystemLoader, NativeProjectSession } = await import(
      bindingSpecifier
    );
    const css = `@theme inline { --spacing:0.25rem; --spacing-ref-small:1rem; --spacing-gutter:var(--spacing-ref-small); --duration-ref-fast:100ms; }
@utility p-* {padding:--value(--spacing-*);} @utility flex {display:flex;} @utility gap {gap:1rem;}
@variant speed-* { & {transition-duration:--value(--duration-*); @slot;} }
@variant max-* { @media (width < --value([length])) { @slot; } }
@variant scaled-* { @media (width >= --value(--spacing-*, [length])) { @slot; } }
@stillwind-variant supports-* (value: [supports-condition]) { @supports (--value()) { @slot; } }
@variant tablet { @media (width >= 900px) { @slot; } }
@stillwind-slot utilities;`;
    const source = `import {style} from 'stillwind/react';
export const a=style('p-ref-small');
export const b=style({'speed-ref-fast:':['flex','gap']});
export const c=style({'max-[900px]:':['flex','gap']});
export const d=style('[@media(min-height:600px)]:flex');
export const e=style('tablet:p-gutter');
export const f=style('[@container(width>=900px)]:flex');`;
    const options: import("../binding.js").NativeCompilerOptions = {
      lint: {
        noRestrictedThemeTokens: { tokens: ["--spacing-ref-*", "--duration-ref-*"] },
        noArbitraryBreakpoints: "deny",
      },
    };
    const output = compileSource(oneShotInput(source, "responsive.tsx", css), options);
    expect(output.diagnostics).toEqual([]);
    expect(output.reports.map((report: { code: string }) => report.code).sort()).toEqual([
      "no-arbitrary-breakpoints",
      "no-arbitrary-breakpoints",
      "no-restricted-theme-tokens",
      "no-restricted-theme-tokens",
    ]);
    expect(
      output.reports
        .filter((report: { code: string }) => report.code === "no-arbitrary-breakpoints")
        .map((report: { severity: string }) => report.severity),
    ).toEqual(["error", "error"]);
    const loader = new NativeDesignSystemLoader(options);
    loader.provide(0, "root.css", css, sourceUri("root.css"));
    const session = new NativeProjectSession(loader, false);
    const prepared = {
      kind: "direct",
      moduleId: "responsive.tsx",
      document: {
        code: source,
        uri: sourceUri("responsive.tsx"),
        format: "jsx",
      },
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
    };
    expect(session.diagnose([prepared])[0].reports).toEqual(output.reports);
    const suppressed = compileSource(
      oneShotInput(
        `import {style} from 'stillwind/react';
// swlint-disable-next-line no-arbitrary-breakpoints
const x=style('max-[900px]:flex');`,
        "suppressed.tsx",
        css,
      ),
      options,
    );
    expect(suppressed.reports).toEqual([]);
    const variables = compileSource(
      oneShotInput(
        `import {style} from 'stillwind/react';
const a=style('supports-[padding:var(--spacing-ref-small)]:flex');
const b=style('max-(--spacing-ref-small):flex');
const c=style('supports-[transition-duration:var(--duration-ref-fast)]:flex');
const d=style('supports-[padding:var(--spacing-gutter)]:flex');`,
        "variables.tsx",
        css,
      ),
      {
        lint: {
          noRestrictedThemeTokens: options.lint!.noRestrictedThemeTokens,
        },
      },
    );
    expect(variables.diagnostics).toEqual([]);
    expect(variables.reports.map((report: { code: string }) => report.code)).toEqual([
      "no-restricted-theme-tokens",
      "unknown-utility",
      "no-restricted-theme-tokens",
    ]);
    const spacing = compileSource(
      oneShotInput(
        "import {style} from 'stillwind/react';const a=style('scaled-gutter:flex');const b=style('scaled-[3rem]:flex');",
        "spacing.tsx",
        css,
      ),
      {
        lint: {
          noArbitraryBreakpoints: "warn",
        },
      },
    );
    expect(spacing.diagnostics).toEqual([]);
    expect(spacing.reports.map((report: { code: string }) => report.code)).toEqual([
      "no-arbitrary-breakpoints",
    ]);
  });

  test("utility policy validation rejects incomplete token policies and invalid length units", async () => {
    const { NativeDesignSystemLoader, compileSource } = await import(bindingSpecifier);
    for (const lint of [
      {
        noRestrictedThemeTokens: {},
      },
      {
        noRestrictedThemeTokens: {
          tokens: [],
        },
      },
      {
        noRestrictedThemeTokens: {
          tokens: ["spacing-*"],
        },
      },
      {
        noRestrictedThemeTokens: {
          tokens: ["--"],
        },
      },
      {
        noRestrictedLengthUnits: {
          rules: [
            {
              properties: ["padding"],
              disallow: ["s"],
            },
          ],
        },
      },
    ]) {
      expect(() => new NativeDesignSystemLoader({ lint })).toThrow(/lint|tokens|length/i);
    }
    expect(
      () =>
        new NativeDesignSystemLoader({
          lint: {
            noRestrictedThemeTokens: {
              severity: "off",
            },
          },
        }),
    ).not.toThrow();
    expect(() =>
      compileSource(oneShotInput("export {};", "invalid.ts", ""), {
        lint: {
          noRestrictedThemeTokens: {
            severity: "warn",
          },
        },
      }),
    ).toThrow(/tokens/);
  });

  test("host options control detection and severity of typed relationship findings", async () => {
    const { compileSource } = await import(bindingSpecifier);
    const source = "import {style} from 'stillwind/react';export const props=style('@sm:flex');";
    for (const severity of ["off", "warn", "deny"]) {
      const output = compileSource(
        oneShotInput(
          source,
          "relationships.tsx",
          "@utility flex{display:flex} @stillwind-slot utilities;",
        ),
        {
          lint: {
            preferTypedRelationships: severity,
          },
        },
      );
      const findings = output.reports.filter(
        (report: { code: string }) => report.code === "prefer-typed-relationships",
      );
      expect(findings.length).toBe(severity === "off" ? 0 : 1);
      if (severity !== "off") {
        expect(findings[0].severity).toBe(severity === "deny" ? "error" : "warning");
      }
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
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
    };
    expect(scanSource(input)).toMatchObject({
      kind: "ready",
      summary: {
        moduleId: input.moduleId,
      },
    });
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
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
    });
    for (const watch of [true, false]) {
      const session = new NativeProjectSession(loader, watch);
      session.compile([source("a.ts", "'red'"), source("b.ts", "'red'")]);
      const css = session.css().css;
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
        expect(session.css().css).toBe(css);
      }
      expect(() =>
        session.planStylesheet({
          modules: ["a.ts", "b.ts"],
          coverage: {
            kind: "observed",
          },
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
          coverage: {
            kind: "observed",
          },
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
        syntax: {
          language: "tsx",
          moduleKind: "unambiguous",
        },
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
      session.compile([
        {
          ...sources[0],
          syntax: {
            ...sources[0].syntax,
            language: "unknown",
          },
        },
      ]),
    ).toThrow(/language/);
    expect(() =>
      session.planStylesheet({
        modules: [],
        coverage: {
          kind: "unknown",
        },
        placement: "document",
      }),
    ).toThrow(/kind|variant/);
    expect(() =>
      session.planStylesheet({
        modules: [],
        coverage: {
          kind: "complete",
        },
        placement: "document",
      }),
    ).toThrow();
    expect(() =>
      session.planStylesheet({
        modules: [],
        coverage: {
          kind: "observed",
        },
        placement: "unknown",
      }),
    ).toThrow(/placement/);
    const plan = session.planStylesheet({
      modules: ["🧭.tsx"],
      coverage: {
        kind: "observed",
      },
      placement: "document",
    });
    expect(typeof plan).toBe("object");
    expect(plan.css).toMatch(/display:flex/);
    expect(plan.producers[0].module).toBe("🧭.tsx");
    expect(plan.sourceMap.entries.length).toBeGreaterThan(0);
    const unmapped = session.planStylesheet(
      {
        modules: ["🧭.tsx"],
        coverage: {
          kind: "observed",
        },
        placement: "document",
      },
      false,
    );
    expect(unmapped).toStrictEqual({
      ...plan,
      sourceMap: {
        sources: [],
        entries: [],
      },
      parts: plan.parts.map((part: import("../binding.js").NativeStylesheetPart) => ({
        ...part,
        sourceMap: {
          sources: [],
          entries: [],
        },
      })),
    });
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
        syntax: {
          language: "tsx",
          moduleKind: "unambiguous",
        },
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
        syntax: {
          language: "tsx",
          moduleKind: "unambiguous",
        },
      },
    ]);
    const shared = session.css().css;
    expect(session.remove("a.ts")).toBe(true);
    expect(
      session
        .diagnosticBatches()
        .some((batch: import("../binding.js").NativeDiagnosticBatch) => batch.module === "a.ts"),
    ).toBe(false);
    expect(session.css().css).not.toBe(shared);
    expect(session.remove("a.ts")).toBe(false);
    expect(session.remove("b.ts")).toBe(true);
    expect(session.css().css).not.toBe(shared);
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
            syntax: {
              language: "tsx",
              moduleKind: "unambiguous",
            },
            typeFacts: [
              {
                start: 0,
                end: 0,
                value,
              },
            ],
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
    const symbolic = (kind: string) => ({
      kind,
      candidates: {
        kind: "universe",
      },
    });
    const finite = (kind: string, values: unknown[]) => ({
      kind,
      candidates: {
        kind: "finite",
        values,
      },
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
        syntax: {
          language: "tsx",
          moduleKind: "unambiguous",
        },
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
            syntax: {
              language: "tsx",
              moduleKind: "unambiguous",
            },
          },
        ],
        cssInput(css),
      ).modules[0].diagnostics,
    ).toStrictEqual([]);
    expect(() => session.css().css).toThrow(/@stillwind-slot/);
    expect(() => session.moduleCss("button.tsx").css).toThrow(/@stillwind-slot/);
    expect(() =>
      session.planStylesheet({
        modules: ["button.tsx"],
        coverage: {
          kind: "observed",
        },
        placement: "document",
      }),
    ).toThrow(/@stillwind-slot/);
    expect(() => renderStylesheet(result.modules[0].css, css)).toThrow(/@stillwind-slot/);
    const standalone = compileSource(oneShotInput(source, "button.tsx", css));
    expect(standalone.diagnostics[0].code).toBe("SW14005");
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
    const valid = session.css().css;
    expect(valid).toMatch(/display:flex/);
    expect(() => session.reload(loader)).toThrow(/@stillwind-slot/);
    expect(session.css().css).toBe(valid);
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
        syntax: {
          language: "ts",
          moduleKind: "unambiguous",
        },
        map: {
          kind: "volar",
          mappings: [
            {
              sourceOffsets: [prefix.length],
              generatedOffsets: [0],
              lengths: [source.length],
            },
          ],
        },
        converter: {
          name: "svelte2tsx",
          version: "0.7.61",
          optionsFingerprint: "test",
        },
        importRealms: [
          {
            id: 0,
            start: prefix.length,
            end: prefix.length + source.length,
          },
        ],
        editableRegions: [
          {
            start: prefix.length,
            end: prefix.length + source.length,
            importRealm: 0,
          },
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
    const css = session.css().css;
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
      {
        ...input,
        projection: {
          ...input.projection,
          editableRegions: [],
        },
      },
    ]).modules[0];
    expect(outsideRegions.diagnostics[0].code).toBe("SW31010");
    expect(session.css().css).toBe(css);
    const invalid = {
      ...input,
      projection: {
        ...input.projection,
        map: {
          kind: "volar",
          mappings: [],
        },
      },
    };
    const failed = session.compile([invalid]).modules[0];
    expect(failed.diagnostics[0].code).toBe("SW31002");
    expect(session.css().css).toBe(css);
    const recovered = session.compile([input]).modules[0];
    expect(recovered.diagnostics).toStrictEqual([]);
    expect(recovered.code).toBe(before.code);
    for (const field of ["code", "syntax", "map", "converter", "importRealms", "editableRegions"]) {
      expect(() =>
        session.compile([
          {
            ...input,
            projection: {
              ...input.projection,
              [field]: undefined,
            },
          },
        ]),
      ).toThrow(new RegExp(field));
    }
    expect(() =>
      session.compile([
        {
          ...input,
          document: {
            ...input.document,
            format: undefined,
          },
        },
      ]),
    ).toThrow(/format/);
    expect(() =>
      session.compile([
        {
          ...input,
          document: {
            ...input.document,
            format: "jsx",
          },
        },
      ]),
    ).toThrow(/component|format/);
    const inferredAdapter = session.compile([
      {
        ...input,
        framework: undefined,
      },
    ]).modules[0];
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
        syntax: {
          language: "tsx",
          moduleKind: "unambiguous",
        },
      },
    ];
    const before = session.compile(sources);
    const beforeCss = session.css().css;
    const update = session.reload(load("second", session));
    expect(update.cssChanged).toBe(true);
    expect(update.transformedModules).toStrictEqual(["button.tsx"]);
    expect(session.css().css).not.toBe(beforeCss);
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
      requests: [
        {
          id: 1,
          importer: "opaque root",
          specifier: "tokens",
        },
      ],
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
        syntax: {
          language: "tsx",
          moduleKind: "unambiguous",
        },
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
        syntax: {
          language: "tsx",
          moduleKind: "unambiguous",
        },
      },
    ]);
    expect(result.modules.length).toBe(2);
    expect(
      result.modules.every(
        (module: import("../binding.js").ProjectModuleOutput) => module.diagnostics.length === 0,
      ),
    ).toBeTruthy();
    const before = session.css().css;
    expect(before.length).toBeGreaterThan(0);
    expect(session.remove("a.tsx")).toBe(true);
    expect(session.css().css).not.toBe(before);
    expect((session.css().css.match(/background-color:/g) ?? []).length).toBe(1);
    const next = session.newLoader();
    next.provide(
      0,
      "root",
      "@theme { --color-brand: blue; } :root,:host { @stillwind-slot theme; } @utility bg-brand {background-color:var(--color-brand)} @stillwind-slot utilities;",
      sourceUri("root"),
    );
    expect(session.reload(next).cssChanged).toBe(true);
    expect(session.css().css).not.toBe(before);
    expect(session.remove("b.tsx")).toBe(true);
  });

  test("native loader accepts partial and reversed batches", async () => {
    const { NativeDesignSystemLoader } = await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader();
    const first = loader.provideMany([
      {
        id: 0,
        name: "root",
        uri: sourceUri("root"),
        content: "@import 'a'; @import 'b';",
      },
    ]);
    expect(
      first.requests.map((request: import("../binding.js").NativeCssRequest) => request.id),
    ).toStrictEqual([1, 2]);
    const next = loader.provide(2, "b", "@import 'c';", sourceUri("b"));
    expect(
      next.requests.map((request: import("../binding.js").NativeCssRequest) => request.id),
    ).toStrictEqual([1, 3]);
    const done = loader.provideMany([
      {
        id: 3,
        name: "c",
        uri: sourceUri("c"),
        content: "@theme { --color-child: green; }",
      },
      {
        id: 1,
        name: "a",
        uri: sourceUri("a"),
        content: "@theme { --color-brand: red; } @utility brand {color:var(--color-brand)}",
      },
    ]);
    expect(done.kind).toBe("complete");
    expect(new Set(done.sources)).toStrictEqual(new Set(["root", "a", "b", "c"]));
    expect(() => loader.provideMany([])).toThrow(/after completion/);
  });

  test("CSS load errors include the accepted document range", async () => {
    const { NativeDesignSystemLoader } = await import(bindingSpecifier);
    const loader = new NativeDesignSystemLoader();
    try {
      loader.provide(0, "broken.css", "@theme {", "untitled:broken.css");
      expect.fail("Invalid CSS must reject loading");
    } catch (error) {
      expect(error).toMatchObject({
        message: expect.stringContaining("unterminated CSS block"),
        reports: [
          {
            code: "SW13002",
            severity: "error",
            primary: {
              scope: "range",
              location: {
                uri: "untitled:broken.css",
                startLine: 0,
                startColumn: 0,
              },
            },
          },
        ],
      });
    }
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
        syntax: {
          language: "tsx",
          moduleKind: "unambiguous",
        },
      },
    ]);
    expect(output.modules[0].diagnostics).toStrictEqual([]);
    const before = session.css().css;
    expect(before.length).toBeGreaterThan(0);
    const invalid = session.newLoader();
    invalid.provide(
      0,
      "theme",
      "@utility brand {@stillwind-require-properties invalid; color:red;} @stillwind-slot utilities; @stillwind-slot keyframes;",
      sourceUri("theme"),
    );
    expect(() => session.reload(invalid)).toThrow(
      /CSS reload compilation failed.*motion\.tsx.*SW14008.*invalid property requirement/s,
    );
    expect(session.diagnosticBatches()[0].reports[0].primary.location.uri).toBe(
      sourceUri("motion.tsx"),
    );
    expect(session.css().css).toBe(before);
    const recovered = session.newLoader();
    recovered.provide(
      0,
      "theme",
      "@utility brand { color:blue; } @stillwind-slot utilities; @stillwind-slot keyframes;",
      sourceUri("theme"),
    );
    expect(session.reload(recovered).cssChanged).toBe(true);
    expect(session.diagnosticBatches()[0].reports).toStrictEqual([]);
    expect(session.css().css).not.toBe(before);
  });

  test("checked non-assets are omitted while absent or nonfinite facts remain fatal", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const code =
      "import {style} from 'stillwind/react'; import {candidate} from './values'; const x=style('flex',candidate.asAnchor());";
    const start = code.lastIndexOf("candidate");
    const source = {
      kind: "direct",
      moduleId: "consumer.ts",
      document: { uri: sourceUri("consumer.ts"), code, format: "script" },
      syntax: { language: "ts", moduleKind: "module" },
    };
    for (const severity of ["deny", "warn", "off"]) {
      const loader = new NativeDesignSystemLoader({ lint: { invalidRefInput: severity } });
      loader.provide(
        0,
        "theme",
        "@utility flex {display:flex;} @stillwind-slot utilities;",
        sourceUri("theme"),
      );
      const session = new NativeProjectSession(loader, false);
      const facts = { start, end: start + "candidate".length, value: null };
      const recovered = session.compile([{ ...source, typeFacts: [facts] }]).modules[0];
      expect(recovered.diagnostics).toStrictEqual([]);
      expect(recovered.code).not.toContain("asAnchor");
      expect(session.css().css).toContain("display:flex");
      expect(
        recovered.lintFindings.map(
          (finding: import("../binding.js").BindingLintFinding) => finding.rule,
        ),
      ).toStrictEqual(severity === "off" ? [] : ["invalid-ref-input"]);
      for (const typeFacts of [
        [],
        [
          {
            ...facts,
            value: { kind: "refs", value: { kind: "ref", candidates: { kind: "universe" } } },
          },
        ],
      ]) {
        const rejected = session.compile([{ ...source, typeFacts }]).modules[0];
        expect(
          rejected.diagnostics.map(
            (finding: import("../binding.js").BindingDiagnostic) => finding.code,
          ),
        ).toContain("SW30001");
        expect(rejected.code).toBeNull();
      }
      expect(() =>
        session.compile([{ ...source, typeFacts: [{ start, end: facts.end }] }]),
      ).toThrow();
    }
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
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
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
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
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
