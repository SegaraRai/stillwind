import { expect, test } from "vite-plus/test";

/** Registers editor and canonical lint contracts for the selected native or WASM loader. */
export function registerEditorContract(specifier: string): void {
  test("canonical replacements obey token and unit restrictions", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const css =
      "@theme {--color-red-500:red;--spacing:0.25rem;} @utility bg-* {background-color:--value(--color-*,[color]);} @utility p-* {padding:--value(--spacing(),[length]);} @stillwind-slot utilities;";
    const load = (options: import("../binding.js").NativeCompilerOptions) => {
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
          lengthUnits: {
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

  test("ordering batches preserve compiler order, duplicates, unresolved candidates, and theme revisions", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const load = (definitions: string) => {
      const loader = new NativeDesignSystemLoader();
      loader.provide(0, "theme.css", definitions, "file:///theme.css");
      return loader.editorQuery();
    };
    const first = load(
      "@utility a { color:red; } @utility b { color:blue; } @variant hover { &:hover { @slot; } }",
    );
    const values = ["b", "hover:a", "a", "a", "unknown", "unknown:a"];
    const ranks = first.candidateRanks(values);
    expect(ranks).toEqual([1, 2, 0, 0, null, null]);
    expect(first.candidateRanks([])).toEqual([]);
    for (let a = 0; a < values.length; a++) {
      for (let b = 0; b < values.length; b++) {
        const expected = first.compareCandidates(values[a], values[b]);
        expect(ranks[a] == null || ranks[b] == null ? null : Math.sign(ranks[a]! - ranks[b]!)).toBe(
          expected,
        );
      }
    }
    const second = load("@utility b { color:blue; } @utility a { color:red; }");
    expect(second.candidateRanks(["a", "b"])).toEqual([1, 0]);
    expect(first.candidateRanks(["a", "b"])).toEqual([0, 1]);
    const malformed = load(
      "@utility bad-* { @stillwind-require-properties invalid; color:--value([color]); }",
    );
    expect(() => malformed.candidateRanks(["bad-[red]"])).toThrow(/bad-\*/);
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
    expect(query.hoverCandidate("good")).not.toBeNull();
    expect(() => query.canonicalCandidate("bad-[red]")).toThrow(/bad-\*/);
  });

  test("canonical indexes preserve old snapshots and isolate theme reloads", async () => {
    const { NativeDesignSystemLoader } = await import(specifier);
    const loader = new NativeDesignSystemLoader();
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
        canonicalUtilities: {
          rootFontSize: 20.5,
          severity: "deny",
        },
      },
    });
    loader.provide(
      0,
      "theme.css",
      `@theme { --spacing:.25rem; --color-brand:red; --text-body:1rem; --leading-tight:1.25rem; --breakpoint-narrow:40rem; }
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
      rootFontSize: 20.5,
      pxToRem: true,
    });
    expect(query.canonicalCandidate("p-[20.5px]")).toBe("p-4");
    expect(query.hoverCandidate("hover:bg-brand")).toContain("&:hover");
    expect(query.formatCss("&{color:red;&:hover{color:blue}}")).toBe(
      "& {\n  color: red;\n\n  &:hover {\n    color: #00f;\n  }\n}\n",
    );
    expect(() => query.formatCss("&{color red}")).toThrow(/CSS parse failed/);
    expect(query.completeCandidate("bg-br")).toContain("bg-brand");
    expect(query.completeCandidate("bg-(--color-br")).toContain("bg-(--color-brand)");
    expect(query.colorRgba("oklch(60% 0 0)")).toHaveLength(4);
    expect(query.colorRgba("currentColor")).toBeNull();
    expect(query.candidateColor("hover:bg-brand")).toEqual([1, 0, 0, 1]);
    expect(query.candidateColor("bg-[currentColor]")).toBeNull();
    expect(query.completeCandidate("hov")).toContain("hover:");
    expect(query.completeCandidate("group-hov")).toContain("group-hover:");
    expect(query.completeCandidate("!bg-br")).toContain("!bg-brand");
    expect(query.completeCandidate("text-body/ti")).toContain("text-body/tight");
    expect(query.completeCandidate("narrow-na")).toContain("narrow-narrow:");
    expect(query.completeCandidate("bg-[var(--color-br")).toContain("bg-[var(--color-brand)]");
    const code = String.raw`import {style} from 'stillwind/react';
const 日本語='🦀'; const card=style('p-\u005b20.5px\u005d');
// swlint-disable-next-line canonical-utilities
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
      .reports.filter((report: { code: string }) => report.code === "canonical-utilities");
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
        (report: { code: string }) => report.code === "canonical-utilities",
      );
    expect(compiled).toEqual(reports);
  });
}
