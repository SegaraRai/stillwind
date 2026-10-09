import type { ProjectInput } from "@stillwind/projection";
import { expect, test } from "vite-plus/test";

import {
  captureProjectConfig,
  defineConfig,
  resolveConfig,
  validateConfig,
  type StillwindConfig,
  type StillwindConfigExport,
} from "./configuration.ts";

test("captured project settings own their data and share only provider functions", () => {
  const naming = () => "card";
  const config = {
    css: "theme.css",
    files: ["*.tsx"],
    sort: {
      arrays: false,
    },
    lint: {
      rules: {
        "length-units": {
          rules: [
            {
              properties: ["*"],
              disallow: ["px"],
            },
          ],
        },
      },
    },
    naming: {
      utilityClass: naming,
    },
  };
  const captured = captureProjectConfig(config);
  config.files[0] = "*.vue";
  config.sort.arrays = true;
  config.lint.rules["length-units"].rules[0].disallow[0] = "rem";
  config.naming.utilityClass = () => "changed";
  expect(captured.files).toEqual(["*.tsx"]);
  expect(captured.sort).toEqual({
    arrays: false,
  });
  expect(captured.naming?.utilityClass).toBe(naming);
  expect(captured.lint).toEqual({
    rules: {
      "length-units": {
        rules: [
          {
            properties: ["*"],
            disallow: ["px"],
          },
        ],
      },
    },
  });
  expect(captureProjectConfig(captured)).toBe(captured);
});

test("captured callbacks retain inherited methods and provider receivers", async () => {
  const input = {
    resolverId: "/entry.ts",
    checkerFileName: "/entry.ts",
    projectionFileName: "/entry.ts",
    moduleId: "entry.ts",
    document: { uri: "file:///entry.ts", format: "script", code: "export const value = 1;" },
    syntax: { language: "ts", moduleKind: "module" },
  } satisfies ProjectInput;
  class Sources {
    readonly kind = "custom";
    #id = input.resolverId;

    supports(id: string): boolean {
      return id === this.#id;
    }

    async load(): Promise<ProjectInput> {
      return { ...input, resolverId: this.#id };
    }

    async resolve(): Promise<string> {
      return this.#id;
    }
  }
  class Projection {
    readonly kind = "custom";
    #source = { ...input, kind: "direct" as const };

    async prepare() {
      return this.#source;
    }
  }
  class Naming {
    utilityClass(): string {
      return "card";
    }
  }
  const sources = new Sources();
  const projection = new Projection();
  const naming = new Naming();
  const config = { css: "theme.css", files: ["*.ts"], sources, projection, naming };
  validateConfig({ projects: [config] });
  const captured = captureProjectConfig(config);
  sources.supports = () => false;
  sources.load = async () => expect.fail("Source method was replaced after capture");
  sources.resolve = async () => expect.fail("Resolution method was replaced after capture");
  projection.prepare = async () => expect.fail("Projection method was replaced after capture");
  naming.utilityClass = () => "changed";

  if (captured.sources?.kind !== "custom" || captured.projection?.kind !== "custom") {
    expect.fail("Expected captured custom providers");
  }
  const context = { root: "/", documents: new Map<string, string>() };
  expect(captured.sources.supports(input.resolverId)).toBe(true);
  await expect(captured.sources.load(input.resolverId, context)).resolves.toEqual(input);
  await expect(captured.sources.resolve?.("./entry", "/importer.ts", context)).resolves.toBe(
    input.resolverId,
  );
  await expect(captured.projection.prepare(input)).resolves.toEqual({ ...input, kind: "direct" });
  expect(
    captured.naming?.utilityClass?.(
      { module: input.moduleId, namespace: "", call: 0, branch: "test" },
      Buffer.alloc(0),
    ),
  ).toBe("card");
});

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
  /**
   * Fails if validation invokes a provider or naming callback instead of checking its shape.
   * @throws Whenever invoked.
   */
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
    /**
     * Records factory evaluation separately from configuration definition.
     * @returns The same settings object after increasing the invocation count.
     */
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
