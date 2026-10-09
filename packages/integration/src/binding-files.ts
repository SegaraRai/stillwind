import { existsSync, readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Returns existing runtime files from the generated loader and installed binding
 * packages. Watching both locations covers napi-rs's local and packaged loading.
 * Missing optional packages are skipped. Other filesystem and resolution errors
 * propagate to the caller.
 * @returns Distinct absolute runtime paths in discovery order, including the generated loader entry.
 * @throws If metadata, filesystem access, or resolution fails beyond a missing optional package.
 */
export function nativeBindingFiles(): string[] {
  const entry = fileURLToPath(import.meta.resolve("@stillwind/native"));
  const require = createRequire(entry);
  const files = new Set([entry]);
  /**
   * Adds native and WASM runtime companions from an installed runtime directory.
   * @param directory - Existing directory to enumerate. Matching filenames are added to files.
   * @throws If enumeration fails.
   */
  const collect = (directory: string) => {
    for (const name of readdirSync(directory)) {
      if (
        (name.startsWith("stillwind_native.") || name.startsWith("wasi-worker")) &&
        /\.(node|wasm|cjs|mjs|js)$/.test(name)
      ) {
        files.add(path.join(directory, name));
      }
    }
  };
  collect(path.dirname(entry));
  const manifest = JSON.parse(
    readFileSync(new URL(import.meta.resolve("@stillwind/native/package.json")), "utf8"),
  ) as {
    /** Optional binding package names mapped to their version requirements in generated metadata. */
    optionalDependencies: Record<string, string>;
  };
  for (const name of Object.keys(manifest.optionalDependencies)) {
    let runtime: string;
    try {
      runtime = require.resolve(name);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "MODULE_NOT_FOUND") {
        continue;
      }
      throw error;
    }
    if (existsSync(runtime)) {
      files.add(runtime);
      collect(path.dirname(runtime));
    }
  }
  return [...files];
}
