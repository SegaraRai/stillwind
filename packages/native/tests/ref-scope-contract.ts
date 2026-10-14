import type { NativeCompilerOptions } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

const declarations = `import {style} from 'stillwind/react';
const menu = style.ref('menu');
const outer = style.ref('outer');
const trigger = style.ref('trigger');`;
const theme = `@variant hover { @media (hover:hover) { &:hover { @slot; } } }
@variant focus { &:focus { @slot; } }
@variant active { &:active { @slot; } }
@utility accent { color:red; }
@stillwind-slot utilities;`;

/** Registers scope contracts against the supplied native or WASM binding entry. */
export function registerRefScopeContract(bindingSpecifier: string) {
  const compile = async (body: string, css = theme, options: NativeCompilerOptions = {}) => {
    const { compileSource } = await import(bindingSpecifier);
    return compileSource(
      {
        source: `${declarations}\n${body}`,
        moduleId: "scopes.tsx",
        uri: "untitled:scopes.tsx",
        sourceMapSource: "untitled:scopes.tsx",
        language: "tsx",
        moduleKind: "unambiguous",
        css: {
          content: css,
          name: "root.css",
          uri: "untitled:root.css",
          sourceMapSource: "untitled:root.css",
        },
      },
      options,
    );
  };

  test("scope calls and tags preserve ref and scope states", async () => {
    const outputs = [];
    for (const expression of [
      '{ [trigger("hover").in(menu("focus"))]: { "active:": "accent" } }',
      '{ [trigger`hover:`.in(menu`focus:`)]: { "active:": "accent" } }',
      '`${trigger("hover").in(menu("focus"))}:active:accent`',
    ]) {
      const output = await compile(`export const props = style(${expression});`);
      expect(output.diagnostics).toStrictEqual([]);
      expect(output.css).toMatch(
        /:is\(:where\(\.sw_menu_\w+\):focus\):has\(:is\(:where\(\.sw_trigger_\w+\):hover\)\)/,
      );
      expect(output.css).toMatch(/\.sw_\w+:active:is\([^{}]*\)\{color:red;/);
      outputs.push(output);
    }
    expect(outputs[0].css).toBe(outputs[1].css);
    expect(outputs[0].css.replace(/\.sw_\w+(?=:active)/g, ".target")).toBe(
      outputs[2].css.replace(/\.sw_\w+(?=:active)/g, ".target"),
    );
  });

  test("repeated scopes each test the original ref predicate in either order", async () => {
    for (const scopes of [".in(menu).in(outer)", ".in(outer).in(menu)"]) {
      const output = await compile(
        `export const props = style({[trigger('hover')${scopes}]: 'accent'});`,
      );
      expect(output.diagnostics).toStrictEqual([]);
      const selectors = [...output.css.matchAll(/(\.sw_\w+[^{}]*)\{color:red;/g)].map(
        (match) => match[1],
      );
      expect(selectors, output.css).toHaveLength(1);
      for (const scope of ["menu", "outer"]) {
        expect(selectors[0]).toMatch(
          new RegExp(
            `:is\\(:where\\(\\.sw_${scope}_\\w+\\)\\):has\\(:is\\(:where\\(\\.sw_trigger_\\w+\\):hover\\)\\)`,
          ),
        );
      }
    }
  });

  test("container queries combine with scoped selectors through style keys", async () => {
    const output = await compile(`
export const container = style(menu, menu.asContainer('inline-size'));
export const props = style({
  [menu.at('width > 40rem')]: {[trigger('hover').in(menu)]: 'accent'},
});`);
    expect(output.diagnostics).toStrictEqual([]);
    expect(output.css).toMatch(/container-type:inline-size/);
    expect(output.css).toMatch(/@container sw-c-menu-[\w-]+\s*\(width > 40rem\)/);
    expect(output.css).toMatch(
      /:is\(:where\(\.sw_menu_\w+\)\):has\(:is\(:where\(\.sw_trigger_\w+\):hover\)\)/,
    );
  });

  test("provider attachment findings honor configured reporting levels", async () => {
    const rule = "require-ref-attachment";
    for (const severity of ["warn", "deny", "off"]) {
      const output = await compile("export const props = style(menu.asAnchor());", theme, {
        lint: {
          requireRefAttachment: severity,
        },
      });
      expect(output.diagnostics).toStrictEqual([]);
      expect(output.lintSettings[rule]).toBe(severity);
      const findings = output.lintFindings.filter(
        (finding: { rule: string }) => finding.rule === rule,
      );
      expect(findings).toHaveLength(severity === "off" ? 0 : 1);
      if (severity !== "off") {
        expect(
          output.reports.some(
            (report: { severity: string }) =>
              report.severity === (severity === "deny" ? "error" : "warning"),
          ),
        ).toBe(true);
      }
    }
    const valid = await compile("export const props = style(menu, menu.asAnchor());");
    expect(valid.diagnostics).toStrictEqual([]);
    expect(valid.lintFindings).toStrictEqual([]);
  });

  test("invalid scope operands produce lints while stored variants remain fatal", async () => {
    for (const body of [
      "export const props = style({[trigger().in()]: 'accent'});",
      "export const props = style({[trigger().in(menu, outer)]: 'accent'});",
      "export const props = style({[trigger().in(menu().in(outer))]: 'accent'});",
      "export const props = style({[trigger().in(menu.at())]: 'accent'});",
      "export const props = style({[trigger().in(menu.at`width > 1px`)]: 'accent'});",
      "export const props = style({[trigger('hover').in(menu).at('width > 1px')]: 'accent'});",
    ]) {
      const output = await compile(body);
      expect(output.diagnostics, body).toStrictEqual([]);
      expect(
        output.lintFindings.some(
          (finding: { rule: string }) => finding.rule === "invalid-ref-input",
        ),
        body,
      ).toBe(true);
      expect(output.css, body).not.toContain("color:");
    }
    const stored = await compile(
      "const variant = trigger('hover').in(menu); export const props = style({[variant]: 'accent'});",
    );
    expect(stored.diagnostics.length).toBeGreaterThan(0);
  });
}
