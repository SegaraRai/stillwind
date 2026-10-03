import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { build } from "vite-plus";
import { expect, test } from "vite-plus/test";

test("a production browser build includes WASI and its worker assets", async () => {
  const temporary = await mkdtemp(path.join(tmpdir(), "stillwind-wasi-bundle-"));
  try {
    const modules = path.join(temporary, "node_modules/@stillwind");
    await mkdir(modules, { recursive: true });
    await symlink(
      path.resolve(import.meta.dirname, ".."),
      path.join(modules, "native"),
      process.platform === "win32" ? "junction" : "dir",
    );
    await writeFile(
      path.join(temporary, "index.html"),
      '<!doctype html><script type="module" src="/main.js"></script>',
    );
    await writeFile(
      path.join(temporary, "main.js"),
      "import {minifyCss} from '@stillwind/native'; document.body.textContent = minifyCss('.a { color: red }').css;",
    );
    const result = await build({
      configFile: false,
      root: temporary,
      logLevel: "silent",
      build: { target: "esnext", write: false },
    });
    if (!Array.isArray(result) && !("output" in result)) {
      expect.fail("expected a completed production build");
    }
    const outputs = (Array.isArray(result) ? result : [result]).flatMap((bundle) => bundle.output);
    expect(outputs.some((output) => output.fileName.endsWith(".wasm"))).toBe(true);
    expect(outputs.some((output) => /wasi-worker.*\.js$/.test(output.fileName))).toBe(true);
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});
