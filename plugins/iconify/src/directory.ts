import type { IconifyIcon, IconifyJSON } from "@iconify/types";
import { convertParsedSVG, parseSVGContent } from "@iconify/utils/lib/svg/parse";

import type { CollectionLoadContext, CollectionSource } from "./collections.ts";
import { ICONIFY_NAME } from "./plugin.ts";

/** Options of {@link svgDirectory}. */
export interface SvgDirectoryOptions {
  /**
   * Reads SVG files in subdirectories. The default icon name joins the path segments with `-`,
   * such as `arrows-left` for `arrows/left.svg`. Defaults to `false`.
   */
  readonly recursive?: boolean;
  /**
   * Maps a file to an icon name. The function receives the file path relative to the directory,
   * with `/` separators and without the `.svg` extension, such as `arrows/left`. Returning
   * undefined skips the file. By default, the path segments are joined with `-`.
   */
  readonly name?: (path: string) => string | undefined;
  /** Rewrites the SVG text of each file before the plugin parses it. */
  readonly transform?: (svg: string, name: string) => string | Promise<string>;
}

/**
 * Creates an icon set from the SVG files in a directory. Each file becomes one icon, and its
 * `viewBox` sets the icon's dimensions. The set watches the directory and its files, so adding,
 * removing, or editing an SVG file reloads the design system.
 *
 * @param directory - Absolute directory path, or a path relative to the project directory.
 * @param options - Subdirectory reading, icon names, and SVG rewriting.
 * @returns A source for the `collections` option of `iconify`.
 */
export function svgDirectory(
  directory: string,
  options: SvgDirectoryOptions = {},
): CollectionSource {
  return async (context) => {
    const files = await svgFiles(context, absolute(context.root, directory), "", options);
    const icons: Record<string, IconifyIcon> = {};
    const owners = new Map<string, string>();
    for (const { file, path } of files) {
      const name = options.name ? options.name(path) : path.replaceAll("/", "-");
      if (name === undefined) {
        continue;
      }
      if (!ICONIFY_NAME.test(name)) {
        throw new Error(
          `SVG icon ${file} has the invalid icon name "${name}". Icon names must consist of lowercase letters and digits separated by single hyphens. Use the name option to rename the file's icon.`,
        );
      }
      const owner = owners.get(name);
      if (owner !== undefined) {
        throw new Error(`SVG icons ${owner} and ${file} have the same icon name "${name}"`);
      }
      owners.set(name, file);
      context.watch(file);
      let svg = await context.read(file);
      if (options.transform) {
        svg = await options.transform(svg, name);
      }
      const parsed = parseSVGContent(svg);
      const icon = parsed && convertParsedSVG(parsed);
      if (!icon) {
        throw new Error(`SVG icon ${file} must be an <svg> element with a viewBox attribute`);
      }
      icons[name] = icon;
    }
    return {
      prefix: context.prefix,
      icons,
    } satisfies IconifyJSON;
  };
}

/**
 * Lists the SVG files of a directory and watches each directory that it lists.
 *
 * @param context - Host operations of the icon set.
 * @param directory - Absolute directory path.
 * @param prefix - Relative path of `directory` with a trailing `/`, or an empty string for the
 * top directory.
 * @param options - Whether subdirectories are read.
 * @returns Files in name order, with their relative paths without the `.svg` extension.
 * @throws If a directory cannot be listed.
 */
async function svgFiles(
  context: CollectionLoadContext,
  directory: string,
  prefix: string,
  options: SvgDirectoryOptions,
): Promise<
  {
    file: string;
    path: string;
  }[]
> {
  context.watch(directory, "directory");
  const entries = [...(await context.list(directory))].sort((a, b) =>
    a.name < b.name ? -1 : a.name > b.name ? 1 : 0,
  );
  const files: {
    file: string;
    path: string;
  }[] = [];
  for (const entry of entries) {
    const file = join(directory, entry.name);
    if (entry.directory) {
      if (options.recursive) {
        files.push(...(await svgFiles(context, file, `${prefix}${entry.name}/`, options)));
      }
    } else if (/\.svg$/i.test(entry.name)) {
      files.push({
        file,
        path: prefix + entry.name.slice(0, -4),
      });
    }
  }
  return files;
}

/**
 * Resolves a directory against the project directory. The result keeps the separator style of
 * the project directory, because hosts compare watched paths with the paths that their file
 * watchers report.
 *
 * @param root - Absolute project directory.
 * @param directory - Absolute path, or a path relative to `root`.
 * @returns The absolute directory path.
 */
function absolute(root: string, directory: string): string {
  if (/^(?:[A-Za-z]:)?[\\/]/.test(directory)) {
    return directory;
  }
  let path = root;
  for (const segment of directory.split(/[\\/]/)) {
    if (segment === "..") {
      path = path.replace(/[\\/][^\\/]*$/, "");
    } else if (segment && segment !== ".") {
      path = join(path, segment);
    }
  }
  return path;
}

/**
 * Appends one entry name to a directory path with the directory's separator style.
 *
 * @param directory - Absolute directory path.
 * @param name - Entry name without separators.
 * @returns The entry path.
 */
function join(directory: string, name: string): string {
  const separator = directory.includes("\\") ? "\\" : "/";
  return directory.endsWith(separator) ? directory + name : directory + separator + name;
}
