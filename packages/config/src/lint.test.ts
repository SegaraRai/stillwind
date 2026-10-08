import { expect, test } from "vite-plus/test";

import { normalizeCompilerConfig } from "./compiler.ts";
import { validateConfig } from "./configuration.ts";
import { lintPresets } from "./lint-presets.ts";
import { lintRules } from "./lint-rules.ts";
import {
  mergeLintConfig,
  resolveLintConfig,
  validateLintConfig,
  type LintPreset,
  type LintRules,
} from "./lint.ts";

test("omission selects recommended and explicit configurations enable only their own rules", () => {
  const recommended = resolveLintConfig({
    extends: [lintPresets.recommended],
  });
  expect(resolveLintConfig()).toEqual(recommended);
  expect(recommended.rules["unknown-utility"]).toBe("warn");
  expect(recommended.rules["macro-argument-mutation"]).toBe("deny");
  for (const setting of Object.values(resolveLintConfig({}).rules)) {
    expect(typeof setting === "string" ? setting : setting.severity).toBe("off");
  }
  const explicit = resolveLintConfig({
    rules: {
      "sort-utilities": "warn",
      "canonical-utilities": {
        rootFontSize: 20,
      },
    },
  });
  expect(explicit.rules["sort-utilities"]).toBe("warn");
  expect(explicit.rules["canonical-utilities"]).toEqual({
    severity: "warn",
    pxToRem: true,
    rootFontSize: 20,
  });
  expect(explicit.rules["unknown-utility"]).toBe("off");
  expect(
    normalizeCompilerConfig(
      normalizeCompilerConfig({
        lint: {},
      }),
    ).lint,
  ).toEqual(resolveLintConfig({}));
});

test("strict denies recommended checks and stylistic adds independent conventions", () => {
  const recommended = resolveLintConfig().rules;
  const strict = resolveLintConfig({
    extends: [lintPresets.strict],
  }).rules;
  for (const [rule, setting] of Object.entries(recommended)) {
    const level = typeof setting === "string" ? setting : setting.severity;
    if (level !== "off") {
      const actual = strict[rule as keyof typeof strict];
      expect(typeof actual === "string" ? actual : actual.severity).toBe("deny");
    }
  }
  expect(strict["no-unused-lint-suppressions"]).toBe("deny");
  expect(strict["no-arbitrary-colors"]).toBe("off");
  const stylistic = resolveLintConfig({
    extends: [lintPresets.stylistic],
  }).rules;
  expect(stylistic["sort-utilities"]).toBe("warn");
  expect(stylistic["unknown-utility"]).toBe("off");
});

test("nested presets apply in order while severity changes preserve options and lists replace", () => {
  const policy = {
    name: "company/policy",
    rules: {
      "canonical-utilities": {
        severity: "warn",
        pxToRem: false,
        rootFontSize: 20,
      },
      "no-restricted-theme-tokens": {
        severity: "warn",
        tokens: ["--color-red-*"],
      },
    },
  } as const satisfies LintPreset;
  const strict: LintPreset = {
    extends: [policy],
    rules: { "canonical-utilities": "deny" },
  };
  const config: LintPreset = {
    extends: [policy, strict],
    rules: {
      "canonical-utilities": {
        pxToRem: undefined,
      },
      "no-restricted-theme-tokens": {
        severity: "deny",
        tokens: ["--spacing-*"],
      },
    },
  };
  const before = structuredClone(config);
  const resolved = resolveLintConfig(config);
  expect(resolved.rules["canonical-utilities"]).toEqual({
    severity: "deny",
    pxToRem: false,
    rootFontSize: 20,
  });
  expect(resolved.rules["no-restricted-theme-tokens"]).toEqual({
    severity: "deny",
    tokens: ["--spacing-*"],
  });
  expect(config).toEqual(before);
  expect(
    resolveLintConfig({
      extends: [policy],
      rules: { "canonical-utilities": "off" },
    }).rules["canonical-utilities"],
  ).toEqual({
    severity: "off",
    pxToRem: false,
    rootFontSize: 20,
  });
  expect(
    resolveLintConfig({
      extends: [policy],
      rules: {
        "no-restricted-theme-tokens": {
          severity: "off",
          tokens: [],
        },
      },
    }).rules["no-restricted-theme-tokens"].tokens,
  ).toEqual([]);
});

test("inline overrides replace preset lists and retain inherited rule fields", () => {
  const policy: LintPreset = {
    rules: {
      "canonical-utilities": {
        pxToRem: false,
        rootFontSize: 20,
      },
      "unknown-utility": "deny",
    },
  };
  const project: LintPreset = {
    extends: [policy],
    rules: { "canonical-utilities": "warn" },
  };
  const override: LintPreset = {
    rules: { "canonical-utilities": "deny" },
  };
  const resolved = resolveLintConfig(mergeLintConfig(project, override));
  expect(resolved.rules["canonical-utilities"]).toEqual({
    severity: "deny",
    pxToRem: false,
    rootFontSize: 20,
  });
  expect(resolved.rules["unknown-utility"]).toBe("deny");
  const replaced = resolveLintConfig(
    mergeLintConfig(project, {
      extends: [],
    }),
  );
  expect(replaced.rules["unknown-utility"]).toBe("off");
  expect(replaced.rules["canonical-utilities"]).toEqual({
    severity: "warn",
    pxToRem: true,
    rootFontSize: 16,
  });
  expect(resolveLintConfig(mergeLintConfig(undefined, override)).rules["unknown-utility"]).toBe(
    "warn",
  );
});

test("inherited token restrictions are validated after all rule overrides", () => {
  const config = {
    projects: [
      {
        css: "root.css",
        files: ["*.tsx"],
        lint: {
          extends: [
            {
              rules: { "no-restricted-theme-tokens": "deny" as const },
            },
          ],
          rules: {
            "no-restricted-theme-tokens": {
              tokens: ["--color-*"],
            },
          },
        },
      },
    ],
  };
  expect(validateConfig(config)).toBe(config);
  expect(() =>
    resolveLintConfig({
      rules: { "no-restricted-theme-tokens": "deny" },
    }),
  ).toThrow("config.lint.rules.no-restricted-theme-tokens.tokens");
});

test("preset errors identify the name and path, and cycles do not reject shared ancestors", () => {
  const invalid = {
    extends: [
      {
        name: "company/broken",
        rules: {
          "canonical-utilities": {
            rootFontSize: 0,
          },
        },
      },
    ],
  };
  expect(() => resolveLintConfig(invalid)).toThrow(
    "config.lint.extends[0] (company/broken).rules.canonical-utilities.rootFontSize",
  );
  const cycle: {
    name: string;
    extends: LintPreset[];
  } = {
    name: "company/cycle",
    extends: [],
  };
  cycle.extends.push(cycle);
  expect(() => resolveLintConfig(cycle)).toThrow("cyclic lint preset extends");
  const shared: LintPreset = {
    rules: { "unknown-utility": "deny" },
  };
  expect(
    resolveLintConfig({
      extends: [
        {
          extends: [shared],
        },
        {
          extends: [shared],
        },
      ],
    }).rules["unknown-utility"],
  ).toBe("deny");
});

test("every catalog rule accepts reporting levels", () => {
  for (const rule of Object.keys(lintRules)) {
    for (const severity of ["warn", "deny", "off"] as const) {
      const setting =
        rule === "no-restricted-theme-tokens" && severity !== "off"
          ? { severity, tokens: ["--color-*"] }
          : severity;
      expect(() =>
        validateLintConfig(
          {
            rules: {
              [rule]: setting,
            },
          },
          "config.lint",
          false,
        ),
      ).not.toThrow();
    }
  }
});

test("utility policies accept typed options and reject malformed restrictions", () => {
  const lint: LintRules = {
    "prefer-direct-styling": "off",
    "duplicate-utilities": "deny",
    "conflicting-utilities": {
      allowShorthandOverrides: true,
    },
    "prefer-logical-properties": "warn",
    "length-units": {
      rules: [
        {
          properties: ["padding*"],
          disallow: ["px"],
        },
      ],
    },
    "no-restricted-theme-tokens": {
      tokens: ["--color-red-*", "--spacing-ref-*", "--radius-ref-*"],
    },
    "no-arbitrary-breakpoints": "deny",
  };
  const config = { rules: lint };
  expect(() => validateLintConfig(config, "config.lint", false)).not.toThrow();
  for (const lint of [
    { "conflicting-utilities": { allowShorthandOverrides: "true" } },
    { "length-units": { rules: [{ properties: [], disallow: ["px"] }] } },
    { "length-units": { rules: [{ properties: ["padding"], disallow: [16] }] } },
    { "no-restricted-theme-tokens": { tokens: ["spacing-*"] } },
    { "no-restricted-theme-tokens": { tokens: ["--"] } },
    { "no-arbitrary-breakpoints": { severity: "warn" } },
    { "no-restricted-theme-tokens": { regex: "red" } },
  ]) {
    expect(() => validateLintConfig({ rules: lint }, "config.lint", false)).toThrow();
  }
});

test("canonical conversion options and sorting levels are validated", () => {
  const config = {
    rules: {
      "canonical-utilities": {
        rootFontSize: 20.5,
        pxToRem: false,
      },
      "sort-utilities": "off",
    },
  };
  expect(() => validateLintConfig(config, "config.lint", false)).not.toThrow();
  for (const rootFontSize of [0, -1, Number.NaN, Number.POSITIVE_INFINITY, "16"]) {
    expect(() =>
      validateLintConfig(
        {
          rules: {
            "canonical-utilities": {
              rootFontSize,
            },
          },
        },
        "config.lint",
        false,
      ),
    ).toThrow();
  }
  expect(() =>
    validateLintConfig(
      {
        rules: {
          "sort-utilities": ["warn"],
        },
      },
      "config.lint",
      false,
    ),
  ).toThrow();
  expect(() =>
    validateLintConfig(
      {
        rules: {
          "canonical-utilities": undefined,
          "sort-utilities": undefined,
        },
      },
      "config.lint",
      false,
    ),
  ).not.toThrow();
});

test("source restrictions accept typed options and reject malformed restrictions", () => {
  const lint: LintRules = {
    "no-arbitrary-colors": "warn",
    "no-arbitrary-spacing": { properties: ["width", "inset-*"] },
    "no-class-property": "deny",
    "no-important": "warn",
    "valid-lint-suppressions": "warn",
    "no-unused-lint-suppressions": "warn",
    "no-restricted-utilities": {
      utilities: [
        {
          pattern: "legacy-*",
          message: "Use semantic utilities",
        },
      ],
    },
    "no-restricted-declarations": {
      rules: [
        {
          properties: ["transition-property"],
          values: ["all"],
          message: "Select properties",
        },
      ],
    },
  };
  const config = {
    rules: lint,
  };
  expect(() => validateLintConfig(config, "config.lint", false)).not.toThrow();
  for (const lint of [
    { "no-arbitrary-spacing": { properties: ["padding left"] } },
    { "no-arbitrary-colors": { severity: "warn" } },
    { "no-restricted-utilities": { utilities: [{ pattern: "" }] } },
    { "no-restricted-utilities": { utilities: [{ pattern: "p-*", message: 1 }] } },
    { "no-restricted-declarations": { rules: [{ properties: [], values: ["all"] }] } },
    { "no-restricted-declarations": { rules: [{ properties: ["padding"], values: [] }] } },
    {
      "no-restricted-declarations": {
        rules: [{ properties: ["padding"], values: ["1px"], fix: "p-1" }],
      },
    },
    { "no-important": "error" },
  ]) {
    expect(() =>
      validateLintConfig(
        {
          rules: lint,
        },
        "config.lint",
        false,
      ),
    ).toThrow();
  }
});
