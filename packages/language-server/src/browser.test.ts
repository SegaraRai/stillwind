import { build } from "esbuild";
import { expect, test } from "vite-plus/test";

test("browser server and memory analysis bundle without Node runtime or CLI dependencies", async () => {
  const result = await build({
    stdin: {
      contents:
        'export {startBrowserLanguageServer} from "@stillwind/language-server/browser";export {createMemoryAnalysisWorkspace} from "@stillwind/analysis/memory";export {defineConfig} from "stillwind/config";',
      resolveDir: import.meta.dirname,
      sourcefile: "web-extension.ts",
    },
    bundle: true,
    platform: "browser",
    format: "esm",
    write: false,
    metafile: true,
  });
  expect(result.outputFiles[0].text).toContain("startBrowserLanguageServer");
  expect(
    Object.keys(result.metafile!.inputs).some(
      (file) =>
        file.includes("packages/cli/") ||
        file.includes("typescript-7/unstable/") ||
        file.includes("worker-client"),
    ),
  ).toBe(false);
});
