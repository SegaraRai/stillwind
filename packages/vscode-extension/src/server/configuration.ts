import { rolldown } from "@rolldown/browser";
import { resolveVirtualModule } from "@stillwind/host/resolution";

/**
 * Bundles a browser configuration from a captured workspace without reading the host filesystem.
 * @param files - Virtual absolute paths and source text, including browser-compatible packages.
 * @param entry - Configuration path present in the inventory.
 * @returns One ESM source whose default export can be evaluated through a Blob URL.
 * @throws If an import is unavailable, uses a Node builtin, or fails compilation.
 */
export async function bundleConfiguration(
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
        name: "virtual-configuration",
        resolveId(id, importer) {
          if (id.startsWith("node:")) {
            throw new Error(`Node module is unavailable in browser configuration: ${id}`);
          }
          const file = importer ? resolveVirtualModule(files, id, importer) : id;
          if (!file || !files.has(file)) {
            throw new Error(`Cannot resolve configuration import ${id}`);
          }
          return file;
        },
        load(id) {
          const code = files.get(id);
          if (code === undefined) {
            throw new Error(`Missing configuration source ${id}`);
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
      throw new Error(`Configuration bundle has no entry: ${entry}`);
    }
    return chunk.code;
  } finally {
    await bundle.close();
  }
}
