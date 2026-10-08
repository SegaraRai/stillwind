import { expect, test } from "vite-plus/test";

import {
  defineConfig,
  resolveConfig,
  validateConfig,
  normalizeCompilerConfig,
  lintRules,
  type LintOptions,
  type StillwindConfig,
  type StillwindConfigExport,
} from "./index.ts";

test("every catalog rule accepts reporting levels and detailed rules retain their options", () => {
  for (const rule of Object.keys(lintRules)) {
    for (const severity of ["warn", "deny", "off"] as const) {
      const setting =
        rule === "no-restricted-theme-tokens" && severity !== "off"
          ? { severity, tokens: ["--color-*"] }
          : severity;
      expect(() =>
        validateConfig({
          projects: [
            {
              css: "root.css",
              files: ["*.tsx"],
              lint: {
                [rule]: setting,
              },
            },
          ],
        }),
      ).not.toThrow();
    }
  }
  expect(
    normalizeCompilerConfig({
      lint: {
        "canonical-utilities": {
          rootFontSize: 20,
        },
        "unknown-utility": "deny",
      },
    }),
  ).toMatchObject({
    utilityOrder: "authored",
    hashLength: 5,
    lint: {
      "canonical-utilities": {
        severity: "warn",
        pxToRem: true,
        rootFontSize: 20,
      },
      "unknown-utility": "deny",
    },
  });
});

test.each([
  [
    { lint: { "no-restricted-theme-tokens": "warn" } },
    "projects[0].lint.no-restricted-theme-tokens.tokens",
  ],
  [
    { lint: { "length-units": { rules: [{ properties: ["padding"], disallow: ["deg"] }] } } },
    "projects[0].lint.length-units.rules[0].disallow",
  ],
  [
    {
      prefix: "bad",
    },
    "projects[0].prefix",
  ],
  [
    {
      prefix: "--bad name-",
    },
    "projects[0].prefix",
  ],
  [
    {
      tailwindPrefix: "--sw-",
    },
    "projects[0].tailwindPrefix",
  ],
  [
    {
      utilityOrder: "unknown",
    },
    "projects[0].utilityOrder",
  ],
  [
    {
      sources: {
        kind: "custom",
        supports: () => true,
        load: async () => null,
        typo: true,
      },
    },
    "projects[0].sources.typo",
  ],
  [
    {
      projection: {
        kind: "custom",
        prepare: async () => null,
        typo: true,
      },
    },
    "projects[0].projection.typo",
  ],
  [
    {
      lint: {
        "canonical-utilities": {
          rootFontSize: 0,
        },
      },
    },
    "projects[0].lint.canonical-utilities.rootFontSize",
  ],
])("invalid settings identify their field: %j", (setting, location) => {
  expect(() =>
    validateConfig({
      projects: [
        {
          css: "root.css",
          files: ["*.tsx"],
          ...setting,
        },
      ],
    }),
  ).toThrow(location);
});

test("utility policies accept typed options and reject malformed restrictions", () => {
  const lint: LintOptions = {
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
  const config = {
    projects: [
      {
        css: "theme.css",
        files: ["*.tsx"],
        lint,
      },
    ],
  };
  expect(validateConfig(config)).toBe(config);
  for (const lint of [
    { "conflicting-utilities": { allowShorthandOverrides: "true" } },
    { "length-units": { rules: [{ properties: [], disallow: ["px"] }] } },
    { "length-units": { rules: [{ properties: ["padding"], disallow: [16] }] } },
    { "no-restricted-theme-tokens": { tokens: ["spacing-*"] } },
    { "no-restricted-theme-tokens": { tokens: ["--"] } },
    { "no-arbitrary-breakpoints": { severity: "warn" } },
    { "no-restricted-theme-tokens": { regex: "red" } },
  ]) {
    expect(() =>
      validateConfig({
        projects: [
          {
            css: "theme.css",
            files: ["*.tsx"],
            lint,
          },
        ],
      }),
    ).toThrow();
  }
});

test("lint conversion fields and independent sorting targets are validated", () => {
  const config = {
    projects: [
      {
        css: "theme.css",
        files: ["*.tsx"],
        lint: {
          "canonical-utilities": {
            rootFontSize: 20.5,
            pxToRem: false,
          },
          "sort-utilities": "off",
        },
        sort: {
          stringLiterals: false,
          arrays: true,
          attributes: true,
        },
      },
    ],
  };
  expect(validateConfig(config)).toBe(config);
  for (const rootFontSize of [0, -1, Number.NaN, Number.POSITIVE_INFINITY, "16"]) {
    expect(() =>
      validateConfig({
        projects: [
          {
            css: "theme.css",
            files: ["*.tsx"],
            lint: {
              "canonical-utilities": {
                rootFontSize,
              },
            },
          },
        ],
      }),
    ).toThrow();
  }
  expect(() =>
    validateConfig({
      projects: [
        {
          css: "theme.css",
          files: ["*.tsx"],
          sort: {
            attributes: "yes",
          },
        },
      ],
    }),
  ).toThrow();
  expect(() =>
    validateConfig({
      projects: [
        {
          css: "theme.css",
          files: ["*.tsx"],
          lint: {
            "sort-utilities": ["warn"],
          },
        },
      ],
    }),
  ).toThrow();
  expect(() =>
    validateConfig({
      projects: [
        {
          css: "theme.css",
          files: ["*.tsx"],
          lint: {
            "canonical-utilities": undefined,
            "sort-utilities": undefined,
          },
        },
      ],
    }),
  ).not.toThrow();
});

test("configuration keeps independent scopes and compiler settings", () => {
  const projects = [
    {
      css: "theme.css",
      files: ["src/**/*.tsx"],
      tsconfig: "tsconfig.app.json",
      prefix: "--app-",
    },
    {
      css: "other.css",
      files: ["src/**/*.tsx"],
      typeFacts: false as const,
      sources: { kind: "filesystem" as const },
    },
  ];
  expect(defineConfig({ projects }).projects).toBe(projects);
});

test.each(["function", "async function", "promise"])(
  "%s exports retain their form until configuration resolution",
  async (kind) => {
    const config: StillwindConfig = {
      projects: [
        {
          css: "theme.css",
          files: ["*.tsx"],
        },
      ],
    };
    let calls = 0;
    const create = () => {
      calls++;
      return config;
    };
    const exports: Record<string, StillwindConfigExport> = {
      function: create,
      "async function": async () => create(),
      promise: Promise.resolve(config),
    };
    const supplied = exports[kind];
    const defined = defineConfig(supplied);
    expect(defined).toBe(supplied);
    expect(calls).toBe(0);
    expect(await resolveConfig(defined)).toBe(config);
    expect(calls).toBe(kind === "promise" ? 0 : 1);
  },
);

test("resolved factory and promise values are validated", async () => {
  await expect(
    resolveConfig(async () => ({
      projects: [],
    })),
  ).rejects.toThrow("nonempty projects array");
  await expect(
    resolveConfig(
      Promise.resolve({
        projects: [
          {
            files: ["*.tsx"],
          },
        ],
      }),
    ),
  ).rejects.toThrow("must specify CSS");
});

test("configuration evaluation preserves factory exceptions and promise rejections", async () => {
  const failure = new Error("configuration failed");
  await expect(
    resolveConfig(() => {
      throw failure;
    }),
  ).rejects.toBe(failure);
  await expect(resolveConfig(Promise.reject(failure))).rejects.toBe(failure);
});

test.each([
  {},
  { projects: [] },
  { projects: [{ files: ["*.ts"] }] },
  { projects: [{ css: "theme.css", files: [] }] },
  { projects: [{ css: "theme.css", files: ["*.ts"], tsconfig: true }] },
  { projects: [{ css: "theme.css", files: ["*.ts"], sources: { kind: "custom" } }] },
  { projects: [{ css: "theme.css", files: ["*.ts"], projection: { kind: "custom" } }] },
])("invalid settings reject before loading resources: %j", (config) => {
  expect(() => validateConfig(config)).toThrow();
});

test("source restrictions validate options at the configuration boundary", () => {
  const lint: LintOptions = {
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
    projects: [
      {
        css: "theme.css",
        files: ["*.tsx"],
        lint,
      },
    ],
  };
  expect(validateConfig(config)).toBe(config);
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
      validateConfig({
        projects: [
          {
            css: "theme.css",
            files: ["*.tsx"],
            lint,
          },
        ],
      }),
    ).toThrow();
  }
});
