import type { IconifyJSON } from "@iconify/types";
import type { PluginContext } from "@stillwind/config";
import { describe, expect, test } from "vite-plus/test";

import { iconify, type IconifyOptions } from "./plugin.ts";

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
    list: async (path) => {
      throw new Error(`${path} does not exist`);
    },
  };
}

/**
 * Decodes the SVG of each icon asset in generated CSS.
 * @param css - CSS that `resolve` returned.
 * @returns SVG text of each asset declaration in order.
 */
function assetSvgs(css: string): string[] {
  return [...css.matchAll(/--sw-icon-svg-[0-9a-z]+: url\("data:image\/svg\+xml,([^"]*)"\)/g)].map(
    (match) => decodeURIComponent(match[1]),
  );
}

describe("iconify", () => {
  test("equal SVGs with different attribution retain separate asset identities", async () => {
    const source = (name: string): IconifyJSON => ({
      ...collection,
      info: {
        name,
        author: {
          name: "Author",
        },
        license: {
          title: "MIT",
        },
      },
    });
    const plugin = iconify({
      collections: {
        first: source("First Set"),
        second: source("Second Set"),
      },
    });
    const css = await plugin.resolve!(["icon-first--close", "icon-second--close"], context());
    const assets = [...css.matchAll(/--sw-icon-svg-([a-z0-9]+):/g)].map((match) => match[1]);
    expect(new Set(assets).size).toBe(2);
    expect(css).toContain("/*! First Set | Author | MIT */");
    expect(css).toContain("/*! Second Set | Author | MIT */");
  });

  test("shared asset names follow SVG content across aliases and instance settings", async () => {
    const plugin = iconify({
      collections: {
        demo: collection,
      },
    });
    const css = await plugin.resolve!(["icon-demo--close", "icon-demo--cross"], context());
    const names = (input: string) => [...new Set(input.match(/--sw-icon-svg-[0-9a-z]+(?=:)/g))];
    expect(css).toContain("@theme default asset {");
    expect(names(css)).toHaveLength(1);
    expect(names(css)[0]).toMatch(/^--sw-icon-svg-[0-9a-z]{4}$/);
    const alternate = await plugin.resolve!(
      ["i-demo--close"],
      context({
        prefix: "i",
        mode: "background",
        scale: "2",
      }),
    );
    expect(names(alternate)).toEqual(names(css));
    const changed = iconify({
      collections: {
        demo: {
          ...collection,
          icons: {
            close: {
              body: '<path fill="red" d="M0 0h12v12z"/>',
            },
          },
        },
      },
    });
    expect(names(await changed.resolve!(["icon-demo--close"], context()))).not.toEqual(names(css));
  });

  test("uses a fixed hash length and lets stylesheet options override TypeScript defaults", async () => {
    const plugin = iconify({
      hashLength: 6,
      collections: {
        demo: collection,
      },
    });
    const suffix = async (options: Record<string, string> = {}) => {
      const css = await plugin.resolve!(["icon-demo--close", "icon-demo--flag"], context(options));
      return [...css.matchAll(/--sw-icon-svg-([0-9a-z]+):/g)].map((match) => match[1]);
    };
    expect(await suffix()).toEqual([
      expect.stringMatching(/^[0-9a-z]{6}$/),
      expect.stringMatching(/^[0-9a-z]{6}$/),
    ]);
    for (const length of [1, 4, 7]) {
      const hashes = await suffix({ "hash-length": String(length) });
      expect(hashes).toHaveLength(2);
      for (const hash of hashes) {
        expect(hash).toHaveLength(length);
      }
    }
    expect(
      await suffix({
        "hash-length": "7",
      }),
    ).toContainEqual(expect.stringMatching(/^0/));
  });
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

  test("bracket candidates define their bracket names and share the named asset", async () => {
    const plugin = iconify({
      syntax: "bracket",
      collections: {
        demo: collection,
      },
    });
    const css = await plugin.resolve!(["icon-[demo--close]", "icon-demo--flag"], context());

    expect(css).toContain("@utility icon-[demo--close] (implementation) {");
    expect(css).toMatch(/mask-image: var\(--sw-icon-svg-[0-9a-z]+\);/);
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
    expect(css).toMatch(/--sw-icon-svg-[0-9a-z]+: url\("data:image\/svg\+xml,/);
    expect(css).toMatch(/mask-image: var\(--sw-icon-svg-[0-9a-z]+\);/);
    expect(css).toContain("@utility icon-demo--flag (implementation) {");
    expect(css).toMatch(/background-image: var\(--sw-icon-svg-[0-9a-z]+\);/);
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
    expect(css).toMatch(/background-image: var\(--sw-icon-svg-[0-9a-z]+\);/);
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
      [
        "/project/node_modules/@iconify-json/demo/info.json",
        JSON.stringify({
          name: "Installed Set",
          author: {
            name: "Author",
          },
          license: {
            title: "MIT",
          },
        }),
      ],
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
      list: async (path) => expect.fail(`unexpected list of ${path}`),
    };
    const plugin = iconify();

    expect(await plugin.resolve!(["icon-demo--close"], host)).toContain(
      "@utility icon-demo--close",
    );
    expect(await plugin.catalog!(host, "")).toContain("icon-demo--flag");
    expect(watched).toContain("/project/node_modules/@iconify-json/demo/icons.json");
    expect(watched).toContain("/project/node_modules/@iconify-json/demo/info.json");
    expect(await plugin.resolve!(["icon-demo--close"], host)).toContain(
      "/*! Installed Set | Author | MIT */",
    );
    files.set(
      "/project/node_modules/@iconify-json/demo/info.json",
      JSON.stringify({
        name: "Installed Set",
        author: {
          name: "Author",
        },
        license: {
          title: "Apache-2.0",
        },
      }),
    );
    expect(await plugin.resolve!(["icon-demo--close"], { ...host })).toContain(
      "/*! Installed Set | Author | Apache-2.0 */",
    );
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
      list: async (path) => expect.fail(`unexpected list of ${path}`),
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

  test("minifies icon SVG with SVGO unless svgo is false, and accepts an SVGO configuration", async () => {
    const verbose: IconifyJSON = {
      prefix: "demo",
      icons: {
        mark: {
          body: '<!-- mark --><g><path fill="currentColor" d="M 0 0 L 16 0 L 16 16 Z"/></g>',
        },
      },
    };
    const svg = async (svgo?: IconifyOptions["svgo"]) => {
      const css = await iconify({ svgo, collections: { demo: verbose } }).resolve!(
        ["icon-demo--mark"],
        context(),
      );
      expect(css).toMatch(/mask-image: var\(--sw-icon-svg-[0-9a-z]+\);/);
      return assetSvgs(css)[0];
    };

    const original = await svg(false);
    expect(original).toContain("<!-- mark -->");
    expect(original).toContain("<g>");
    const minified = await svg();
    expect(minified).not.toContain("<!--");
    expect(minified).not.toContain("<g>");
    expect(minified).toContain("viewBox='0 0 16 16'");
    expect(minified.length).toBeLessThan(original.length);
    expect(await svg(true)).toBe(minified);
    const configured = await svg({ plugins: ["removeComments"] });
    expect(configured).not.toContain("<!--");
    expect(configured).toContain("<g>");
  });

  test("rejects invalid options", async () => {
    expect(() => iconify({ prefix: "Icon" })).toThrow("Invalid Iconify utility prefix");
    expect(() => iconify().css(context({ scale: "0" }))).toThrow("positive number");
    expect(() => iconify().css(context({ mode: "outline" }))).toThrow("Iconify mode");
    expect(() => iconify().css(context({ syntax: "plain" }))).toThrow("Iconify syntax");
    expect(() => iconify({ svgo: "yes" as never })).toThrow("Iconify svgo");
    expect(() => iconify({ svgo: null as never })).toThrow("Iconify svgo");
    for (const hashLength of [0, -1, 8, 1.5, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(() => iconify({ hashLength })).toThrow("Iconify hash length");
      expect(() => iconify().css(context({ "hash-length": String(hashLength) }))).toThrow(
        "Iconify hash length",
      );
    }
  });
});
