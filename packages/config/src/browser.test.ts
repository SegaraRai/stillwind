import { expect, test } from "vite-plus/test";

import { ConfigurationLoadError, loadBrowserConfig, resolveBrowserConfig } from "./browser.ts";
import { defineConfig } from "./configuration.ts";

test.each(["ts", "mts", "cts", "js", "mjs", "cjs", "json"])(
  "browser discovery passes the nearest %s configuration to its evaluator",
  async (extension) => {
    const file = `/workspace/app/stillwind.config.${extension}`;
    const loaded = await loadBrowserConfig("/workspace/app/src", {
      exists: (id) => id === file || id === "/workspace/stillwind.config.ts",
      list: () => [],
      evaluate: async (id) => {
        expect(id).toBe(file);
        return {
          projects: [
            {
              css: "theme.css",
              files: ["*.tsx"],
            },
          ],
        };
      },
    });
    expect(loaded?.configFile).toBe(file);
    expect(loaded?.projects[0].css).toBe("/workspace/app/theme.css");
  },
);

test("browser discovery evaluates async factories and merges selected project overrides", async () => {
  const host = {
    exists: (id: string) => id === "/workspace/stillwind.config.ts",
    list: () => [],
    evaluate: async () =>
      defineConfig(async () => ({
        projects: [
          {
            name: "react",
            root: "app",
            css: "theme.css",
            files: ["**/*.tsx"],
          },
          {
            name: "other",
            root: "other",
            css: "other.css",
            files: ["**/*.ts"],
          },
        ],
      })),
  };
  const result = await resolveBrowserConfig("/workspace/app", host, {
    automatic: false,
  });
  expect(result.configFile).toBe("/workspace/stillwind.config.ts");
  expect(result.config).toMatchObject({
    name: "react",
    root: "/workspace/app",
    automatic: false,
  });
  expect(result.projects[0]).toEqual(result.config);
  await expect(resolveBrowserConfig("/workspace", host)).rejects.toThrow(/Cannot select/);
  expect((await loadBrowserConfig("/workspace", host))?.projects).toMatchObject([
    { root: "/workspace/app" },
    { root: "/workspace/other" },
  ]);
  await expect(
    resolveBrowserConfig("/workspace", host, {
      configFile: "missing.ts",
    }),
  ).rejects.toThrow(/Cannot find/);
  expect(
    (
      await resolveBrowserConfig("/workspace", host, {
        configFile: false,
        css: "inline.css",
      })
    ).config.css,
  ).toBe("/workspace/inline.css");
});

test("browser load failures name the file that failed", async () => {
  const root = "/workspace/stillwind.config.ts";
  const referenced = "/workspace/app/stillwind.config.ts";
  const load = (settings: Record<string, unknown>) =>
    loadBrowserConfig("/workspace", {
      exists: (id) => id in settings,
      list: () => Object.keys(settings),
      evaluate: async (id) => {
        if (settings[id] instanceof Error) {
          throw settings[id];
        }
        return settings[id];
      },
    });
  const failure = new Error("evaluation failed");

  await expect(load({ [root]: failure })).rejects.toMatchObject({
    configFile: root,
    message: "evaluation failed",
    cause: failure,
  });
  await expect(
    load({
      [root]: {
        projects: ["app"],
      },
      [referenced]: failure,
    }),
  ).rejects.toBeInstanceOf(ConfigurationLoadError);
  await expect(
    load({
      [root]: {
        projects: ["app"],
      },
      [referenced]: failure,
    }),
  ).rejects.toMatchObject({
    configFile: referenced,
  });
  await expect(
    load({
      [root]: {
        projects: ["absent"],
      },
    }),
  ).rejects.toMatchObject({
    configFile: root,
  });
});

test("browser references expand through the captured file inventory", async () => {
  const settings: Record<string, unknown> = {
    "/workspace/stillwind.config.ts": {
      projects: ["packages/*", "!packages/legacy", "tools/custom.ts"],
    },
    "/workspace/packages/web/stillwind.config.ts": {
      projects: [
        {
          name: "web",
          css: "theme.css",
          files: ["*.tsx"],
        },
      ],
    },
    "/workspace/packages/web/stillwind.config.json": {
      projects: [
        {
          name: "unused",
          css: "unused.css",
          files: ["*.tsx"],
        },
      ],
    },
    "/workspace/packages/legacy/stillwind.config.ts": {
      projects: [
        {
          name: "legacy",
          css: "theme.css",
          files: ["*.tsx"],
        },
      ],
    },
    "/workspace/tools/custom.ts": {
      projects: [
        {
          root: "site",
          css: "theme.css",
          files: ["*.tsx"],
        },
      ],
    },
  };
  const loaded = await loadBrowserConfig("/workspace", {
    exists: (id) => id in settings,
    list: () => [...Object.keys(settings), "/workspace/packages/web/README.md"],
    evaluate: async (id) => settings[id],
  });
  expect(loaded?.projects).toMatchObject([
    {
      name: "web",
      root: "/workspace/packages/web",
      css: "/workspace/packages/web/theme.css",
    },
    {
      root: "/workspace/tools/site",
      css: "/workspace/tools/site/theme.css",
    },
  ]);
});
