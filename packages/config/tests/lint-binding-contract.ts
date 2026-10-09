import { pathToFileURL } from "node:url";

import type { NativeCompileSourceInput } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { lintPresets, lintRules, resolveLintConfig, type LintRule } from "../src/index.ts";

/**
 * Registers lint configuration contracts for a process using the built binding.
 * The caller configures binding selection before the tests run.
 * @param target - Expected target exported by @stillwind/native.
 */
export function registerLintBindingContract(target: "native" | "wasm32-wasi") {
  /**
   * Creates a standalone compilation input for comparing resolved policies across binding targets.
   * @param source - TSX source, including invalid syntax when compiler diagnostics are exercised.
   * @param moduleId - Compiler identity also used to derive document and source-map URLs.
   * @param theme - CSS design-system text loaded as the fixture's root stylesheet.
   * @returns Unambiguous TSX input with source and stylesheet metadata expected by compileSource.
   */
  const oneShotInput = (
    source: string,
    moduleId: string,
    theme: string,
  ): NativeCompileSourceInput => ({
    source,
    moduleId,
    uri: pathToFileURL(moduleId).href,
    sourceMapSource: pathToFileURL(moduleId).href,
    language: "tsx",
    moduleKind: "unambiguous",
    css: {
      content: theme,
      name: "root.css",
      uri: pathToFileURL("root.css").href,
      sourceMapSource: pathToFileURL("root.css").href,
    },
  });
  test("resolved presets agree with compiler defaults and disabled lint retains compilation errors", async () => {
    const { compileSource, __napiBindingTarget } = await import("@stillwind/native");
    expect(__napiBindingTarget).toBe(target);
    const input = oneShotInput(
      "import {style} from 'stillwind/react'; export const props=style('missing');",
      "preset.tsx",
      "@stillwind-slot utilities;",
    );
    const defaults = compileSource(input);
    for (const [config, severity] of [
      [undefined, "warning"],
      [{ extends: [lintPresets.strict] }, "error"],
      [{}, undefined],
      [false, undefined],
    ] as const) {
      const { rules } = resolveLintConfig(config);
      const lint = Object.fromEntries(
        Object.entries(rules).map(([name, setting]) => [
          lintRules[name as LintRule].nativeName,
          setting,
        ]),
      );
      const output = compileSource(input, { lint });
      expect(output.code).toBe(defaults.code);
      expect(
        output.reports.map(
          (report: {
            /** Serialized report level compared with the policy's expected severity. */
            severity: string;
          }) => report.severity,
        ),
      ).toEqual(severity === undefined ? [] : [severity]);
      const expected = Object.fromEntries(
        Object.entries(rules).map(([name, setting]) => [
          name,
          typeof setting === "string" ? setting : setting.severity,
        ]),
      );
      expect(output.lintSettings).toEqual(expected);
      if (config === undefined) {
        expect(output.lintSettings).toEqual(defaults.lintSettings);
      }
      if (severity === undefined) {
        const invalid = compileSource(
          oneShotInput("const = ;", "invalid.tsx", "@stillwind-slot utilities;"),
          {
            lint,
          },
        );
        expect(invalid.diagnostics.length).toBeGreaterThan(0);
        expect(invalid.reports.length).toBeGreaterThan(0);
      }
    }
  });
}
