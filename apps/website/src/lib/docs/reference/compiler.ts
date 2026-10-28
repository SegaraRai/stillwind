import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { loadCssGraph } from "@stillwind/host";
import {
  NativeProjectSession,
  type NativeCompilerOptions,
  type NativePreparedSource,
} from "@stillwind/native";

const require = createRequire(import.meta.url);

export async function referenceCompiler(
  options: NativeCompilerOptions = {},
  stylesheet = '@import "stillwind";',
) {
  const { loader } = await loadCssGraph(
    resolve(import.meta.dirname, "Reference.css"),
    options,
    {
      resolve: async (specifier, importer) =>
        specifier.startsWith(".")
          ? resolve(dirname(importer), specifier)
          : require.resolve(specifier),
      read: (id) => readFile(id, "utf8"),
      documentUri: (id) => pathToFileURL(id).href,
      sourceMapSource: (id) => id,
      lint: (id) => id.endsWith("Reference.css"),
      watch() {},
    },
    stylesheet,
  );
  const source = (code: string): NativePreparedSource => ({
    kind: "direct",
    moduleId: "Reference.tsx",
    document: { uri: "file:///Reference.tsx", format: "jsx", code },
    syntax: { language: "tsx", moduleKind: "module" },
  });
  return {
    loader,
    query: loader.editorQuery(),
    compile(code: string) {
      return new NativeProjectSession(loader, false).compile([source(code)]).modules[0]!;
    },
    /** Compiles `code` when present and renders the complete stylesheet of the session. */
    document(code?: string) {
      const session = new NativeProjectSession(loader, false);
      const module = code === undefined ? undefined : session.compile([source(code)]).modules[0]!;
      return {
        module,
        css: session.css().css,
      };
    },
  };
}

let shared: ReturnType<typeof referenceCompiler> | undefined;

export const documentationCompiler = () => (shared ??= referenceCompiler());

export async function compilerRevision() {
  const compiler = await documentationCompiler();
  const hash = createHash("sha256").update(compiler.query.revision());
  const binaries = Object.keys(require.cache)
    .filter((path) => /stillwind_native.*\.node$/.test(path))
    .sort();
  for (const path of [import.meta.filename, require.resolve("@stillwind/native"), ...binaries]) {
    hash.update(await readFile(path));
  }
  return hash.digest("hex");
}
