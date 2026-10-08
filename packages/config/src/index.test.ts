import { expect, test } from "vite-plus/test";

import {
  defineConfig,
  resolveConfig,
  validateConfig,
  type LintOptions,
  type StillwindConfig,
  type StillwindConfigExport,
} from "./index.ts";

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
