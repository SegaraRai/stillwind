import type { IconifyJSON } from "@iconify/types";
import type { PluginContext } from "@stillwind/config";
import { describe, expect, test } from "vite-plus/test";

import { iconify } from "./plugin.ts";

const collection: IconifyJSON = {
  prefix: "demo",
  width: 24,
  height: 24,
  icons: {
    close: {
      body: '<path fill="currentColor" d="M0 0h24v24z"/>',
    },
    flag: {
      body: '<path fill="#f00" d="M0 0h24v12z"/>',
      width: 48,
    },
  },
  aliases: {
    cross: {
      parent: "close",
    },
  },
};

/**
 * Creates the context of one `@plugin` statement.
 * @param options - Declarations of the statement's options block.
 * @returns A context whose project directory contains no icon set packages.
 */
function context(options: Record<string, string> = {}): PluginContext {
  return {
    options,
    root: import.meta.dirname,
    watch: () => {},
    resolve: async () => undefined,
    read: async (path) => {
      throw new Error(`${path} does not exist`);
    },
  };
}

describe("iconify", () => {
  test("declares the forms of its prefix that the syntax selects", async () => {
    const plugin = iconify();

    const dynamic = async (options?: Record<string, string>, instance = plugin) =>
      (await instance.css(context(options))).match(/^@stillwind-dynamic .*$/gm);

    expect(await dynamic()).toStrictEqual([
      "@stillwind-dynamic icon-*;",
      "@stillwind-dynamic icon-[*];",
    ]);
    expect(await dynamic({ prefix: "i", syntax: "named" })).toStrictEqual([
      "@stillwind-dynamic i-*;",
    ]);
    expect(await dynamic({}, iconify({ syntax: "bracket" }))).toStrictEqual([
      "@stillwind-dynamic icon-[*];",
    ]);
  });

  test("defines size utilities with its prefix and a shared size namespace", async () => {
    const css = await iconify().css(context({ prefix: "i" }));

    expect(css).toContain("@stillwind-theme-namespace --icon-size-* [length] [percentage];");
    expect(css).toContain("value-for: height;");
    expect(css).toContain("@utility i-full {\n  --sw-icon-size: 100%;\n}");
    expect(css).toContain(
      "@utility i-* {\n  --sw-icon-size: --value(--icon-size-*, [length], [percentage]);\n}",
    );
  });

  test("bracket candidates define their bracket names and share the named token", async () => {
    const plugin = iconify({
      syntax: "bracket",
      collections: {
        demo: collection,
      },
    });
    const css = await plugin.resolve!(["icon-[demo--close]", "icon-demo--flag"], context());

    expect(css).toContain("@utility icon-[demo--close] (implementation) {");
    expect(css).toContain("mask-image: var(--icon-svg-demo--close);");
    expect(css).not.toContain("icon-demo--flag");
    expect(await plugin.catalog!(context(), "")).toContain("icon-[demo--close]");
  });

  test("paints currentColor icons through a mask and colored icons as a background", async () => {
    const plugin = iconify({
      collections: {
        demo: collection,
      },
    });
    const css = await plugin.resolve!(["icon-demo--close", "icon-demo--flag"], context());

    expect(css).toContain("@utility icon-demo--close (implementation) {");
    expect(css).toContain('--icon-svg-demo--close: url("data:image/svg+xml,');
    expect(css).toContain("mask-image: var(--icon-svg-demo--close);");
    expect(css).toContain("@utility icon-demo--flag (implementation) {");
    expect(css).toContain("background-image: var(--icon-svg-demo--flag);");
    expect(css).toContain("aspect-ratio: 48 / 24;");
  });

  test("resolves aliases and applies statement options", async () => {
    const plugin = iconify({
      collections: {
        demo: collection,
      },
    });
    const css = await plugin.resolve!(
      ["i-demo--cross"],
      context({
        prefix: "i",
        scale: "1.5",
        mode: "background",
      }),
    );

    expect(css).toContain("@utility i-demo--cross (implementation) {");
    expect(css).toContain("height: var(--sw-icon-size, 1.5em);");
    expect(css).toContain("background-image: var(--icon-svg-demo--cross);");
  });

  test("leaves candidates without a known icon undefined", async () => {
    const plugin = iconify({
      collections: {
        demo: collection,
      },
    });

    expect(
      await plugin.resolve!(
        ["icon-demo--missing", "icon-unknown--close", "icon-demo", "icon-demo--Close"],
        context(),
      ),
    ).toBe("");
  });

  test("lists configured icons for completion", async () => {
    const plugin = iconify({
      collections: {
        demo: collection,
      },
    });

    expect(await plugin.catalog!(context(), "")).toEqual(
      expect.arrayContaining(["icon-demo--close", "icon-demo--flag", "icon-demo--cross"]),
    );
  });

  test("installed icon set packages are found through the host", async () => {
    const files = new Map([
      ["/project/package.json", JSON.stringify({ devDependencies: { "@iconify-json/demo": "1" } })],
      ["/project/node_modules/@iconify-json/demo/icons.json", JSON.stringify(collection)],
    ]);
    const watched: string[] = [];
    const host: PluginContext = {
      options: {},
      root: "/project",
      watch: (path) => watched.push(path),
      resolve: async (specifier) =>
        specifier === "@iconify-json/demo/icons.json"
          ? "/project/node_modules/@iconify-json/demo/icons.json"
          : undefined,
      read: async (path) => files.get(path) ?? expect.fail(`unexpected read of ${path}`),
    };
    const plugin = iconify();

    expect(await plugin.resolve!(["icon-demo--close"], host)).toContain(
      "@utility icon-demo--close",
    );
    expect(await plugin.catalog!(host, "")).toContain("icon-demo--flag");
    expect(watched).toContain("/project/node_modules/@iconify-json/demo/icons.json");
  });

  test("@iconify/json completes set names, then the icons of the set that the input names", async () => {
    const files = new Map([
      ["/project/package.json", "{}"],
      [
        "/project/node_modules/@iconify/json/collections.json",
        JSON.stringify({
          demo: {},
          other: {},
        }),
      ],
      ["/project/node_modules/@iconify/json/json/demo.json", JSON.stringify(collection)],
    ]);
    const watched: string[] = [];
    const host: PluginContext = {
      options: {},
      root: "/project",
      watch: (path) => watched.push(path),
      resolve: async (specifier) =>
        files.has(`/project/node_modules/${specifier}`)
          ? `/project/node_modules/${specifier}`
          : undefined,
      read: async (path) => files.get(path) ?? expect.fail(`unexpected read of ${path}`),
    };
    const plugin = iconify();

    expect(await plugin.catalog!(host, "")).toEqual([]);
    expect(await plugin.catalog!(host, "icon-")).toEqual(["icon-demo--", "icon-other--"]);
    expect(await plugin.catalog!(host, "icon-[")).toEqual(["icon-[demo--", "icon-[other--"]);
    expect(await plugin.catalog!(host, "icon-demo--cl")).toEqual([
      "icon-demo--close",
      "icon-demo--flag",
      "icon-demo--cross",
    ]);
    expect(await plugin.catalog!(host, "icon-[demo--")).toContain("icon-[demo--close]");
    expect(await plugin.catalog!(host, "icon-missing--")).toEqual([]);
    expect(watched).toContain("/project/node_modules/@iconify/json/collections.json");
  });

  test("rejects invalid options", async () => {
    expect(() => iconify({ prefix: "Icon" })).toThrow("Invalid Iconify utility prefix");
    expect(() => iconify().css(context({ scale: "0" }))).toThrow("positive number");
    expect(() => iconify().css(context({ mode: "outline" }))).toThrow("Iconify mode");
    expect(() => iconify().css(context({ syntax: "plain" }))).toThrow("Iconify syntax");
  });
});
