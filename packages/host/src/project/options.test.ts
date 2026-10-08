import { expect, test } from "vite-plus/test";

import { compilerOptions } from "./options.ts";

test("utility policies retain their fields at the native boundary", () => {
  const options = compilerOptions({
    css: "theme.css",
    lint: {
      "prefer-direct-styling": "warn",
      "duplicate-utilities": "deny",
      "conflicting-utilities": {
        allowShorthandOverrides: true,
      },
      "prefer-logical-properties": "off",
      "length-units": {
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
  });
  expect(options.lint).toMatchObject({
    preferDirectStyling: "warn",
    duplicateUtilities: "deny",
    conflictingUtilities: {
      allowShorthandOverrides: true,
    },
    preferLogicalProperties: "off",
    lengthUnits: {
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
        "conflicting-utilities": "off",
        "length-units": "warn",
      },
    }).lint,
  ).toMatchObject({
    conflictingUtilities: {
      severity: "off",
    },
    lengthUnits: {
      severity: "warn",
    },
  });
});

test("source restriction options retain native names and list replacements", () => {
  expect(
    compilerOptions({
      lint: {
        "no-arbitrary-colors": "deny",
        "no-arbitrary-spacing": "warn",
        "no-class-property": "off",
        "no-important": "deny",
        "valid-lint-suppressions": "warn",
        "no-unused-lint-suppressions": "warn",
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
    }).lint,
  ).toMatchObject({
    noArbitraryColors: "deny",
    noArbitrarySpacing: {
      severity: "warn",
    },
    noClassProperty: "off",
    noImportant: "deny",
    validLintSuppressions: "warn",
    noUnusedLintSuppressions: "warn",
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
        "no-arbitrary-spacing": {
          properties: ["width"],
        },
      },
    }).lint?.noArbitrarySpacing,
  ).toEqual({
    properties: ["width"],
  });
});
