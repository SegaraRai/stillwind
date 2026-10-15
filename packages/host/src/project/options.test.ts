import { lintRules, type AnalysisProjectConfig, type LintRule } from "@stillwind/config";
import { expect, test } from "vite-plus/test";

import { compilerOptions } from "./options.ts";

test("analysis projects pass their compiler settings to native loaders", () => {
  const project: AnalysisProjectConfig = {
    css: "root.css",
    files: ["*.tsx"],
    name: "web",
    typeFacts: false,
    utilityOrder: "tailwind",
    target: ["chrome111", "safari17"],
    prefix: "--project-",
    automatic: false,
  };
  expect(compilerOptions(project)).toMatchObject({
    utilityOrder: "tailwind",
    target: ["chrome111", "safari17"],
    prefix: "--project-",
  });
});

test("catalog reporting settings retain each native field name", () => {
  for (const [name, rule] of Object.entries(lintRules)) {
    const setting =
      name === "no-restricted-theme-tokens"
        ? {
            severity: "deny" as const,
            tokens: ["--color-*"],
          }
        : ("deny" as const);
    expect(
      compilerOptions({
        utilityOrder: "tailwind",
        lint: {
          rules: {
            [name as LintRule]: setting,
          },
        },
      }),
    ).toMatchObject({
      utilityOrder: "tailwind",
      lint: {
        [rule.nativeName]: rule.options
          ? {
              severity: "deny",
            }
          : "deny",
      },
    });
  }
});

test("utility policies retain their fields at the native boundary", () => {
  const options = compilerOptions({
    css: "theme.css",
    lint: {
      rules: {
        "prefer-direct-styling": "warn",
        "duplicate-utility": "deny",
        "conflicting-utilities": {
          allowShorthandOverrides: true,
        },
        "prefer-logical-properties": "off",
        "no-restricted-length-units": {
          rules: [
            {
              properties: ["padding*"],
              disallow: ["px"],
            },
          ],
        },
        "no-restricted-theme-tokens": {
          tokens: ["--color-red-*"],
        },
        "no-arbitrary-breakpoints": "deny",
      },
    },
  });
  expect(options.lint).toMatchObject({
    preferDirectStyling: "warn",
    duplicateUtility: "deny",
    conflictingUtilities: {
      allowShorthandOverrides: true,
    },
    preferLogicalProperties: "off",
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
    noArbitraryBreakpoints: "deny",
  });
  expect(
    compilerOptions({
      lint: {
        rules: {
          "conflicting-utilities": "off",
          "no-restricted-length-units": "warn",
        },
      },
    }).lint,
  ).toMatchObject({
    conflictingUtilities: {
      severity: "off",
    },
    noRestrictedLengthUnits: {
      severity: "warn",
    },
  });
});

test("source restriction options retain native names and list replacements", () => {
  expect(
    compilerOptions({
      lint: {
        rules: {
          "no-arbitrary-colors": "deny",
          "no-arbitrary-spacing": "warn",
          "no-class-property": "off",
          "no-important": "deny",
          "invalid-lint-suppression": "warn",
          "unused-lint-suppression": "warn",
          "no-restricted-utilities": {
            utilities: [
              {
                pattern: "legacy*",
                message: "Use semantic utilities",
              },
            ],
          },
          "no-restricted-declarations": {
            rules: [
              {
                properties: ["position"],
                values: ["fixed"],
              },
            ],
          },
        },
      },
    }).lint,
  ).toMatchObject({
    noArbitraryColors: "deny",
    noArbitrarySpacing: {
      severity: "warn",
    },
    noClassProperty: "off",
    noImportant: "deny",
    invalidLintSuppression: "warn",
    unusedLintSuppression: "warn",
    noRestrictedUtilities: {
      utilities: [
        {
          pattern: "legacy*",
          message: "Use semantic utilities",
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
  });
  expect(
    compilerOptions({
      lint: {
        rules: {
          "no-arbitrary-spacing": {
            properties: ["width"],
          },
        },
      },
    }).lint?.noArbitrarySpacing,
  ).toEqual({
    severity: "warn",
    properties: ["width"],
  });
});
