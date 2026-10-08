import { dirname, join, resolve } from "pathe";

/**
 * Resolves browser runtime modules or CSS from an immutable virtual filesystem.
 * @param files - Absolute paths to text, including package.json files.
 * @param specifier - Authored request. Relative and package subpath imports are accepted.
 * @param importer - Absolute importing file path.
 * @param kind - Source resolution selects browser/import exports; style resolution also selects style exports.
 * @returns An existing virtual path, or undefined when the request cannot be resolved.
 */
export function resolveVirtualModule(
  files: ReadonlyMap<string, string>,
  specifier: string,
  importer: string,
  kind: "source" | "style" = "source",
): string | undefined {
  const find = (name: string): string | undefined => {
    if (files.has(name)) {
      return name;
    }
    if (/\.[cm]?jsx?$/.test(name)) {
      const base = name.replace(/\.[cm]?jsx?$/, "");
      for (const extension of [".ts", ".tsx", ".mts", ".cts"]) {
        if (files.has(base + extension)) {
          return base + extension;
        }
      }
    }
    const extensions =
      kind === "style" ? [".css"] : [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".json"];
    for (const extension of extensions) {
      if (files.has(name + extension)) {
        return name + extension;
      }
    }
    for (const extension of extensions) {
      if (files.has(join(name, "index" + extension))) {
        return join(name, "index" + extension);
      }
    }
    return undefined;
  };

  if (specifier.startsWith(".") || specifier.startsWith("/")) {
    return find(resolve(dirname(importer), specifier));
  }

  const parts = specifier.split("/");
  const packageName = parts.splice(0, specifier.startsWith("@") ? 2 : 1).join("/");
  const subpath = parts.length ? "./" + parts.join("/") : ".";

  let directory = dirname(importer);
  while (true) {
    const base = join(directory, "node_modules", packageName);
    const text = files.get(join(base, "package.json"));
    if (text) {
      const metadata = JSON.parse(text) as Record<string, unknown>;
      const choose = (entry: unknown): string | undefined => {
        if (typeof entry === "string") {
          return entry;
        }
        if (Array.isArray(entry)) {
          for (const value of entry) {
            const result = choose(value);
            if (result) {
              return result;
            }
          }
        } else if (entry && typeof entry === "object") {
          const conditions = entry as Record<string, unknown>;
          const active = new Set(
            kind === "style"
              ? ["style", "browser", "import", "default"]
              : ["browser", "import", "default"],
          );
          for (const [condition, target] of Object.entries(conditions)) {
            if (!active.has(condition)) {
              continue;
            }
            const result = choose(target);
            if (result) {
              return result;
            }
          }
        }
        return undefined;
      };
      const exports = metadata.exports;
      const subpaths =
        exports && typeof exports === "object" && !Array.isArray(exports)
          ? (exports as Record<string, unknown>)
          : undefined;
      const pattern =
        subpaths && !(subpath in subpaths)
          ? Object.keys(subpaths)
              .filter(
                (key) =>
                  key.includes("*") &&
                  subpath.startsWith(key.split("*", 1)[0]) &&
                  subpath.endsWith(key.split("*", 2)[1]),
              )
              .sort((a, b) => b.indexOf("*") - a.indexOf("*") || b.length - a.length)[0]
          : undefined;
      const target = choose(
        exports &&
          typeof exports === "object" &&
          !Array.isArray(exports) &&
          Object.keys(exports).some((key) => key.startsWith("."))
          ? (subpaths![subpath] ?? (pattern ? subpaths![pattern] : undefined))
          : subpath === "."
            ? exports
            : undefined,
      );
      if (target) {
        const expanded = pattern
          ? target.replaceAll(
              "*",
              subpath.slice(pattern.indexOf("*"), subpath.length - pattern.split("*", 2)[1].length),
            )
          : target;
        if (!expanded.startsWith("./") || expanded.split("/").includes("..")) {
          return undefined;
        }
        return find(resolve(base, expanded));
      }
      if (exports !== undefined) {
        return undefined;
      }
      if (parts.length) {
        return find(join(base, ...parts));
      }
      return find(
        resolve(
          base,
          choose(kind === "style" ? metadata.style : metadata.browser) ??
            (typeof metadata.module === "string"
              ? metadata.module
              : typeof metadata.main === "string"
                ? metadata.main
                : "index"),
        ),
      );
    }

    if (directory === "/") {
      break;
    }

    directory = dirname(directory);
  }

  return undefined;
}
