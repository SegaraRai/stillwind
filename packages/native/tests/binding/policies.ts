import { expect, test } from "vite-plus/test";

import { sourceUri, oneShotInput } from "./support.ts";

/**
 * Registers utility policy tests for the selected native or WASM binding.
 *
 * @param bindingSpecifier - Absolute URL of the binding module.
 */
export function registerPoliciesContract(bindingSpecifier: string): void {
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

  test("important modifiers report once per occurrence and preserve suppression", async () => {
    const { compileSource } = await import(bindingSpecifier);
    const css = `@utility multi { display:inline-block; width:1em; height:1em; vertical-align:middle; color:red; background:blue; padding:0; margin:0; border:0; &:hover { color:blue; } }
@utility multi { opacity:1; }
@utility authored { color:red!important; display:block!important; &:hover { color:blue!important; } }
@stillwind-slot utilities;`;
    const source = `import {style} from "stillwind/react";
const label = "😀";
export const icons = style("!multi multi!");
// swlint-disable-next-line no-important
export const suppressed = style("!multi");
export const authored = style("authored");`;
    for (const severity of ["warn", "deny"] as const) {
      const output = compileSource(oneShotInput(source, "important.tsx", css), {
        lint: {
          noImportant: {
            severity,
          },
          unusedLintSuppression: "warn",
        },
      });
      expect(output.diagnostics).toEqual([]);
      expect(output.reports.map((report: { code: string }) => report.code)).toEqual([
        "no-important",
        "no-important",
        "no-important",
        "no-important",
      ]);
      expect(
        output.reports.map((report: import("../../binding.js").BindingReport) => ({
          text: source.slice(report.primary.location.startUtf16, report.primary.location.endUtf16),
          severity: report.severity,
        })),
      ).toEqual([
        { text: "!multi", severity: severity === "warn" ? "warning" : "error" },
        { text: "multi!", severity: severity === "warn" ? "warning" : "error" },
        { text: "authored", severity: severity === "warn" ? "warning" : "error" },
        { text: "authored", severity: severity === "warn" ? "warning" : "error" },
      ]);
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
    const options: import("../../binding.js").NativeCompilerOptions = {
      lint: {
        preferDirectStyling: "warn",
        duplicateUtility: "deny",
        preferLogicalProperties: {
          severity: "warn",
        },
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
      "prefer-canonical-utilities",
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
}
