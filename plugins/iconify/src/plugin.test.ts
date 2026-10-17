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

    expect(await plugin.css(context())).toBe(
      "@stillwind-dynamic icon-*;\n@stillwind-dynamic icon-[*];\n",
    );
    expect(await plugin.css(context({ prefix: "i", syntax: "named" }))).toBe(
      "@stillwind-dynamic i-*;\n",
    );
    expect(await iconify({ syntax: "bracket" }).css(context())).toBe(
      "@stillwind-dynamic icon-[*];\n",
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

    expect(css).toContain("@utility icon-[demo--close] {");
    expect(css).toContain("mask-image: var(--icon-demo--close);");
    expect(css).not.toContain("icon-demo--flag");
    expect(await plugin.catalog!(context())).toContain("icon-[demo--close]");
  });

  test("paints currentColor icons through a mask and colored icons as a background", async () => {
    const plugin = iconify({
      collections: {
        demo: collection,
      },
    });
    const css = await plugin.resolve!(["icon-demo--close", "icon-demo--flag"], context());

    expect(css).toContain("@utility icon-demo--close {");
    expect(css).toContain('--icon-demo--close: url("data:image/svg+xml,');
    expect(css).toContain("mask-image: var(--icon-demo--close);");
    expect(css).toContain("@utility icon-demo--flag {");
    expect(css).toContain("background-image: var(--icon-demo--flag);");
    expect(css).toContain("width: 2em;");
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

    expect(css).toContain("@utility i-demo--cross {");
    expect(css).toContain("height: 1.5em;");
    expect(css).toContain("background-image: var(--i-demo--cross);");
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

    expect(await plugin.catalog!(context())).toEqual(
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
    expect(await plugin.catalog!(host)).toContain("icon-demo--flag");
    expect(watched).toContain("/project/node_modules/@iconify-json/demo/icons.json");
  });

  test("rejects invalid options", async () => {
    expect(() => iconify({ prefix: "Icon" })).toThrow("Invalid Iconify utility prefix");
    expect(() => iconify().css(context({ scale: "0" }))).toThrow("positive number");
    expect(() => iconify().css(context({ mode: "outline" }))).toThrow("Iconify mode");
    expect(() => iconify().css(context({ syntax: "plain" }))).toThrow("Iconify syntax");
  });
});
