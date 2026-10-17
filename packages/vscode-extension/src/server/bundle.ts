import { rolldown } from "@rolldown/browser";
import { resolveVirtualModule } from "@stillwind/host/resolution";

/**
 * Bundles a browser module, such as a configuration or a plugin package, from a captured
 * workspace without reading the host filesystem.
 * @param files - Virtual absolute paths and source text, including browser-compatible packages.
 * @param entry - Configuration path present in the inventory.
 * @returns One ESM source whose default export can be evaluated through a Blob URL.
 * @throws If an import is unavailable, uses a Node builtin, or fails compilation.
 */
export async function bundleModule(
  files: ReadonlyMap<string, string>,
  entry: string,
): Promise<string> {
  const bundle = await rolldown({
    input: entry,
    cwd: "/",
    platform: "browser",
    tsconfig: false,
    plugins: [
      {
        name: "virtual-workspace",
        resolveId(id, importer) {
          if (id.startsWith("node:")) {
            throw new Error(`Node module is unavailable in the browser: ${id}`);
          }
          const file = importer ? resolveVirtualModule(files, id, importer) : id;
          if (!file || !files.has(file)) {
            throw new Error(`Cannot resolve import ${id}`);
          }
          return file;
        },
        load(id) {
          const code = files.get(id);
          if (code === undefined) {
            throw new Error(`Missing module source ${id}`);
          }
          return {
            code,
            moduleType: id.endsWith(".json") ? "json" : "ts",
          };
        },
      },
    ],
  });
  try {
    const result = await bundle.generate({ format: "esm", codeSplitting: false });
    const chunk = result.output.find((output) => output.type === "chunk" && output.isEntry);
    if (chunk?.type !== "chunk") {
      throw new Error(`Module bundle has no entry: ${entry}`);
    }
    return chunk.code;
  } finally {
    await bundle.close();
  }
}
