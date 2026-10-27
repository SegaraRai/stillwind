import type { PluginContext, PluginWatchKind } from "@stillwind/config";
import { describe, expect, test } from "vite-plus/test";

import { svgDirectory, type SvgDirectoryOptions } from "./directory.ts";
import { iconify } from "./plugin.ts";

const square =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="M0 0h24v24H0z"/></svg>';
const wide =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 24" fill="#f00"><path d="M0 0h48v24H0z"/></svg>';

/**
 * Creates a statement context over in-memory files.
 * @param files - Absolute file paths and their text. Parent directories are inferred.
 * @returns The context and the paths that the plugin watches.
 */
function project(files: Record<string, string>) {
  const watched: [string, PluginWatchKind][] = [];
  const context: PluginContext = {
    options: {},
    root: "/project",
    watch: (path, kind = "file") => {
      watched.push([path, kind]);
    },
    resolve: async () => undefined,
    read: async (path) => files[path] ?? expect.fail(`unexpected read of ${path}`),
    list: async (path) => {
      const entries = new Map<string, boolean>();
      for (const file of Object.keys(files)) {
        if (file.startsWith(`${path}/`)) {
          const rest = file.slice(path.length + 1);
          entries.set(rest.split("/", 1)[0], rest.includes("/"));
        }
      }
      if (!entries.size) {
        throw new Error(`${path} does not exist`);
      }
      return [...entries].map(([name, directory]) => ({
        name,
        directory,
      }));
    },
  };
  return {
    context,
    watched,
  };
}

/**
 * Loads a directory icon set through the plugin and returns the set's data.
 * @param context - Statement context.
 * @param directory - Directory argument of `svgDirectory`.
 * @param options - Options of `svgDirectory`.
 * @returns The icon set that the source produces.
 */
async function load(context: PluginContext, directory: string, options?: SvgDirectoryOptions) {
  const source = svgDirectory(directory, options);
  if (typeof source !== "function") {
    expect.fail("svgDirectory must return a function source");
  }
  return source({
    ...context,
    prefix: "brand",
  });
}

describe("svgDirectory", () => {
  test("creates an icon for each SVG file and watches the directory and the files", async () => {
    const { context, watched } = project({
      "/project/icons/logo.svg": square,
      "/project/icons/banner.svg": wide,
      "/project/icons/notes.txt": "",
      "/project/icons/nested/arrow.svg": square,
    });
    const data = await load(context, "./icons");

    expect(data.prefix).toBe("brand");
    expect(Object.keys(data.icons)).toEqual(["banner", "logo"]);
    expect(data.icons.logo).toEqual({
      left: 0,
      top: 0,
      width: 24,
      height: 24,
      body: '<path fill="currentColor" d="M0 0h24v24H0z"/>',
    });
    expect(data.icons.banner).toMatchObject({
      width: 48,
      height: 24,
    });
    expect(data.icons.banner.body).toContain('fill="#f00"');
    expect(watched).toEqual([
      ["/project/icons", "directory"],
      ["/project/icons/banner.svg", "file"],
      ["/project/icons/logo.svg", "file"],
    ]);
  });

  test("reads subdirectories when recursive, and applies name and transform", async () => {
    const { context, watched } = project({
      "/project/icons/logo.svg": square,
      "/project/icons/arrows/left.svg": square,
      "/project/icons/arrows/Draft.svg": square,
    });

    const absolute = await load(context, "/project/icons/arrows", {
      name: (path) => path.toLowerCase(),
    });
    expect(Object.keys(absolute.icons)).toEqual(["draft", "left"]);
    watched.length = 0;
    const data = await load(context, "icons", {
      recursive: true,
      name: (path) => (path.endsWith("Draft") ? undefined : path.replace("arrows/", "arrow-")),
      transform: (svg, name) => svg.replace("currentColor", name === "logo" ? "#00f" : "#0f0"),
    });

    expect(Object.keys(data.icons).sort()).toEqual(["arrow-left", "logo"]);
    expect(data.icons.logo.body).toContain('fill="#00f"');
    expect(data.icons["arrow-left"].body).toContain('fill="#0f0"');
    expect(watched).toContainEqual(["/project/icons/arrows", "directory"]);
    expect(watched).not.toContainEqual(["/project/icons/arrows/Draft.svg", "file"]);
  });

  test("rejects invalid and duplicate icon names and SVG without a viewBox", async () => {
    await expect(
      load(
        project({
          "/project/icons/Logo.svg": square,
        }).context,
        "icons",
      ),
    ).rejects.toThrow('/project/icons/Logo.svg has the invalid icon name "Logo"');
    await expect(
      load(
        project({
          "/project/icons/a-b.svg": square,
          "/project/icons/a/b.svg": square,
        }).context,
        "icons",
        {
          recursive: true,
        },
      ),
    ).rejects.toThrow(
      'SVG icons /project/icons/a/b.svg and /project/icons/a-b.svg have the same icon name "a-b"',
    );
    await expect(
      load(
        project({
          "/project/icons/logo.svg": '<svg width="24" height="24"></svg>',
        }).context,
        "icons",
      ),
    ).rejects.toThrow("/project/icons/logo.svg must be an <svg> element with a viewBox attribute");
    await expect(load(project({}).context, "icons")).rejects.toThrow(
      "/project/icons does not exist",
    );
  });

  test("defines utilities for the icons of a configured directory", async () => {
    const { context } = project({
      "/project/icons/logo.svg": square,
    });
    const plugin = iconify({
      collections: {
        brand: svgDirectory("icons"),
      },
    });

    expect(await plugin.resolve!(["icon-brand--logo"], context)).toContain(
      "@utility icon-brand--logo (implementation) {",
    );
    expect(await plugin.catalog!(context, "icon-brand--")).toEqual(["icon-brand--logo"]);
  });
});
