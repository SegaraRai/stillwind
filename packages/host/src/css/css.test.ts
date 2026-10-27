import type { StillwindPlugin } from "@stillwind/config";
import { expect, test } from "vite-plus/test";

import { lintsStylesheet, loadCssGraph, type CssHost } from "./css.ts";
import type { PluginHost } from "./plugins/host.ts";

test("only the user's own CSS is selected for stylesheet lint", () => {
  expect(lintsStylesheet("/app/src/app.css")).toBe(true);
  expect(lintsStylesheet("/app/node_modules/pkg/app.css")).toBe(false);
  const user = {
    id: "/app/src/app.css",
    lint: true,
  };
  expect(lintsStylesheet("/app/src/local.css", user, "./local.css")).toBe(true);
  expect(lintsStylesheet("/app/shared.css", user, "../shared.css")).toBe(true);
  expect(lintsStylesheet("C:\\app\\local.css", user, "C:\\app\\local.css")).toBe(true);
  expect(lintsStylesheet("/repo/packages/core/index.css", user, "stillwind")).toBe(false);
  expect(lintsStylesheet("/app/node_modules/pkg/a.css", user, "./node_modules/pkg/a.css")).toBe(
    false,
  );
  expect(
    lintsStylesheet(
      "/repo/packages/core/css/theme.css",
      {
        ...user,
        lint: false,
      },
      "./theme.css",
    ),
  ).toBe(false);
});

test("loading marks files for stylesheet lint and reports their identities", async () => {
  const files = new Map([
    ["/app/app.css", '@import "./local.css"; @import "pkg"; @stillwind-slot utilities;'],
    ["/app/local.css", ".a { color: red; }"],
    ["/pkg/index.css", '@import "./theme.css";'],
    ["/pkg/theme.css", ".b { color: blue; }"],
  ]);
  /**
   * Builds a CSS host over the in-memory files.
   * @param lint - Optional lint decision supplied by the host.
   * @returns A host that resolves `pkg` to `/pkg/index.css` and relative paths beside the importer.
   */
  const host = (lint?: CssHost["lint"]): CssHost => ({
    resolve: async (specifier, importer) =>
      specifier === "pkg"
        ? "/pkg/index.css"
        : `${importer.slice(0, importer.lastIndexOf("/"))}/${specifier.slice(2)}`,
    read: async (id) => files.get(id)!,
    documentUri: (id) => `file://${id}`,
    sourceMapSource: (id) => `file://${id}`,
    watch: () => {},
    lint,
  });
  expect((await loadCssGraph("/app/app.css", {}, host())).lintSources).toEqual([
    "/app/app.css",
    "/app/local.css",
  ]);
  expect(
    (
      await loadCssGraph(
        "/app/app.css",
        {},
        host(() => true),
      )
    ).lintSources,
  ).toHaveLength(4);
});

test("loading skips imports that the host cannot resolve or read", async () => {
  const files = new Map([
    [
      "/app/app.css",
      '@import "./missing.css"; @import "unknown"; @import "./local.css"; @stillwind-slot utilities;',
    ],
    ["/app/local.css", ".a { color: red; }"],
  ]);
  const watched: string[] = [];
  const graph = await loadCssGraph(
    "/app/app.css",
    {},
    {
      resolve: async (specifier, importer) => {
        if (specifier === "unknown") {
          throw new Error(`cannot resolve ${specifier}`);
        }
        return `${importer.slice(0, importer.lastIndexOf("/"))}/${specifier.slice(2)}`;
      },
      read: async (id) => {
        const content = files.get(id);
        if (content === undefined) {
          throw new Error(`cannot read ${id}`);
        }
        return content;
      },
      documentUri: (id) => `file://${id}`,
      sourceMapSource: (id) => `file://${id}`,
      watch: (id) => {
        watched.push(id);
      },
    },
  );
  expect(graph.lintSources).toEqual(["/app/app.css", "/app/local.css"]);
  // A file that resolves but cannot be read stays watched, so creating it reloads the graph.
  expect(watched).toContain("/app/missing.css");
});

test("plugins list directories through the host and watch directories apart from files", async () => {
  const files = new Map([["/app/app.css", '@plugin "lister"; @stillwind-slot utilities;']]);
  const listed: string[][] = [];
  const plugin: StillwindPlugin = {
    name: "lister",
    async css(context) {
      context.watch("/app/icons", "directory");
      context.watch("/app/icons/logo.svg");
      listed.push((await context.list("/app/icons")).map((entry) => entry.name));
      return "";
    },
  };
  const watched: string[] = [];
  const directories: string[] = [];
  const host = (list?: PluginHost["list"]): CssHost => ({
    resolve: async () => expect.fail("unexpected resolution"),
    read: async (id) => files.get(id) ?? expect.fail(`unexpected read of ${id}`),
    documentUri: (id) => `file://${id}`,
    sourceMapSource: (id) => `file://${id}`,
    watch: (id) => {
      watched.push(id);
    },
    watchDirectory: (id) => {
      directories.push(id);
    },
    plugins: {
      load: async () => expect.fail("unexpected plugin load"),
      resolve: async () => undefined,
      read: async (id) => expect.fail(`unexpected plugin read of ${id}`),
      list,
    },
  });

  await loadCssGraph(
    "/app/app.css",
    {},
    host(async (id) =>
      id === "/app/icons"
        ? [
            {
              name: "logo.svg",
              directory: false,
            },
          ]
        : [],
    ),
    undefined,
    undefined,
    {
      plugins: [plugin],
      root: "/app",
    },
  );
  expect(listed).toEqual([["logo.svg"]]);
  expect(directories).toEqual(["/app/icons"]);
  expect(watched).toContain("/app/icons/logo.svg");
  expect(watched).not.toContain("/app/icons");

  await expect(
    loadCssGraph("/app/app.css", {}, host(), undefined, undefined, {
      plugins: [plugin],
      root: "/app",
    }),
  ).rejects.toThrow("This host cannot list plugin directories: /app/icons");
});
