import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { createServer } from "vite-plus";

import { stillwind } from "../src/index.ts";

void test("Vite loads imported CSS and keeps a failed reload's last successful stylesheet", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".session-test-"));
  const css = path.join(root, "root.css");
  const tokens = path.join(root, "tokens.css");
  writeFileSync(css, "@import './tokens.css';");
  writeFileSync(tokens, "@theme { --color-brand: red; }");
  const plugin = stillwind({ css, types: false });
  const server = await createServer({
    configFile: false,
    root,
    plugins: [plugin],
    logLevel: "silent",
    server: { middlewareMode: true, watch: null },
  });
  try {
    await server.pluginContainer.buildStart({});
    const sourcePath = path.join(root, "button.tsx");
    const source =
      "import { style } from 'stillwind/react'; export const props = style('bg-brand');";
    writeFileSync(sourcePath, source);
    const transform = plugin.transform;
    if (typeof transform !== "function") throw new Error("expected transform hook");
    const transformed = await transform.call(
      {
        resolve: (specifier: string, importer: string) =>
          server.pluginContainer.resolveId(specifier, importer),
        addWatchFile: () => {},
        warn: () => {},
        error: (message: string) => {
          throw new Error(message);
        },
      } as unknown as ThisParameterType<typeof transform>,
      source,
      sourcePath,
    );
    assert.ok(
      typeof transformed === "object" && transformed?.code?.toString().includes("className"),
    );
    const load = plugin.load;
    assert.equal(typeof load, "function");
    if (typeof load !== "function") throw new Error("expected load hook");
    const context = server.pluginContainer as unknown as ThisParameterType<typeof load>;
    const before = await load.call(context, "\0virtual:stillwind.css");
    // Pinning is observable even after the file changes, before the reload hook runs.
    writeFileSync(tokens, "@theme { --color-brand: blue; }");
    assert.equal(await load.call(context, "\0virtual:stillwind.css"), before);
    const hot = plugin.handleHotUpdate;
    if (typeof hot !== "function") throw new Error("expected HMR hook");
    const hotContext = {
      file: tokens,
      timestamp: Date.now(),
      modules: [],
      read: async () => "",
      server,
    };
    await hot.call({ warn: () => {} } as unknown as ThisParameterType<typeof hot>, hotContext);
    const after = await load.call(context, "\0virtual:stillwind.css");
    assert.equal(typeof after, "string");
    assert.notEqual(after, before);
    writeFileSync(tokens, "@theme {");
    await hot.call({ warn: () => {} } as unknown as ThisParameterType<typeof hot>, hotContext);
    assert.equal(await load.call(context, "\0virtual:stillwind.css"), after);
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

void test("Vite resolves sibling CSS imports concurrently and preserves authored order", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".session-test-"));
  const css = path.join(root, "root.css");
  const first = path.join(root, "first.css");
  const second = path.join(root, "second.css");
  writeFileSync(css, "@import 'parallel-first'; @import 'parallel-second';");
  writeFileSync(first, ".first { color: red; }");
  writeFileSync(second, ".second { color: blue; }");
  const plugin = stillwind({ css, types: false });
  let releaseFirst: (() => void) | undefined;
  const server = await createServer({
    configFile: false,
    root,
    plugins: [
      {
        name: "parallel-css-test",
        enforce: "pre",
        async resolveId(specifier) {
          if (specifier === "parallel-first") {
            await new Promise<void>((resolve, reject) => {
              const timeout = setTimeout(
                () => reject(new Error("sibling import was not started concurrently")),
                5000,
              );
              releaseFirst = () => {
                clearTimeout(timeout);
                resolve();
              };
            });
            return first;
          }
          if (specifier === "parallel-second") {
            assert.ok(releaseFirst);
            releaseFirst();
            return second;
          }
          return null;
        },
      },
      plugin,
    ],
    logLevel: "silent",
    server: { middlewareMode: true, watch: null },
  });
  try {
    await server.pluginContainer.buildStart({});
    const load = plugin.load;
    if (typeof load !== "function") throw new Error("expected load hook");
    const output = await load.call(
      server.pluginContainer as unknown as ThisParameterType<typeof load>,
      "\0virtual:stillwind.css",
    );
    assert.equal(typeof output, "string");
    if (typeof output !== "string") throw new Error("expected CSS");
    assert.ok(output.includes(".first"));
    assert.ok(output.indexOf(".first") < output.indexOf(".second"));
  } finally {
    releaseFirst?.();
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});
