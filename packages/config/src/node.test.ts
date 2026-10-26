import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, onTestFinished, test } from "vite-plus/test";

import { resolveBrowserConfig } from "./browser.ts";
import { lintPresets } from "./lint-presets.ts";
import { findConfiguration, loadConfiguration, resolveIntegrationConfig } from "./node.ts";

/**
 * Creates an isolated configuration directory and registers its removal after the test.
 * @returns The root, default TypeScript config path, and a helper for replacing its project list.
 * @throws If creating the temporary directory fails.
 */
async function fixture() {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-"));
  onTestFinished(() => rm(root, { recursive: true, force: true }));
  const configFile = path.join(root, "stillwind.config.ts");
  /**
   * Replaces the fixture's configuration with a default export of the supplied projects.
   * @param projects - Project records, including intentionally invalid records used by validation tests.
   * @returns Completion of writing the TypeScript configuration.
   * @throws If serialization or file writing fails.
   */
  const write = (projects: object[]) =>
    writeFile(configFile, `export default ${JSON.stringify({ projects })}`);
  return {
    root,
    configFile,
    write,
  };
}

test("adapter targets are defaults below project and inline targets, including empty lists", async () => {
  const { root, write } = await fixture();
  const defaults = { target: ["chrome120", "safari17"] };
  const inline = { css: "theme.css", configFile: false } as const;
  expect((await resolveIntegrationConfig(inline, root, defaults)).config.target).toEqual(
    defaults.target,
  );
  expect(
    (await resolveIntegrationConfig({ ...inline, target: [] }, root, defaults)).config.target,
  ).toEqual([]);

  await write([{ css: "theme.css", files: ["*.ts"] }]);
  expect((await resolveIntegrationConfig({}, root, defaults)).config.target).toEqual(
    defaults.target,
  );
  for (const target of [[], ["safari16.4"]]) {
    await write([{ css: "theme.css", files: ["*.ts"], target }]);
    expect((await resolveIntegrationConfig({}, root, defaults)).config.target).toEqual(target);
    expect(
      (await resolveIntegrationConfig({ target: "firefox120" }, root, defaults)).config.target,
    ).toEqual(["firefox120"]);
    expect((await resolveIntegrationConfig({ target: [] }, root, defaults)).config.target).toEqual(
      [],
    );
  }
  expect(defaults.target).toEqual(["chrome120", "safari17"]);
});

test.each(["ts", "mts", "cts", "js", "mjs", "cjs", "json"])(
  "%s configuration is discovered, evaluated, and reloaded with its imports",
  async (extension) => {
    const { root } = await fixture();
    const configFile = path.join(root, `stillwind.config.${extension}`);
    /**
     * Creates settings for checking configuration loading across module formats.
     * @param css - Authored stylesheet path that identifies the current fixture revision.
     * @returns One project whose scope contains TSX files in the configuration directory.
     */
    const settings = (css: string) => ({
      projects: [
        {
          css,
          files: ["*.tsx"],
        },
      ],
    });
    const helper = path.join(root, "settings.json");
    await writeFile(helper, JSON.stringify(settings("first.css")));
    await writeFile(
      configFile,
      extension === "json"
        ? JSON.stringify(settings("first.css"))
        : extension === "cts" || extension === "cjs"
          ? 'const settings = require("./settings.json"); module.exports = async () => settings;'
          : 'import settings from "./settings.json"; export default async () => settings;',
    );
    const app = path.join(root, "app");
    await mkdir(app);
    expect(findConfiguration(app)).toBe(configFile);
    const first = await resolveIntegrationConfig({}, app);
    expect(first.configFile).toBe(configFile);
    expect(first.config.css).toBe(path.join(root, "first.css"));
    expect(first.dependencies).toContain(configFile);
    if (extension === "json") {
      await writeFile(configFile, JSON.stringify(settings("second.css")));
    } else {
      expect(first.dependencies).toContain(helper);
      await writeFile(helper, JSON.stringify(settings("second.css")));
    }
    expect((await loadConfiguration(configFile)).config.projects[0].css).toBe("second.css");
  },
);

test("discovery prefers the nearest directory and tracks candidates before its selected extension", async () => {
  const { root, configFile, write } = await fixture();
  await write([
    {
      css: "parent.css",
      files: ["*.tsx"],
    },
  ]);
  const app = path.join(root, "app");
  await mkdir(app);
  const local = path.join(app, "stillwind.config.mts");
  await writeFile(local, 'export default { projects: [{ css: "local.css", files: ["*.tsx"] }] };');
  const resolved = await resolveIntegrationConfig({}, app);
  expect(resolved.configFile).toBe(local);
  expect(resolved.missing).toEqual([path.join(app, "stillwind.config.ts")]);
  expect(resolved.dependencies).not.toContain(configFile);
  await writeFile(path.join(app, "stillwind.config.ts"), "export default { projects: [] };");
  await expect(resolveIntegrationConfig({}, app)).rejects.toThrow("nonempty projects array");
});

test("Node and browser compose imported presets and keep project defaults through inline overrides", async () => {
  const { root, configFile, write } = await fixture();
  const presetFile = path.join(root, "policy.ts");
  const policy = {
    extends: [lintPresets.recommended],
    rules: {
      "prefer-canonical-utilities": {
        severity: "warn",
        pxToRem: false,
      },
    },
  } as const;
  await writeFile(presetFile, `export default ${JSON.stringify(policy)}`);
  await writeFile(
    configFile,
    'import policy from "./policy.ts"; export default { projects: [{css:"theme.css",files:["*.tsx"],lint:{extends:[policy]}}] };',
  );
  const inline = { lint: { rules: { "prefer-canonical-utilities": "deny" } } } as const;
  const node = await resolveIntegrationConfig(inline, root);
  expect(node.dependencies).toContain(presetFile);
  const browser = await resolveBrowserConfig(
    root.replaceAll("\\", "/"),
    {
      exists: (id) => id === configFile.replaceAll("\\", "/"),
      evaluate: async () => ({
        projects: [
          {
            css: "theme.css",
            files: ["*.tsx"],
            lint: {
              extends: [policy],
            },
          },
        ],
      }),
    },
    inline,
  );
  expect(browser.config.lint).toEqual(node.config.lint);
  expect(node.config.lint.rules["prefer-canonical-utilities"]).toEqual({
    severity: "deny",
    pxToRem: false,
    collapse: true,
    logicalToPhysical: false,
    preserveSpecificity: true,
  });
  const replaced = await resolveIntegrationConfig(
    {
      lint: {
        extends: [],
      },
    },
    root,
  );
  expect(replaced.config.lint.rules["unknown-utility"]).toBe("off");
  await write([
    {
      css: "theme.css",
      files: ["*.tsx"],
    },
  ]);
  expect((await resolveIntegrationConfig(inline, root)).config.lint.rules["unknown-utility"]).toBe(
    "warn",
  );
  const isolated = await resolveIntegrationConfig(
    {
      configFile: false,
      css: "theme.css",
      lint: {},
    },
    root,
  );
  for (const setting of Object.values(isolated.config.lint.rules)) {
    expect(typeof setting === "string" ? setting : setting.severity).toBe("off");
  }
});

test("discovery and project paths use the configuration directory", async () => {
  const { root, configFile, write } = await fixture();
  const app = path.join(root, "apps/web");
  await mkdir(app, { recursive: true });
  await write([
    {
      root: "apps/web",
      css: "theme.css",
      files: ["src/*.tsx"],
    },
  ]);
  expect(findConfiguration(app)).toBe(configFile);
  const resolved = await resolveIntegrationConfig({}, app);
  expect(resolved.config).toMatchObject({
    root: app,
    css: path.join(app, "theme.css"),
  });
  expect(resolved.dependencies).toContain(configFile);
  expect(resolved.missing).toContain(path.join(app, "stillwind.config.ts"));
});

test("an explicit file uses the application root while inline overrides preserve false", async () => {
  const { root, configFile, write } = await fixture();
  await write([
    {
      root: "old",
      css: "theme.css",
      files: ["*.tsx"],
      automatic: true,
    },
  ]);
  const config = (
    await resolveIntegrationConfig(
      {
        configFile: path.basename(configFile),
        root: "new",
        automatic: false,
        css: undefined,
      },
      root,
    )
  ).config;
  expect(config).toMatchObject({
    root: path.join(root, "new"),
    css: path.join(root, "old", "theme.css"),
    automatic: false,
  });
});

test("Node and browser preserve file-authored paths when inline root changes", async () => {
  const { root, configFile, write } = await fixture();
  const settings = {
    root: "old",
    css: "theme.css",
    tsconfig: "tsconfig.app.json",
    files: ["*.tsx"],
    lint: {
      rules: {
        "unknown-utility": "warn",
      },
    },
  };
  await write([settings]);
  const overrides = {
    root: "new",
    lint: {
      rules: {
        "unknown-utility": "deny" as const,
      },
    },
  };
  const node = await resolveIntegrationConfig(overrides, root);
  const virtualRoot = root.replaceAll("\\", "/");
  const browser = await resolveBrowserConfig(
    virtualRoot,
    {
      exists: (file) => file === configFile.replaceAll("\\", "/"),
      evaluate: async () => ({
        projects: [settings],
      }),
    },
    overrides,
  );
  expect(node.config).toMatchObject({
    root: path.join(root, "new"),
    css: path.join(root, "old", "theme.css"),
    tsconfig: path.join(root, "old", "tsconfig.app.json"),
    lint: {
      rules: {
        "unknown-utility": "deny",
      },
    },
  });
  expect(browser.config.css).toBe(node.config.css.replaceAll("\\", "/"));
  expect(browser.config.tsconfig).toBe((node.config.tsconfig as string).replaceAll("\\", "/"));
  expect(
    (
      await resolveIntegrationConfig(
        {
          ...overrides,
          css: "inline.css",
          tsconfig: false,
        },
        root,
      )
    ).config,
  ).toMatchObject({
    css: path.join(root, "new", "inline.css"),
    tsconfig: false,
  });
});

test("Node and browser preserve disabling and reenable explicit lint and sort options", async () => {
  const { root, configFile, write } = await fixture();
  const disabled = {
    lint: false,
    sort: false,
  } as const;
  for (const [settings, inline, sort, unknownUtility] of [
    [
      disabled,
      {},
      {
        stringLiterals: false,
        arrays: false,
        applyUtilities: false,
        applyStatements: false,
        variantBlocks: false,
      },
      "off",
    ],
    [
      {
        lint: {
          extends: [lintPresets.strict],
        },
        sort: {
          applyStatements: true,
        },
      },
      disabled,
      {
        stringLiterals: false,
        arrays: false,
        applyUtilities: false,
        applyStatements: false,
        variantBlocks: false,
      },
      "off",
    ],
    [
      disabled,
      {
        lint: {
          rules: {
            "unknown-utility": "deny",
          },
        },
        sort: {
          applyStatements: true,
        },
      },
      {
        stringLiterals: true,
        arrays: true,
        applyUtilities: true,
        applyStatements: true,
        variantBlocks: false,
      },
      "deny",
    ],
  ] as const) {
    const project = {
      css: "theme.css",
      files: ["*.tsx"],
      ...settings,
    };
    await write([project]);
    const node = await resolveIntegrationConfig(inline, root);
    const browser = await resolveBrowserConfig(
      root.replaceAll("\\", "/"),
      {
        exists: (file) => file === configFile.replaceAll("\\", "/"),
        evaluate: async () => ({
          projects: [project],
        }),
      },
      inline,
    );
    expect(node.config.sort).toEqual(sort);
    expect(node.config.lint.rules["unknown-utility"]).toBe(unknownUtility);
    expect(node.config.lint.rules["mutating-macro-argument"]).toBe("off");
    expect(browser.config.sort).toEqual(node.config.sort);
    expect(browser.config.lint).toEqual(node.config.lint);
  }
});

test("configFile false bypasses an invalid configuration", async () => {
  const { root, configFile } = await fixture();
  await writeFile(configFile, "throw new Error('configuration evaluated')");
  const resolved = await resolveIntegrationConfig(
    {
      configFile: false,
      css: "inline.css",
    },
    root,
  );
  expect(resolved.config.css).toBe(path.join(root, "inline.css"));
  expect(resolved.dependencies).toEqual([]);
  await expect(
    resolveIntegrationConfig(
      {
        css: "inline.css",
      },
      root,
    ),
  ).rejects.toThrow("configuration evaluated");
});

test("inline overrides validate after merging with the selected project", async () => {
  const { root, write } = await fixture();
  await write([
    {
      css: "root.css",
      files: ["*.tsx"],
      prefix: "--project-",
      lint: {
        rules: {
          "no-restricted-theme-tokens": {
            severity: "warn",
            tokens: ["--color-*"],
          },
        },
      },
    },
  ]);
  expect(
    (
      await resolveIntegrationConfig(
        {
          tailwindPrefix: "--sw-",
        },
        root,
      )
    ).config,
  ).toMatchObject({
    prefix: "--project-",
    tailwindPrefix: "--sw-",
  });
  expect(
    (
      await resolveIntegrationConfig(
        {
          lint: {
            rules: {
              "no-restricted-theme-tokens": {
                severity: "deny",
              },
            },
          },
        },
        root,
      )
    ).config.lint.rules,
  ).toMatchObject({
    "no-restricted-theme-tokens": {
      severity: "deny",
      tokens: ["--color-*"],
    },
  });
  await expect(
    resolveIntegrationConfig(
      {
        tailwindPrefix: "--project-",
      },
      root,
    ),
  ).rejects.toThrow("config.tailwindPrefix");
});

test("missing configuration accepts complete inline options and rejects explicit missing paths", async () => {
  const { root } = await fixture();
  expect(
    (
      await resolveIntegrationConfig(
        {
          css: "inline.css",
        },
        root,
      )
    ).config.root,
  ).toBe(root);
  await expect(resolveIntegrationConfig({}, root)).rejects.toThrow("must specify CSS");
  await expect(
    resolveIntegrationConfig(
      {
        configFile: "absent.ts",
        css: "inline.css",
      },
      root,
    ),
  ).rejects.toThrow("Cannot find Stillwind configuration");
});

test("unnamed projects can be selected by their distinct roots", async () => {
  const { root, write } = await fixture();
  await write([
    { css: "base.css", files: ["*.ts"] },
    { root: "web", css: "web.css", files: ["*.tsx"] },
  ]);
  expect((await resolveIntegrationConfig({}, path.join(root, "web"))).config.css).toBe(
    path.join(root, "web", "web.css"),
  );
});

test("overlapping project roots require an explicit name", async () => {
  const { root, write } = await fixture();
  await write([
    { css: "first.css", files: ["*.ts"] },
    { name: "web", css: "second.css", files: ["*.tsx"] },
  ]);
  await expect(resolveIntegrationConfig({}, root)).rejects.toThrow(
    "Cannot select one Stillwind project",
  );
  expect((await resolveIntegrationConfig({ project: "web" }, root)).config.css).toBe(
    path.join(root, "second.css"),
  );
  await expect(resolveIntegrationConfig({ project: "absent" }, root)).rejects.toThrow(
    "expected one matching project",
  );
});

test("projects outside the application root do not silently select an array element", async () => {
  const { root, write } = await fixture();
  await write([
    { root: "first", css: "first.css", files: ["*.ts"] },
    { root: "second", css: "second.css", files: ["*.tsx"] },
  ]);
  await expect(resolveIntegrationConfig({}, root)).rejects.toThrow(
    "Cannot select one Stillwind project",
  );
});

test.each(["ts", "cjs", "json"])(
  "a changed %s helper is tracked and evaluated afresh",
  async (extension) => {
    const { root, configFile } = await fixture();
    const helper = path.join(root, `settings.${extension}`);
    /**
     * Creates settings for detecting edits to an imported configuration helper.
     * @param css - Authored stylesheet path that identifies the helper revision.
     * @returns One project whose scope contains TSX files in the configuration directory.
     */
    const settings = (css: string) => ({
      projects: [
        {
          css,
          files: ["*.tsx"],
        },
      ],
    });
    /**
     * Serializes the imported helper in the module format selected by the test.
     * @param css - Authored stylesheet path inserted into the helper's project settings.
     * @returns JSON text, a CommonJS assignment, or an ESM default export.
     */
    const source = (css: string) =>
      extension === "json"
        ? JSON.stringify(settings(css))
        : `${extension === "cjs" ? "module.exports =" : "export default"} ${JSON.stringify(settings(css))}`;
    await writeFile(helper, source("first.css"));
    await writeFile(
      configFile,
      `import settings from './settings.${extension}'; export default async () => settings;`,
    );
    const first = await loadConfiguration(configFile);
    expect(first.dependencies).toContain(helper);
    expect(first.config.projects[0].css).toBe("first.css");
    await writeFile(helper, source("second.css"));
    expect((await loadConfiguration(configFile)).config.projects[0].css).toBe("second.css");
  },
);

test("bundled helpers retain their own directory and unused imports do not execute", async () => {
  const { root, configFile } = await fixture();
  const helperRoot = path.join(root, "config");
  await mkdir(helperRoot);
  await writeFile(path.join(helperRoot, "paths.ts"), "export const root = import.meta.dirname;");
  await writeFile(path.join(root, "unused.cjs"), "throw new Error('unused import executed');");
  await writeFile(
    configFile,
    `import {root} from './config/paths.ts';
import type {Unused} from './unused.cjs';
export default {projects:[{root,css:'theme.css',files:['*.tsx']}]};`,
  );
  expect((await loadConfiguration(configFile)).config.projects[0].root).toBe(helperRoot);
});

test.each(["mts", "cts"])(
  "%s configuration resolves and watches the selected package export",
  async (extension) => {
    const { root } = await fixture();
    const directory = path.join(root, "node_modules/preset");
    await mkdir(directory, { recursive: true });
    await writeFile(
      path.join(directory, "package.json"),
      JSON.stringify({
        type: "module",
        exports: {
          import: "./import.js",
          require: "./require.cjs",
        },
      }),
    );
    const settings = (css: string) =>
      JSON.stringify({
        projects: [
          {
            css,
            files: ["*.tsx"],
          },
        ],
      });
    await writeFile(path.join(directory, "import.js"), `export default ${settings("import.css")};`);
    await writeFile(
      path.join(directory, "require.cjs"),
      `module.exports = ${settings("require.css")};`,
    );
    const file = path.join(root, `stillwind.config.${extension}`);
    await writeFile(
      file,
      extension === "mts"
        ? 'import settings from "preset"; export default settings;'
        : 'module.exports = require("preset");',
    );
    const result = await loadConfiguration(file);
    expect(result.config.projects[0].css).toBe(extension === "mts" ? "import.css" : "require.css");
    expect(result.dependencies).toContain(
      path.join(directory, extension === "mts" ? "import.js" : "require.cjs"),
    );
  },
);
