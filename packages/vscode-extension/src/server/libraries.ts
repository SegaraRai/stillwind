import { join } from "pathe";

/**
 * Adds compiler libraries and fallback Stillwind packages to a captured workspace.
 * @param files - Mutable inventory using decoded virtual paths.
 * @param libraries - Virtual path/text pairs from the extension bundle. Standard libraries replace
 * existing entries. Files under /__stillwind/packages/ provide fallback package contents.
 * @param roots - Workspace roots where absent packages are installed in the virtual inventory.
 * A package.json already present under a root skips that package's entire fallback inventory.
 */
export function installLibraries(
  files: Map<string, string>,
  libraries: readonly (readonly [string, string])[],
  roots: readonly string[],
): void {
  for (const [id, text] of libraries) {
    if (!id.startsWith("/__stillwind/packages/")) {
      files.set(id, text);
    }
  }
  for (const root of roots) {
    for (const name of ["stillwind", "@stillwind/config"]) {
      const directory = join(root, "node_modules", name);
      if (files.has(join(directory, "package.json"))) {
        continue;
      }
      const prefix = `/__stillwind/packages/${name}/`;
      for (const [id, text] of libraries) {
        if (id.startsWith(prefix)) {
          files.set(join(directory, id.slice(prefix.length)), text);
        }
      }
    }
  }
}
