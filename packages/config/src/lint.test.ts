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
  expect(recommended.rules["mutating-macro-argument"]).toBe("deny");
  for (const setting of Object.values(resolveLintConfig({}).rules)) {
    expect(typeof setting === "string" ? setting : setting.severity).toBe("off");
  }
  const explicit = resolveLintConfig({
    rules: {
      "prefer-sorted-utilities": "warn",
      "prefer-canonical-utilities": {
        pxToRem: false,
      },
    },
  });
  expect(explicit.rules["prefer-sorted-utilities"]).toBe("warn");
  expect(explicit.rules["prefer-canonical-utilities"]).toEqual({
    severity: "warn",
    pxToRem: false,
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

test("false disables inherited lint and object overrides enable only their own rules", () => {
  const base = {
    extends: [lintPresets.strict],
  };
  expect(mergeLintConfig(base, false)).toBe(false);
  expect(mergeLintConfig(false, undefined)).toBe(false);
  expect(resolveLintConfig(false)).toEqual(resolveLintConfig({}));
  const enabled = resolveLintConfig(
    mergeLintConfig(false, {
      rules: {
        "unknown-utility": "deny",
      },
    }),
  );
  expect(enabled.rules["unknown-utility"]).toBe("deny");
  expect(enabled.rules["mutating-macro-argument"]).toBe("off");
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
  expect(strict["unused-lint-suppression"]).toBe("deny");
  expect(strict["no-arbitrary-colors"]).toBe("off");
  const stylistic = resolveLintConfig({
    extends: [lintPresets.stylistic],
  }).rules;
  expect(stylistic["prefer-sorted-utilities"]).toBe("warn");
  expect(stylistic["unknown-utility"]).toBe("off");
});

test("nested presets apply in order while severity changes preserve options and lists replace", () => {
  const policy = {
    name: "company/policy",
    rules: {
      "prefer-canonical-utilities": {
        severity: "warn",
        pxToRem: false,
      },
      "no-restricted-theme-tokens": {
        severity: "warn",
        tokens: ["--color-red-*"],
      },
    },
  } as const satisfies LintPreset;
  const strict: LintPreset = {
    extends: [policy],
    rules: { "prefer-canonical-utilities": "deny" },
  };
  const config: LintPreset = {
    extends: [policy, strict],
    rules: {
      "prefer-canonical-utilities": {
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
  expect(resolved.rules["prefer-canonical-utilities"]).toEqual({
    severity: "deny",
    pxToRem: false,
  });
  expect(resolved.rules["no-restricted-theme-tokens"]).toEqual({
    severity: "deny",
    tokens: ["--spacing-*"],
  });
  expect(config).toEqual(before);
  expect(
    resolveLintConfig({
      extends: [policy],
      rules: { "prefer-canonical-utilities": "off" },
    }).rules["prefer-canonical-utilities"],
  ).toEqual({
    severity: "off",
    pxToRem: false,
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
      "prefer-canonical-utilities": {
        pxToRem: false,
      },
      "unknown-utility": "deny",
    },
  };
  const project: LintPreset = {
    extends: [policy],
    rules: { "prefer-canonical-utilities": "warn" },
  };
  const override: LintPreset = {
    rules: { "prefer-canonical-utilities": "deny" },
  };
  const resolved = resolveLintConfig(mergeLintConfig(project, override));
  expect(resolved.rules["prefer-canonical-utilities"]).toEqual({
    severity: "deny",
    pxToRem: false,
  });
  expect(resolved.rules["unknown-utility"]).toBe("deny");
  const replaced = resolveLintConfig(
    mergeLintConfig(project, {
      extends: [],
    }),
  );
  expect(replaced.rules["unknown-utility"]).toBe("off");
  expect(replaced.rules["prefer-canonical-utilities"]).toEqual({
    severity: "warn",
    pxToRem: true,
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
          "prefer-canonical-utilities": {
            pxToRem: "yes",
          },
        },
      },
    ],
  };
  expect(() => resolveLintConfig(invalid as unknown as LintPreset)).toThrow(
    "config.lint.extends[0] (company/broken).rules.prefer-canonical-utilities.pxToRem",
  );
  const cycle: {
    /** Display name included in the cycle's configuration error path. */
    name: string;
    /** Mutable parent list that receives this preset itself to construct the cycle. */
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
    "duplicate-utility": "deny",
    "conflicting-utilities": {
      allowShorthandOverrides: true,
    },
    "prefer-logical-properties": "warn",
    "no-restricted-length-units": {
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
    { "no-restricted-length-units": { rules: [{ properties: [], disallow: ["px"] }] } },
    { "no-restricted-length-units": { rules: [{ properties: ["padding"], disallow: [16] }] } },
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
      "prefer-canonical-utilities": {
        pxToRem: false,
      },
      "prefer-sorted-utilities": "off",
    },
  };
  expect(() => validateLintConfig(config, "config.lint", false)).not.toThrow();
  for (const options of [
    {
      pxToRem: "true",
    },
    { pxToRem: 1 },
  ]) {
    expect(() =>
      validateLintConfig(
        {
          rules: {
            "prefer-canonical-utilities": options,
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
          "prefer-sorted-utilities": ["warn"],
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
          "prefer-canonical-utilities": undefined,
          "prefer-sorted-utilities": undefined,
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
    "invalid-lint-suppression": "warn",
    "unused-lint-suppression": "warn",
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

test("variant directive replacements keep px lengths separate unless enabled", () => {
  const resolved = resolveLintConfig({
    extends: [lintPresets.stylistic],
  });
  expect(resolved.rules["prefer-variant-directive"]).toEqual({
    severity: "warn",
    pxToRem: false,
  });
  expect(resolveLintConfig().rules["prefer-variant-directive"]).toEqual({
    severity: "off",
    pxToRem: false,
  });
  expect(
    resolveLintConfig({
      rules: {
        "prefer-variant-directive": {
          pxToRem: true,
        },
      },
    }).rules["prefer-variant-directive"],
  ).toEqual({ severity: "warn", pxToRem: true });
  expect(() =>
    validateLintConfig(
      {
        rules: {
          "prefer-variant-directive": {
            pxToRem: "yes",
          },
        },
      },
      "config.lint",
      false,
    ),
  ).toThrow();
});
