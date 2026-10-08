import { expect, test } from "vite-plus/test";

import { loadBrowserConfig, resolveBrowserConfig } from "./browser.ts";
import { defineConfig } from "./configuration.ts";

test.each(["ts", "mts", "cts", "js", "mjs", "cjs", "json"])(
  "browser discovery passes the nearest %s configuration to its evaluator",
  async (extension) => {
    const file = `/workspace/app/stillwind.config.${extension}`;
    const loaded = await loadBrowserConfig("/workspace/app/src", {
      exists: (id) => id === file || id === "/workspace/stillwind.config.ts",
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
