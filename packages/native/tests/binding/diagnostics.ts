import { expect, test } from "vite-plus/test";

import { sourceUri, oneShotInput } from "./support.ts";

/**
 * Registers diagnostic tests for the selected native or WASM binding.
 *
 * @param bindingSpecifier - Absolute URL of the binding module.
 */
export function registerDiagnosticsContract(bindingSpecifier: string): void {
  test("plugin token restrictions retain source reports through a dynamic reload", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(bindingSpecifier);
    const source =
      'import {style} from "stillwind/react"; const label = "😀"; export const icon = style("icon-demo");';
    const input = {
      kind: "direct",
      moduleId: "icon.tsx",
      document: {
        uri: sourceUri("icon.tsx"),
        code: source,
        format: "jsx",
      },
      syntax: {
        language: "tsx",
        moduleKind: "unambiguous",
      },
    } as const;
    const rootCss = '@plugin "demo"; :root { @stillwind-slot theme; } @stillwind-slot utilities;';
    const family = "@stillwind-dynamic icon-*;";
    const definition = `${family}
@theme { --icon-svg-demo: url("data:image/svg+xml,<svg/>"); }
@utility icon-demo { mask-image: var(--icon-svg-demo); }`;
    for (const severity of ["warn", "deny"] as const) {
      for (const definitionUri of [undefined, sourceUri("plugin.css")]) {
        const options = {
          lint: {
            noRestrictedThemeTokens: {
              severity,
              tokens: ["--icon-svg-*"],
            },
          },
        };
        const loader = new NativeDesignSystemLoader(options);
        expect(loader.provide(0, "root.css", rootCss, sourceUri("root.css")).kind).toBe("needFile");
        expect(loader.provide(1, "plugin", family, definitionUri).kind).toBe("complete");
        const session = new NativeProjectSession(loader, false);
        expect(session.compile([input]).modules[0].dynamicCandidates).toEqual(["icon-demo"]);
        const replacement = session.newLoader(options);
        expect(replacement.provide(0, "root.css", rootCss, sourceUri("root.css")).kind).toBe(
          "needFile",
        );
        expect(replacement.provide(1, "plugin", definition, definitionUri).kind).toBe("complete");
        session.reload(replacement);
        const output = session.compile([input]).modules[0];
        expect(output.diagnostics).toEqual([]);
        expect(output.reports).toHaveLength(1);
        expect(output.reports[0]).toMatchObject({
          code: "no-restricted-theme-tokens",
          severity: severity === "warn" ? "warning" : "error",
          primary: {
            location: {
              uri: sourceUri("icon.tsx"),
              startUtf16: source.indexOf("icon-demo"),
              endUtf16: source.indexOf("icon-demo") + "icon-demo".length,
            },
          },
        });
        expect(
          output.reports[0].related.map(
            (label: import("../../binding.js").BindingLabel) => label.location.uri,
          ),
        ).toEqual(definitionUri ? [definitionUri] : []);
      }
    }
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
        noImportant: {
          severity: "warn",
        },
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
}
