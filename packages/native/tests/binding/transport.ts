import { expect, test } from "vite-plus/test";

import { sourceUri, cssInput, oneShotInput } from "./support.ts";

/**
 * Registers binding transport tests for the selected native or WASM binding.
 *
 * @param bindingSpecifier - Absolute URL of the binding module.
 */
export function registerTransportContract(bindingSpecifier: string): void {
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
        (module: import("../../binding.js").ProjectModuleOutput) => module.moduleId,
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
      (entry: import("../../binding.js").NativeExport) => entry.name === "handle",
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
      parts: plan.parts.map((part: import("../../binding.js").NativeStylesheetPart) => ({
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

  test("loads the selected package through its public entry", async () => {
    const binding = await import(bindingSpecifier);
    expect(typeof binding.compileSource).toBe("function");
    expect(typeof binding.compileProject).toBe("function");
  });
}
