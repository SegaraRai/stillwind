import { join } from "pathe";

/**
 * Adds compiler libraries and fallback Stillwind packages to a captured workspace.
 * @param files - Mutable inventory using decoded virtual paths.
 * @param libraries - Static standard libraries and package files from the extension bundle.
 * @param roots - Workspace roots where absent packages are installed in the virtual inventory.
 * Existing package.json files retain the workspace's selected package version.
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
