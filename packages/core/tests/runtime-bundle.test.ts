import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { build } from "vite-plus";
import { expect, test } from "vite-plus/test";

async function bundle(source: string): Promise<string> {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-runtime-bundle-"));
  try {
    const entry = path.join(root, "entry.js");
    await writeFile(entry, source);
    const result = await build({
      configFile: false,
      logLevel: "silent",
      build: {
        write: false,
        minify: false,
        lib: { entry, formats: ["es"] },
      },
    });
    const output = Array.isArray(result) ? result[0] : result;
    if (!("output" in output)) {
      expect.fail("Expected a finite bundle");
    }
    return output.output
      .filter((file) => file.type === "chunk")
      .map((file) => file.code)
      .join("\n");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("each packaged framework keeps only the requested style conversion", async () => {
  for (const framework of ["react", "preact", "solid", "vue", "svelte", "astro"]) {
    const code = await bundle(
      `export { stillwindStyleValue as convert } from "stillwind/runtime/${framework}";`,
    );
    const runtime = await import(
      `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
    );
    expect(runtime.convert({ color: "red" })).toEqual(
      ["svelte", "astro"].includes(framework) ? "color:red" : { color: "red" },
    );
    expect(code).not.toMatch(
      /mergeFragments|toFragment|stillwindClassKey|stillwindCustomPropertiesKey/,
    );
  }
});

test("packaged production merges exclude validation and unused conversion", async () => {
  const code = await bundle('export { stillwindMerge as merge } from "stillwind/runtime/react";');
  const runtime = await import(
    `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
  );
  expect(runtime.merge({ title: "ready" })).toEqual({ title: "ready" });
  expect(code).not.toMatch(/function toFragment\b|Checked|NODE_ENV|different runtime instance/);
});

test("development class checks can be bundled without framework materialization", async () => {
  const code = await bundle(
    'export { stillwindRefClassChecked as read } from "stillwind/development";',
  );
  const runtime = await import(
    `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
  );
  expect(runtime.read({ c: "marker" })).toBe("marker");
  expect(() => runtime.read({})).toThrow(/class token/);
  expect(code).not.toMatch(/mergeFragments|toFragment|stillwindClassKey/);
});
