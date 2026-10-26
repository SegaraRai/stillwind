import { expect, test } from "vite-plus/test";

import { sourceUri, cssInput } from "./support.ts";

/**
 * Registers source input tests for the selected native or WASM binding.
 *
 * @param bindingSpecifier - Absolute URL of the binding module.
 */
export function registerSourceContract(bindingSpecifier: string): void {
  test("module summaries preserve UTF-8 module identities across the binding", async () => {
    const { summarizeSource } = await import(bindingSpecifier);
    for (const moduleId of ["short.ts", "a".repeat(16), "a".repeat(17), "日本/😀\0entry.ts"]) {
      const summary = summarizeSource("export const value = 1;", moduleId, "ts", "module");
      expect(summary.moduleId).toBe(moduleId);
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
        (mapping: import("../../binding.js").NativeExpressionMapping) =>
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
          (finding: import("../../binding.js").BindingLintFinding) => finding.rule,
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
            (finding: import("../../binding.js").BindingDiagnostic) => finding.code,
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
              (entry: import("../../binding.js").NativeExport) => entry.name === "marker",
            ).value,
          },
        ],
      },
      producer,
    ]);
    expect(
      withFacts.modules.map(
        (module: import("../../binding.js").ProjectModuleOutput) => module.moduleId,
      ),
    ).toStrictEqual([consumer.moduleId, producer.moduleId]);
    expect(
      withFacts.modules.every(
        (module: import("../../binding.js").ProjectModuleOutput) => module.diagnostics.length === 0,
      ),
      JSON.stringify(withFacts),
    ).toBeTruthy();
    expect(withFacts.modules[0].code).toMatch(/sw_shared_/);
    expect(withFacts.modules[0].code).not.toMatch(/style\(marker\)/);
  });
}
