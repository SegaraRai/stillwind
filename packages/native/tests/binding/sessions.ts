import { expect, test } from "vite-plus/test";

import { sourceUri, cssInput, oneShotInput } from "./support.ts";

/**
 * Registers project session tests for the selected native or WASM binding.
 *
 * @param bindingSpecifier - Absolute URL of the binding module.
 */
export function registerSessionsContract(bindingSpecifier: string): void {
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
          (module: import("../../binding.js").ProjectModuleOutput) => module.diagnostics.length,
        ),
      ).toBeTruthy();
      const attempted = session
        .diagnosticBatches()
        .find((batch: import("../../binding.js").NativeDiagnosticBatch) => batch.module === "b.ts");
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
          .find(
            (batch: import("../../binding.js").NativeDiagnosticBatch) => batch.module === "b.ts",
          ).reports,
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
        .some((batch: import("../../binding.js").NativeDiagnosticBatch) => batch.module === "a.ts"),
    ).toBe(false);
    expect(session.css().css).not.toBe(shared);
    expect(session.remove("a.ts")).toBe(false);
    expect(session.remove("b.ts")).toBe(true);
    expect(session.css().css).not.toBe(shared);
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
    expect(standalone.diagnostics[0].code).toBe("SW14001");
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

  test("session reload applies code generation options independently of CSS definitions", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const load = (
      classNamespace: string,
      session?: import("../../binding.js").NativeProjectSession,
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
        (module: import("../../binding.js").ProjectModuleOutput) => module.diagnostics.length === 0,
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
      first.requests.map((request: import("../../binding.js").NativeCssRequest) => request.id),
    ).toStrictEqual([1, 2]);
    const next = loader.provide(2, "b", "@import 'c';", sourceUri("b"));
    expect(
      next.requests.map((request: import("../../binding.js").NativeCssRequest) => request.id),
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
      /CSS reload compilation failed.*motion\.tsx.*SW14004.*invalid property requirement/s,
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
}
