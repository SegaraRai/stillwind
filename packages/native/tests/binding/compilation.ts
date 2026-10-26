import { runInNewContext } from "node:vm";

import { expect, test } from "vite-plus/test";

import { stillwindCva } from "../../../core/src/runtime/class-name-object.ts";
import { sourceUri, cssInput, oneShotInput } from "./support.ts";

/**
 * Registers compilation tests for the selected native or WASM binding.
 *
 * @param bindingSpecifier - Absolute URL of the binding module.
 */
export function registerCompilationContract(bindingSpecifier: string): void {
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
    const codes = (output: import("../../binding.js").BindingOutput) =>
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
        (entry: import("../../binding.js").NativeExport) => entry.name === name,
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
            (module: import("../../binding.js").ProjectModuleOutput) =>
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
            (batch: import("../../binding.js").NativeDiagnosticBatch) =>
              batch.module === app.moduleId,
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
            (batch: import("../../binding.js").NativeDiagnosticBatch) =>
              batch.module === app.moduleId,
          ),
      ),
    ).not.toContain("interfering-imported-style-base");
    expect(session.compile([base]).modules[0].diagnostics).toStrictEqual([]);
    expect(
      JSON.stringify(
        session
          .diagnosticBatches()
          .find(
            (batch: import("../../binding.js").NativeDiagnosticBatch) =>
              batch.module === app.moduleId,
          ),
      ),
    ).toContain("interfering-imported-style-base");
    expect(
      session.compile([prepared(baseCode.replace("'pad'", "'ink'"), base.moduleId)]).modules[0]
        .diagnostics,
    ).toStrictEqual([]);
    const batches = session.diagnosticBatches();
    const appBatch = batches.find(
      (batch: import("../../binding.js").NativeDiagnosticBatch) => batch.module === app.moduleId,
    );
    expect(appBatch).toBeDefined();
    expect(JSON.stringify(appBatch)).not.toContain("interfering-imported-style-base");
    expect(session.remove(base.moduleId)).toBe(true);
    expect(
      JSON.stringify(
        session
          .diagnosticBatches()
          .find(
            (batch: import("../../binding.js").NativeDiagnosticBatch) =>
              batch.module === app.moduleId,
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

  test("naming callbacks preserve Unicode identities and names across session calls", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const moduleId = "部品/🧭.tsx";
    for (const name of ["小", "abcdefgh", "abcdefghijklmnop", "名称".repeat(20)]) {
      const loader = new NativeDesignSystemLoader();
      loader.provide(0, "theme.css", "@utility flex {display:flex;} @stillwind-slot utilities;");
      const seen: string[] = [];
      const naming: import("../../binding.js").NativeIdentifierNaming = {
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
            (symbol: import("../../binding.js").NativeGeneratedSymbol) => symbol.kind === "class",
          ),
        ).toMatchObject({ name, sourceKind: "module", sourceId: moduleId });
        expect(output.modules[0].code).toContain(name);
      }
      expect(seen).toEqual([moduleId, moduleId]);
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
}
