import { expect, test } from "vite-plus/test";

import { lintsStylesheet, loadCssGraph, type CssHost } from "./css.ts";

test("only the user's own CSS is selected for stylesheet lint", () => {
  expect(lintsStylesheet("/app/src/app.css")).toBe(true);
  expect(lintsStylesheet("/app/node_modules/pkg/app.css")).toBe(false);
  const user = { id: "/app/src/app.css", lint: true };
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
