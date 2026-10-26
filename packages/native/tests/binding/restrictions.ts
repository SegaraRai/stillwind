import { expect, test } from "vite-plus/test";

import { sourceUri, oneShotInput } from "./support.ts";

/**
 * Registers utility restriction tests for the selected native or WASM binding.
 *
 * @param bindingSpecifier - Absolute URL of the binding module.
 */
export function registerRestrictionsContract(bindingSpecifier: string): void {
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
    const options: import("../../binding.js").NativeCompilerOptions = {
      lint: {
        noArbitraryColors: "deny",
        noArbitrarySpacing: {
          severity: "warn",
        },
        noClassProperty: "warn",
        noImportant: {
          severity: "warn",
        },
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
      { noImportant: { severity: "error" } },
    ]) {
      expect(() => new NativeDesignSystemLoader({ lint })).toThrow();
    }
  });

  test("value-for lists apply value policies across one-shot and session APIs", async () => {
    const { compileSource, NativeDesignSystemLoader, NativeProjectSession } = await import(
      bindingSpecifier
    );
    const css = `@stillwind-property --Size {
  activation: declared; syntax: "*"; inherits: false; value-for: width, height, width;
}
@utility box-* { --Size:--value([length]); }
@stillwind-slot properties;
*, ::before, ::after { @stillwind-slot property-initials; }
@stillwind-slot utilities;`;
    const source = `import {style} from 'stillwind/react'; export const box=style('box-[20px]');`;
    const options: import("../../binding.js").NativeCompilerOptions = {
      lint: {
        preferCanonicalUtilities: {
          severity: "off",
        },
        preferLogicalProperties: {
          severity: "warn",
        },
        noArbitrarySpacing: {
          properties: ["height"],
        },
        noRestrictedLengthUnits: {
          rules: [
            {
              properties: ["width", "height"],
              disallow: ["px"],
            },
          ],
        },
      },
    };
    const expected = [
      "no-arbitrary-spacing",
      "no-restricted-length-units",
      "no-restricted-length-units",
    ];
    const output = compileSource(oneShotInput(source, "value-for.tsx", css), options);
    expect(output.diagnostics).toEqual([]);
    expect(output.reports.map((report: { code: string }) => report.code).sort()).toEqual(expected);
    expect(output.css).toContain("@property --Size");
    expect(output.css).not.toContain("value-for");
    const loader = new NativeDesignSystemLoader(options);
    loader.provide(0, "root.css", css, sourceUri("root.css"));
    const session = new NativeProjectSession(loader, false);
    expect(
      session
        .diagnose([
          {
            kind: "direct",
            moduleId: "value-for.tsx",
            document: { code: source, uri: sourceUri("value-for.tsx"), format: "jsx" },
            syntax: { language: "tsx", moduleKind: "unambiguous" },
          },
        ])[0]
        .reports.map((report: { code: string }) => report.code)
        .sort(),
    ).toEqual(expected);
  });

  test("value policies inspect mask and filter colors through aliases", async () => {
    const { compileSource, NativeDesignSystemLoader, NativeProjectSession } = await import(
      bindingSpecifier
    );
    const css = `@stillwind-property --paint {
  syntax: "*"; inherits: false; value-for: mask-image, filter, backdrop-filter;
}
@stillwind-slot keyframes;
@stillwind-slot utilities;`;
    const options: import("../../binding.js").NativeCompilerOptions = {
      lint: {
        preferCanonicalUtilities: {
          severity: "off",
        },
        noArbitraryColors: "warn",
      },
    };
    const loader = new NativeDesignSystemLoader(options);
    loader.provide(0, "root.css", css, sourceUri("root.css"));
    const session = new NativeProjectSession(loader, false);
    for (const [value, expected] of [
      ["red", 3],
      ["var(--runtime)", 0],
    ] as const) {
      for (const expression of [
        `style(${JSON.stringify(`[--paint:${value}]`)})`,
        `style.keyframes({from:${JSON.stringify(`[--paint:${value}]`)}})`,
      ]) {
        const source = `import {style} from 'stillwind/react'; export const paint=${expression};`;
        const reports = Array.from<string>({ length: expected }).fill("no-arbitrary-colors");
        const output = compileSource(oneShotInput(source, "paint.tsx", css), options);
        expect(output.diagnostics).toEqual([]);
        expect(output.reports.map((report: { code: string }) => report.code)).toEqual(reports);
        expect(
          session
            .diagnose([
              {
                kind: "direct",
                moduleId: "paint.tsx",
                document: {
                  code: source,
                  uri: sourceUri("paint.tsx"),
                  format: "jsx",
                },
                syntax: {
                  language: "tsx",
                  moduleKind: "unambiguous",
                },
              },
            ])[0]
            .reports.map((report: { code: string }) => report.code),
        ).toEqual(reports);
      }
    }
  });

  test("implementation checks preserve captured values across one-shot and session APIs", async () => {
    const { compileSource, NativeDesignSystemLoader, NativeProjectSession } = await import(
      bindingSpecifier
    );
    const css = `@utility fixed (implementation) { width:max-content; height:1em; }
@utility input-* (implementation) { width:--value([length]); height:1em; }
@utility extended (implementation) { width:20px; }
@utility extended { width:20px; }
@stillwind-slot utilities;`;
    const source = `import {style} from 'stillwind/react';
export const fixed=style('fixed!'); export const input=style('input-[20px]');
export const extended=style('extended');`;
    for (const checkImplementation of [undefined, false, true]) {
      const options: import("../../binding.js").NativeCompilerOptions = {
        lint: {
          preferCanonicalUtilities: {
            severity: "off",
          },
          preferLogicalProperties: {
            severity: "warn",
            checkImplementation,
          },
          noRestrictedLengthUnits: {
            severity: "warn",
            checkImplementation,
            rules: [
              {
                properties: ["*"],
                disallow: ["px", "em"],
              },
            ],
          },
          noImportant: {
            severity: "warn",
          },
        },
      };
      const expected = [
        "no-important",
        ...Array.from<string>({ length: checkImplementation ? 4 : 2 }).fill(
          "no-restricted-length-units",
        ),
        ...Array.from<string>({ length: checkImplementation ? 5 : 2 }).fill(
          "prefer-logical-properties",
        ),
      ].sort();
      const output = compileSource(oneShotInput(source, "implementation.tsx", css), options);
      expect(output.diagnostics).toEqual([]);
      expect(output.reports.map((report: { code: string }) => report.code).sort()).toEqual(
        expected,
      );
      const loader = new NativeDesignSystemLoader(options);
      loader.provide(0, "root.css", css, sourceUri("root.css"));
      const session = new NativeProjectSession(loader, false);
      const prepared = {
        kind: "direct",
        moduleId: "implementation.tsx",
        document: { code: source, uri: sourceUri("implementation.tsx"), format: "jsx" },
        syntax: { language: "tsx", moduleKind: "unambiguous" },
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
    }
  });

  test("implementation restrictions preserve source selections across bindings and session APIs", async () => {
    const { compileSource, NativeDesignSystemLoader, NativeProjectSession } = await import(
      bindingSpecifier
    );
    const css = `@theme inline { --spacing-ref-small:20px; }
@utility fixed (implementation) { width:var(--spacing-ref-small)!important; }
@utility input-* (implementation) { width:--value(--spacing-*)!important; }
@utility extended (implementation) { width:var(--spacing-ref-small)!important; }
@utility extended { width:var(--spacing-ref-small)!important; }
@stillwind-slot utilities;`;
    const source = `import {style} from 'stillwind/react';
export const fixed=style('fixed'); export const important=style('fixed!');
export const input=style('input-ref-small'); export const extended=style('extended');`;
    for (const checkImplementation of [undefined, false, true]) {
      const options: import("../../binding.js").NativeCompilerOptions = {
        lint: {
          preferCanonicalUtilities: {
            severity: "off",
          },
          noImportant: {
            severity: "warn",
            checkImplementation,
          },
          noRestrictedDeclarations: {
            severity: "warn",
            checkImplementation,
            rules: [
              {
                properties: ["width"],
                values: ["*"],
              },
            ],
          },
          noRestrictedThemeTokens: {
            severity: "warn",
            checkImplementation,
            tokens: ["--spacing-ref-*"],
          },
        },
      };
      const expected = [
        ...Array.from<string>({ length: checkImplementation === false ? 3 : 4 }).fill(
          "no-important",
        ),
        ...Array.from<string>({ length: checkImplementation === false ? 2 : 4 }).fill(
          "no-restricted-declarations",
        ),
        ...Array.from<string>({ length: checkImplementation === false ? 2 : 4 }).fill(
          "no-restricted-theme-tokens",
        ),
      ].sort();
      const output = compileSource(
        oneShotInput(source, "implementation-restrictions.tsx", css),
        options,
      );
      expect(output.diagnostics).toEqual([]);
      expect(output.reports.map((report: { code: string }) => report.code).sort()).toEqual(
        expected,
      );
      const loader = new NativeDesignSystemLoader(options);
      loader.provide(0, "root.css", css, sourceUri("root.css"));
      const session = new NativeProjectSession(loader, false);
      const prepared = {
        kind: "direct",
        moduleId: "implementation-restrictions.tsx",
        document: {
          code: source,
          uri: sourceUri("implementation-restrictions.tsx"),
          format: "jsx",
        },
        syntax: { language: "tsx", moduleKind: "unambiguous" },
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
    }
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
    const options: import("../../binding.js").NativeCompilerOptions = {
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
    const afterJsxText = compileSource(
      oneShotInput(
        `import {style} from 'stillwind/react';
const note=<p>Don't</p>;
// swlint-disable-next-line no-arbitrary-breakpoints
const x=style('max-[900px]:flex');`,
        "suppressed-jsx.tsx",
        css,
      ),
      options,
    );
    expect(afterJsxText.reports).toEqual([]);
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
      "invalid-variant",
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
}
