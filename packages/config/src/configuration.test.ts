import { expect, test } from "vite-plus/test";

import {
  defineConfig,
  resolveConfig,
  validateConfig,
  type StillwindConfig,
  type StillwindConfigExport,
} from "./configuration.ts";

test.each([
  [
    {
      lint: {
        rules: {
          "no-restricted-theme-tokens": "warn",
        },
      },
    },
    "projects[0].lint.rules.no-restricted-theme-tokens.tokens",
  ],
  [
    {
      lint: {
        rules: {
          "length-units": {
            rules: [
              {
                properties: ["padding"],
                disallow: ["deg"],
              },
            ],
          },
        },
      },
    },
    "projects[0].lint.rules.length-units.rules[0].disallow",
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
        rules: {
          "canonical-utilities": {
            rootFontSize: 0,
          },
        },
      },
    },
    "projects[0].lint.rules.canonical-utilities.rootFontSize",
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

test("validation preserves objects and callbacks without executing providers", () => {
  const callback = () => expect.fail("Validation must not execute callbacks");
  const config = {
    projects: [
      {
        css: "theme.css",
        files: ["*.tsx"],
        naming: {
          utilityClass: callback,
        },
        sources: {
          kind: "custom" as const,
          supports: callback,
          load: callback,
        },
        projection: {
          kind: "custom" as const,
          prepare: callback,
        },
      },
    ],
  };
  const validated = validateConfig(config);
  expect(validated).toBe(config);
  expect(validated.projects[0]).toBe(config.projects[0]);
});

test.each([
  [{ files: [" "] }, "files"],
  [{ hashLength: 1.5 }, "hashLength"],
  [{ hashLength: 51 }, "hashLength"],
  [{ sort: { typo: true } }, "sort.typo"],
  [{ naming: { utilityClass: "name" } }, "naming.utilityClass"],
  [{ naming: [] }, "naming"],
  [{ lint: [] }, "lint"],
  [{ typeFacts: true }, "typeFacts"],
  [{ exclude: [""] }, "exclude"],
  [{ unknown: undefined }, "unknown"],
])("invalid project fields report their path: %j", (fields, field) => {
  expect(() =>
    validateConfig({
      projects: [
        {
          css: "theme.css",
          files: ["*.tsx"],
          ...fields,
        },
      ],
    }),
  ).toThrow(`projects[0].${field}`);
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
  ).rejects.toThrow("projects[0].css");
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
