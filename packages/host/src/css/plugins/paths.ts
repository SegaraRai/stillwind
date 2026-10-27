import type { PluginDirectoryEntry } from "@stillwind/config";
import { fingerprint, stylesheetPluginPaths } from "@stillwind/native";
import { dirname, isAbsolute, join, relative, resolve } from "pathe";

import type { EditorRequest, EditorResult } from "../../editor/request.ts";

/**
 * A complete module specifier that an editor can insert into a CSS string.
 */
export interface PluginPathCompletion {
  specifier: string;
  kind: "module" | "file" | "directory";
}

/**
 * Lists immediate virtual children without reading the physical filesystem.
 *
 * @param files - Absolute file identities, including unsaved documents.
 * @param directory - Absolute directory whose immediate children are requested.
 * @param caseSensitive - Whether directory identity is case sensitive. Defaults to true.
 * @returns Files and inferred parent directories, each listed once.
 */
export function pluginDirectory(
  files: ReadonlyMap<string, unknown>,
  directory: string,
  caseSensitive = true,
): PluginDirectoryEntry[] {
  const prefix = directory.replaceAll("\\", "/").replace(/\/$/, "") + "/";
  const key = (text: string) => (caseSensitive ? text : text.toLowerCase());
  const entries = new Map<string, PluginDirectoryEntry>();
  for (const file of files.keys()) {
    const path = file.replaceAll("\\", "/");
    if (!key(path).startsWith(key(prefix))) {
      continue;
    }
    const tail = path.slice(prefix.length);
    const name = tail.split("/", 1)[0];
    if (name) {
      entries.set(key(name), {
        name,
        directory: tail.includes("/"),
      });
    }
  }
  return [...entries.values()];
}

/**
 * Completes local script files, directory prefixes, and package dependencies.
 *
 * @param prefix - Decoded specifier before the CSS cursor.
 * @param importer - Absolute stylesheet identity.
 * @param root - Project directory whose package manifest supplies package names.
 * @param host - Text reads and immediate directory entries from the same captured host.
 * @returns Complete specifiers in lexical order. Missing files and malformed manifests omit suggestions.
 */
export async function completePluginPaths(
  prefix: string,
  importer: string,
  root: string,
  host: {
    read(id: string): Promise<string>;
    directory(id: string): Promise<readonly PluginDirectoryEntry[]>;
  },
): Promise<PluginPathCompletion[]> {
  const entries = new Map<string, PluginPathCompletion>();
  if (!prefix || prefix.startsWith(".") || prefix.startsWith("/") || /^[A-Za-z]:\//.test(prefix)) {
    const slash = prefix.lastIndexOf("/");
    const before = slash < 0 ? "./" : prefix.slice(0, slash + 1);
    const partial = slash < 0 ? prefix : prefix.slice(slash + 1);
    const children = await host.directory(resolve(dirname(importer), before)).catch(() => []);
    for (const child of children) {
      if (
        !child.name.startsWith(partial) ||
        child.name === ".git" ||
        (!child.directory &&
          (!/\.[cm]?[jt]s$/i.test(child.name) || /\.d\.[cm]?ts$/i.test(child.name)))
      ) {
        continue;
      }
      const specifier = before + child.name + (child.directory ? "/" : "");
      entries.set(specifier, {
        specifier,
        kind: child.directory ? "directory" : "file",
      });
    }
  }
  if (!prefix.startsWith(".") && !prefix.startsWith("/") && !/^[A-Za-z]:/.test(prefix)) {
    const packages = new Set<string>();
    for (const directory of new Set([dirname(importer), root])) {
      const metadata: unknown = await host
        .read(join(directory, "package.json"))
        .then(JSON.parse)
        .catch(() => undefined);
      if (!metadata || typeof metadata !== "object") {
        continue;
      }
      for (const key of [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
      ]) {
        const dependencies = (metadata as Record<string, unknown>)[key];
        if (!dependencies || typeof dependencies !== "object" || Array.isArray(dependencies)) {
          continue;
        }
        for (const specifier of Object.keys(dependencies)) {
          packages.add(specifier);
        }
      }
    }
    for (const specifier of packages) {
      if (specifier.startsWith(prefix)) {
        entries.set(specifier, {
          specifier,
          kind: "module",
        });
      }
      if (!prefix.startsWith(specifier + "/")) {
        continue;
      }
      let base = dirname(importer);
      while (true) {
        const metadata: unknown = await host
          .read(join(base, "node_modules", specifier, "package.json"))
          .then(JSON.parse)
          .catch(() => undefined);
        if (metadata && typeof metadata === "object") {
          const exports = (metadata as Record<string, unknown>).exports;
          if (exports && typeof exports === "object" && !Array.isArray(exports)) {
            for (const [subpath, target] of Object.entries(exports)) {
              const name = specifier + subpath.slice(1);
              if (
                target !== null &&
                subpath.startsWith("./") &&
                !subpath.includes("*") &&
                name.startsWith(prefix)
              ) {
                entries.set(name, {
                  specifier: name,
                  kind: "module",
                });
              }
            }
          }
          break;
        }
        const parent = dirname(base);
        if (parent === base) {
          break;
        }
        base = parent;
      }
    }
  }
  return [...entries.values()].sort((a, b) =>
    a.specifier < b.specifier ? -1 : a.specifier > b.specifier ? 1 : 0,
  );
}

/**
 * Answers plugin path completion and links without loading or evaluating plugins.
 *
 * @param id - Absolute stylesheet identity inside the project root.
 * @param text - Captured original CSS, including incomplete strings.
 * @param request - Original UTF-16 editor operation.
 * @param options - Configured plugin names, project scope, resolver, and URI mapping.
 * @returns An answer for links or a selected plugin path, otherwise undefined.
 * Unresolved and configured-only names have links without targets. Their callers can suppress
 * ambiguous targets when overlapping projects disagree.
 */
export async function queryPluginPaths(
  id: string,
  text: string,
  request: EditorRequest,
  options: {
    root: string;
    names: readonly string[];
    host?: PluginPathHost;
    documentUri(id: string): string;
  },
): Promise<EditorResult | undefined> {
  if (request.kind !== "completion" && request.kind !== "resolve" && request.kind !== "links") {
    return undefined;
  }
  const name = relative(options.root, id);
  if (name === ".." || name.startsWith("../") || isAbsolute(name)) {
    return undefined;
  }
  const encoder = new TextEncoder();
  const bytes = encoder.encode(text);
  const utf16 = (offset: number) => new TextDecoder().decode(bytes.subarray(0, offset)).length;
  const offset =
    request.kind === "completion"
      ? request.offset
      : request.kind === "resolve"
        ? request.input
        : undefined;
  const paths = stylesheetPluginPaths(
    text,
    offset === undefined ? undefined : encoder.encode(text.slice(0, offset)).length,
  );
  const result: EditorResult = {
    revision: fingerprint(text),
  };
  const snapshot = fingerprint(JSON.stringify([text, options.root, options.names]));
  if (request.kind === "links") {
    result.links = await Promise.all(
      paths
        .filter((path) => path.closed)
        .map(async (path) => {
          const target = options.names.includes(path.specifier)
            ? undefined
            : await options.host?.resolve(path.specifier, id).catch(() => undefined);
          return {
            start: utf16(path.start),
            end: utf16(path.end),
            target: target ? options.documentUri(target) : undefined,
          };
        }),
    );
    return result;
  }
  const path = paths.find((path) => path.prefix != null);
  if (!path) {
    return undefined;
  }
  const prefix = path.prefix!;
  const available = new Map(
    ((await options.host?.complete?.(prefix, id, options.root).catch(() => [])) ?? []).map(
      (entry) => [entry.specifier, entry],
    ),
  );
  for (const specifier of options.names) {
    if (specifier.startsWith(prefix)) {
      available.set(specifier, {
        specifier,
        kind: "module",
      });
    }
  }
  const detail = (entry: PluginPathCompletion) =>
    options.names.includes(entry.specifier)
      ? "Configured plugin"
      : entry.kind === "directory"
        ? "Directory"
        : "Plugin module";
  if (request.kind === "resolve") {
    if (request.snapshot === snapshot && available.has(request.candidate)) {
      result.hover = {
        start: 0,
        end: 0,
        css: "",
        detail: detail(available.get(request.candidate)!),
        documentation: detail(available.get(request.candidate)!),
      };
    }
    return result;
  }
  result.completions = [...available.values()]
    .filter((entry) => entry.specifier.startsWith(prefix))
    .sort((a, b) => (a.specifier < b.specifier ? -1 : a.specifier > b.specifier ? 1 : 0))
    .map((entry) => ({
      start: utf16(path.start),
      end: utf16(path.end),
      label: entry.specifier,
      candidate: entry.specifier,
      input: request.offset,
      snapshot,
      newText:
        entry.specifier
          .replaceAll("\\", "\\\\")
          .replaceAll(path.quote, "\\" + path.quote)
          .replace(/[\n\r\f]/g, (character) => "\\" + character.charCodeAt(0).toString(16) + " ") +
        (path.closed ? "" : path.quote),
      detail: detail(entry),
      category: entry.kind,
      retrigger: entry.kind === "directory",
    }));
  return result;
}

/**
 * File resolution and completion used by plugin path navigation. No plugin evaluation is required.
 */
export interface PluginPathHost {
  /**
   * Lists complete specifiers for an editor without evaluating their modules.
   *
   * @param prefix - Decoded path text before the CSS cursor.
   * @param importer - Absolute identity of the stylesheet containing the path.
   * @param root - Absolute project directory used to find package metadata.
   * @returns Full specifiers with trailing slashes for directory prefixes.
   */
  complete?(
    prefix: string,
    importer: string,
    root: string,
  ): Promise<readonly PluginPathCompletion[]>;

  /**
   * Resolves a module request for plugin file reads and editor navigation without evaluating it.
   *
   * @param specifier - Package specifier with an optional subpath, or a relative or absolute path.
   * @param importer - Resolved identity of the stylesheet that contains the statement.
   * @returns The resolved path, or undefined when the request does not resolve.
   */
  resolve(specifier: string, importer: string): Promise<string | undefined>;
}
