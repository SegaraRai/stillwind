import { expect, test } from "vite-plus/test";

import { completionLabels } from "./support.ts";

/**
 * Registers canonical spelling tests for the selected native or WASM binding.
 *
 * @param specifier - Absolute URL of the binding module.
 */
export function registerCanonicalEditorContract(specifier: string): void {
  test("canonical dynamic suggestions require resolved names", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const css = `@stillwind-dynamic icon-*; @stillwind-dynamic icon-[*];
@utility icon-* { --size: --value([length]); }
@variant hover { &:hover { @slot; } }`;
    for (const defined of [false, true]) {
      const loader = new NativeDesignSystemLoader();
      loader.provide(
        0,
        "theme.css",
        css + (defined ? "@utility icon-demo { display: inline-block; }" : ""),
      );
      const query = loader.editorQuery();
      expect(query.canonicalCandidate("icon-[demo]")).toBe(defined ? "icon-demo" : null);
      expect(query.canonicalCandidate("hover:!icon-[demo]")).toBe(
        defined ? "hover:icon-demo!" : null,
      );
      for (const raw of ["icon-[red]", "icon-[missing]", "icon-[2rem]", "icon-[demo]/50"]) {
        expect(query.canonicalCandidate(raw)).toBeNull();
      }
    }
  });

  test("CSS declarations define scale functions, fixed keys, completions, and canonical values", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `@stillwind-theme-namespace --unit-* [length] scale(hair: 1px);
@theme { --unit: .5rem; --unit-wide: 2rem; }
@utility space-* { padding: --value(--unit(), [length]); }
@utility inset-* { margin: --value(--unit-*); }
@utility fixed { width: --unit(2); }`,
    );
    const query = loader.editorQuery();
    expect(query.previewCandidate("fixed")).toBe("&{width: calc(var(--unit) * 2);}");
    expect(query.previewCandidate("space-hair")).toBe("&{padding: 1px;}");
    expect(query.previewCandidate("inset-hair")).toBe("&{margin: 1px;}");
    expect(completionLabels(query, "space-")).toEqual(["space-hair", "space-wide"]);
    expect(completionLabels(query, "inset-h")).toEqual(["inset-hair"]);
    expect(query.canonicalCandidate("space-[16px]")).toBe("space-2");
    expect(query.canonicalCandidate("space-[1px]")).toBe("space-hair");
  });

  test("canonical replacements obey token and unit restrictions", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const css =
      "@stillwind-theme-namespace --spacing-* [length] scale; @theme {--color-red-500:red;--spacing:0.25rem;} @utility bg-* {background-color:--value(--color-*,[color]);} @utility p-* {padding:--value(--spacing(),[length]);} @stillwind-slot utilities;";
    /**
     * Loads the shared stylesheet with compiler options.
     *
     * @param options - Options under test.
     * @returns Editor queries over the completed design system.
     */
    const load = (options: import("../../binding.js").NativeCompilerOptions) => {
      const loader = new NativeDesignSystemLoader(options);
      loader.provide(0, "theme.css", css, "file:///theme.css");
      return loader.editorQuery();
    };
    expect(load({}).canonicalCandidate("bg-[rgb(255_0_0)]")).toBe("bg-red-500");
    expect(
      load({
        lint: {
          noRestrictedThemeTokens: {
            tokens: ["--color-red-*"],
          },
        },
      }).canonicalCandidate("bg-[rgb(255_0_0)]"),
    ).toBeNull();
    expect(load({}).canonicalCandidate("p-[16px]")).toBe("p-4");
    expect(
      load({
        lint: {
          noRestrictedThemeTokens: {
            tokens: ["--spacing"],
          },
        },
      }).canonicalCandidate("p-[16px]"),
    ).toBeNull();
    expect(
      load({
        lint: {
          noRestrictedLengthUnits: {
            rules: [
              {
                properties: ["padding*"],
                disallow: ["rem"],
              },
            ],
          },
        },
      }).canonicalCandidate("p-[16px]"),
    ).toBeNull();
  });

  test("a loader that defers its canonical index answers canonical queries the same way", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.deferCanonicalIndex();
    expect(() => loader.prepareCanonicalIndex()).toThrow();
    loader.provide(
      0,
      "theme.css",
      "@stillwind-theme-namespace --spacing-* [length] scale; @theme {--spacing:0.25rem;} @utility p-* {padding:--value(--spacing(),[length]);} @stillwind-slot utilities;",
      "file:///theme.css",
    );
    expect(loader.editorQuery().canonicalCandidate("p-[16px]")).toBe("p-4");
    loader.prepareCanonicalIndex();
    expect(loader.editorQuery().canonicalCandidate("p-[16px]")).toBe("p-4");
  });

  test("canonical queries preserve aliases, required registrations, and modifier branches", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(
      0,
      "theme.css",
      `@theme { --*:initial; --spacing-gap:1rem; --color-a:red; --color-b:blue; --color-special-a:blue; --color-alias:var(--color-a); --text-body:1rem; --leading-tight:1.25rem; }
@property --flag { syntax:'*'; inherits:false; initial-value:0; }
@utility tone-* { @stillwind-require-properties --flag; &:hover { color:--value(--color-*, [color]); outline-color:var(--color-alias); content:'var(--color-a)'; } }
@utility text-* { font-size:--value(--text-*, [length]); line-height:--modifier(--leading-*, [length]); }
@utility -m-* { margin:calc(--value(--spacing-*, [length]) * -1); }
@utility red { @stillwind-require-properties --flag; color:red; }
@utility overlap-* { color:--value(--color-*, [color]); opacity:--modifier(number); }
@utility overlap-special-* { color:--value(--color-*, [color]); opacity:--modifier(number); }
@utility overlap-a { color:blue; opacity:--modifier(number); }
@utility overlap-alias { color:blue; opacity:--modifier(number); }
@stillwind-slot utilities;`,
      "file:///theme.css",
    );
    const query = loader.editorQuery();
    expect(query.canonicalCandidate("tone-[#f00]")).toBe("tone-a");
    expect(query.canonicalCandidate("tone-[#00f]")).toBe("tone-b");
    expect(query.canonicalCandidate("text-[1rem]/tight")).toBe("text-body/tight");
    expect(query.canonicalCandidate("[margin:-1rem]")).toBe("-m-gap");
    expect(query.canonicalCandidate("[color:red]")).toBeNull();
    expect(query.canonicalCandidate("overlap-[rgb(255_0_0)]/50")).toBe("overlap-special-a/50");
  });

  test("CSS loading prepares canonical indexes without reporting unused requirement errors", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    const loaded = loader.provide(
      0,
      "theme.css",
      "@utility good { color:red; } @utility bad-* { @stillwind-require-properties invalid; color:--value([color]); }",
      "file:///theme.css",
    );
    expect(loaded.kind).toBe("complete");
    const query = loader.editorQuery();
    expect(query.previewCandidate("good")).not.toBeNull();
    expect(() => query.canonicalCandidate("bad-[red]")).toThrow(/bad-\*/);
  });

  test("canonical indexes preserve old snapshots and isolate theme reloads", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    /**
     * Builds a theme whose brand color differs between revisions.
     * @param value - Brand color.
     * @returns Stylesheet text.
     */
    const css = (value: string) =>
      `@theme { --color-*:initial; --color-brand:${value}; }
@utility bg-* { background-color:--value(--color-*, [color]); }
@variant over { &:hover { @slot; } }
@stillwind-slot utilities;`;
    loader.provide(0, "theme.css", css("red"), "file:///theme.css");
    const original = loader.editorQuery();
    expect(original.canonicalCandidate("bg-[#f00]")).toBe("bg-brand");
    expect(original.canonicalCandidate("[&:hover]:bg-brand")).toBe("over:bg-brand");
    expect(loader.editorQuery().canonicalCandidate("bg-[#f00]")).toBe("bg-brand");
    const reloaded = new NativeDesignSystemLoader();
    reloaded.provide(0, "theme.css", css("blue"), "file:///theme.css");
    const updated = reloaded.editorQuery();
    expect(updated.canonicalCandidate("bg-[#f00]")).toBeNull();
    expect(updated.canonicalCandidate("bg-[#00f]")).toBe("bg-brand");
    expect(original.canonicalCandidate("bg-[#f00]")).toBe("bg-brand");
    expect(original.canonicalCandidate("bg-[#00f]")).toBeNull();
  });

  test("editor queries and compiler diagnostics share canonical options and source suppression", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession, fingerprint } = await import(specifier);
    const loader = new NativeDesignSystemLoader({
      lint: {
        preferCanonicalUtilities: {
          severity: "deny",
        },
      },
      rootFontSize: 20.5,
    });
    loader.provide(
      0,
      "theme.css",
      `@stillwind-theme-namespace --spacing-* [length] scale;
@theme { --spacing:.25rem; --color-brand:red; --text-body:1rem; --leading-tight:1.25rem; --breakpoint-narrow:40rem; }
@utility p-* { padding:--value(--spacing-*, --spacing(), [length]); }
@utility bg-* { background-color:--value(--color-*, [color]); }
@utility text-* { font-size:--value(--text-*, [length]); line-height:--modifier(--leading-*, [length]); }
@variant hover { &:hover { @slot; } }
@stillwind-variant group-* (value: [variant]) { &:is(:where(.group):--value() *) { @slot; } }
@variant narrow-* { @media (width < --value(--breakpoint-*, [length])) { @slot; } }
@stillwind-slot utilities;`,
      "file:///theme.css",
    );
    const query = loader.editorQuery();
    expect(query.canonicalOptions()).toMatchObject({
      severity: "deny",
      pxToRem: true,
    });
    expect(query.canonicalCandidate("p-[20.5px]")).toBe("p-4");
    expect(query.previewCandidate("hover:bg-brand")).toContain("&:hover");
    expect(query.formatCss("&{color:red;&:hover{color:blue}}")).toBe(
      "& {\n  color: red;\n\n  &:hover {\n    color: #00f;\n  }\n}\n",
    );
    expect(() => query.formatCss("&{color red}")).toThrow(/CSS parse failed/);
    expect(
      query.formatCss("&{padding:calc(var(--spacing) * 4);color:var(--color-brand)}", {}),
    ).toBe(
      "& {\n  padding: calc(var(--spacing) * 4) /* 1rem = 20.5px */;\n  color: var(--color-brand) /* red = #ff0000 */;\n}\n",
    );
    expect(
      query.formatCss("&{padding:calc(var(--spacing) * 4)}", {
        themeValues: false,
        colorEquivalents: false,
      }),
    ).toBe("& {\n  padding: calc(var(--spacing) * 4);\n}\n");
    expect(() =>
      query.formatCss("&{color:red}", {
        colorEquivalents: true as unknown as false,
      }),
    ).toThrow("colorEquivalents");
    expect(completionLabels(query, "bg-br")).toContain("bg-brand");
    expect(completionLabels(query, "bg-(--color-br")).toContain("bg-(--color-brand)");
    expect(query.colorRgba("oklch(60% 0 0)")).toHaveLength(4);
    expect(query.colorRgba("currentColor")).toBeNull();
    expect(query.candidateColor("hover:bg-brand")).toEqual([1, 0, 0, 1]);
    expect(query.candidateColor("bg-[currentColor]")).toBeNull();
    expect(completionLabels(query, "hov")).toContain("hover:");
    expect(completionLabels(query, "group-hov")).toContain("group-hover:");
    expect(completionLabels(query, "!bg-br")).toContain("!bg-brand");
    expect(completionLabels(query, "text-body/ti")).toContain("text-body/tight");
    expect(completionLabels(query, "narrow-na")).toContain("narrow-narrow:");
    expect(completionLabels(query, "bg-[var(--color-br")).toContain("bg-[var(--color-brand)]");
    const code = String.raw`import {style} from 'stillwind/react';
const 日本語='🦀'; const card=style('p-\u005b20.5px\u005d');
// swlint-disable-next-line prefer-canonical-utilities
const ignored=style('p-[20.5px]');`;
    const session = new NativeProjectSession(loader, false);
    const input = {
      kind: "direct",
      moduleId: "entry.tsx",
      document: {
        uri: "file:///entry.tsx",
        code,
        format: "jsx",
      },
      syntax: {
        language: "tsx",
        moduleKind: "module",
      },
      typeFacts: [],
    };
    const reports = session
      .diagnose([input])[0]
      .reports.filter((report: { code: string }) => report.code === "prefer-canonical-utilities");
    expect(reports).toHaveLength(1);
    expect(reports[0]).toMatchObject({
      severity: "error",
      primary: {
        location: {
          revision: fingerprint(code),
          startUtf16: code.indexOf(String.raw`p-\u005b`),
        },
      },
    });
    expect(reports[0].actions[0].edits[0].replacement).toBe("p-4");
    const compiled = session
      .compile([input])
      .modules[0].reports.filter(
        (report: { code: string }) => report.code === "prefer-canonical-utilities",
      );
    expect(compiled).toEqual(reports);
  });
}

const definitions = `
@stillwind-theme-namespace --spacing-* [length] scale;
@theme { --spacing:.25rem; }
@utility w-* { width:--value(--spacing(),[length]); }
@utility h-* { height:--value(--spacing(),[length]); }
@utility size-* { width:--value(--spacing(),[length]); height:--value(--spacing(),[length]); }
@utility flex { display:flex; }
@variant focus { &:focus { @slot; } }
@variant hover { &:hover { @slot; } }
@stillwind-variant in-* (value:[variant]) { :where(:--value()) & { @slot; } }
@stillwind-slot utilities;`;

/**
 * Registers canonical queries and source fixes for a native or WASM binding.
 * @param specifier - Module specifier that resolves to the binding under test.
 */
export function registerCanonicalContract(specifier: string): void {
  test("canonical sets keep conditions, importance, and unrelated source order", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(0, "theme.css", definitions);
    const query = loader.editorQuery();
    expect(query.canonicalCandidate("!w-4")).toBe("w-4!");
    expect(query.canonicalCandidate("!unknown")).toBeNull();
    expect(query.canonicalCandidate("unknown:!w-4")).toBeNull();
    expect(query.canonicalCandidates(["w-4", "h-4"])).toEqual(["size-4"]);
    expect(query.canonicalCandidates(["hover:w-4", "focus:flex", "hover:h-4"])).toEqual([
      "focus:flex",
      "hover:size-4",
    ]);
    expect(query.canonicalCandidates(["w-4!", "h-4"])).toEqual(["w-4!", "h-4"]);
    expect(query.canonicalCandidates(["w-4", "hover:h-4"])).toEqual(["w-4", "hover:h-4"]);
    expect(query.canonicalCandidates(["w-4", "h-5"])).toEqual(["w-4", "h-5"]);
    expect(query.canonicalCandidates(["w-5", "w-4"])).toEqual(["w-5"]);
    expect(query.canonicalCandidates(["w-4", "unknown", "h-5"])).toEqual(["w-4", "unknown", "h-5"]);
    const separate = new NativeDesignSystemLoader({
      lint: { preferCanonicalUtilities: { collapse: false } },
    });
    separate.provide(0, "theme.css", definitions);
    expect(separate.editorQuery().canonicalCandidates(["w-[1rem]", "h-4"])).toEqual(["w-4", "h-4"]);
  });

  test("canonical source fixes preserve escaped whitespace and report the replacement", async () => {
    const { NativeDesignSystemLoader, NativeProjectSession } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
    loader.provide(0, "theme.css", definitions);
    const session = new NativeProjectSession(loader, false);
    const code =
      'import {style} from "stillwind/react"; const emoji="😀"; export const x=style("w-4\\th-4");';
    const output = session.compile([
      {
        kind: "direct",
        moduleId: "entry.tsx",
        document: { uri: "file:///entry.tsx", code, format: "jsx" },
        syntax: { language: "tsx", moduleKind: "module" },
        typeFacts: [],
      },
    ]).modules[0];
    const reports = output.reports.filter(
      (report: import("../../binding.js").BindingReport) =>
        report.code === "prefer-canonical-utilities",
    );
    expect(reports).toHaveLength(1);
    expect(reports[0].message).toContain("size-4");
    const edit = reports[0].actions[0].edits[0];
    expect(code.slice(edit.location.startUtf16, edit.location.endUtf16)).toBe("w-4\\th-4");
    expect(edit.replacement).toBe("size-4");
  });

  test("canonical CSS fixes stay within one apply statement and keep comments", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const css = `${definitions}\n.a { @apply w-4 h-4; } .b { @apply w-4 /* keep */ h-4; } .c { @apply w-4; @apply h-4; }`;
    const loader = new NativeDesignSystemLoader();
    loader.provide(0, "theme.css", css, "file:///theme.css", undefined, true);
    const reports = loader
      .stylesheetReports()
      .filter(
        (report: import("../../binding.js").BindingReport) =>
          report.code === "prefer-canonical-utilities",
      );
    expect(reports).toHaveLength(1);
    const edit = reports[0].actions[0].edits[0];
    expect(css.slice(edit.location.startUtf16, edit.location.endUtf16)).toBe("w-4 h-4");
    expect(edit.replacement).toBe("size-4");
  });
}
