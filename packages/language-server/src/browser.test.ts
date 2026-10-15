import { isBuiltin } from "node:module";
import { fileURLToPath } from "node:url";

import { rolldown } from "rolldown";
import { expect, test } from "vite-plus/test";

test("browser server and memory analysis bundle without Node runtime or CLI dependencies", async () => {
  const entry = fileURLToPath(new URL("./web-extension.ts", import.meta.url));
  const bundle = await rolldown({
    input: entry,
    platform: "browser",
    onLog(level, log, defaultHandler) {
      if (level === "warn") {
        expect.fail(log.message);
      }
      defaultHandler(level, log);
    },
    plugins: [
      {
        name: "browser-server-entry",
        resolveId(id) {
          if (id === entry) {
            return id;
          }
          return null;
        },
        load(id) {
          if (id === entry) {
            return 'export {startBrowserLanguageServer} from "@stillwind/language-server/browser";export {createMemoryAnalysisWorkspace} from "@stillwind/analysis/memory";export {defineConfig} from "stillwind/config";';
          }
          return null;
        },
      },
    ],
  });
  try {
    const result = await bundle.generate({ format: "esm" });
    const chunks = result.output.filter((output) => output.type === "chunk");
    expect(chunks.some((chunk) => chunk.code.includes("startBrowserLanguageServer"))).toBe(true);
    expect(
      chunks.flatMap((chunk) => [...chunk.imports, ...chunk.dynamicImports]).filter(isBuiltin),
    ).toEqual([]);
    expect(
      chunks
        .flatMap((chunk) => chunk.moduleIds)
        .some((file) => {
          file = file.replaceAll("\\", "/");
          return (
            file.includes("packages/cli/") ||
            file.includes("typescript-7/unstable/") ||
            file.includes("worker-client")
          );
        }),
    ).toBe(false);
  } finally {
    await bundle.close();
  }
});
