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
  writeFileSync(
    css,
    "@import '../packages/core/index.css'; @import './tokens.css'; .base { color: var(--color-brand); }",
  );
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
    // React Router's route-export scanner can transform before buildStart.
    const sourcePath = path.join(root, "button.tsx");
    const source =
      "import { style } from 'stillwind/react'; export const props = style('bg-brand');";
    writeFileSync(sourcePath, source);
    const transform =
      typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler;
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
    const before = await load.call(context, "\0stillwind:snapshot.css");
    // Pinning is observable even after the file changes, before the reload hook runs.
    writeFileSync(tokens, "@theme { --color-brand: blue; }");
    assert.deepEqual(await load.call(context, "\0stillwind:snapshot.css"), before);
    const hot = plugin.hotUpdate;
    if (typeof hot !== "function") throw new Error("expected HMR hook");
    const hotContext = {
      file: tokens,
      timestamp: Date.now(),
      modules: [],
      read: async () => "",
      server,
    };
    await hot.call(
      { warn: () => {}, environment: server.environments.client } as unknown as ThisParameterType<
        typeof hot
      >,
      { ...hotContext, type: "update" },
    );
    const after = await load.call(context, "\0stillwind:snapshot.css");
    assert.ok(after && typeof after === "object" && after.map);
    assert.notDeepEqual(after, before);
    writeFileSync(tokens, "@theme {");
    await hot.call(
      { warn: () => {}, environment: server.environments.client } as unknown as ThisParameterType<
        typeof hot
      >,
      { ...hotContext, type: "update" },
    );
    assert.deepEqual(await load.call(context, "\0stillwind:snapshot.css"), after);
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

void test("Vite supplies ready CSS and discovers nested imports before a slow sibling finishes", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".session-test-"));
  const css = path.join(root, "root.css");
  const first = path.join(root, "first.css");
  const second = path.join(root, "second.css");
  const child = path.join(root, "child.css");
  const resolutions = new Map<string, number>();
  writeFileSync(css, "@import 'parallel-first'; @import 'parallel-second';");
  writeFileSync(first, ".first { color: red; }");
  writeFileSync(second, "@import 'parallel-child'; .second { color: blue; }");
  writeFileSync(child, ".child { color: green; }");
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
          if (specifier.startsWith("parallel-"))
            resolutions.set(specifier, (resolutions.get(specifier) ?? 0) + 1);
          if (specifier === "parallel-first") {
            await new Promise<void>((resolve, reject) => {
              const timeout = setTimeout(
                () =>
                  reject(
                    new Error("nested import was not started before the slow sibling finished"),
                  ),
                5000,
              );
              releaseFirst = () => {
                clearTimeout(timeout);
                resolve();
              };
            });
            return first;
          }
          if (specifier === "parallel-second") return second;
          if (specifier === "parallel-child") {
            assert.ok(releaseFirst);
            releaseFirst();
            return child;
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
      "\0stillwind:snapshot.css",
    );
    assert.ok(output && typeof output === "object" && output.map);
    if (!output || typeof output !== "object") throw new Error("expected mapped CSS");
    assert.ok(output.code.includes(".first"));
    assert.ok(output.code.indexOf(".first") < output.code.indexOf(".child"));
    assert.ok(output.code.indexOf(".child") < output.code.indexOf(".second"));
    assert.deepEqual(
      [...resolutions.entries()],
      [
        ["parallel-first", 1],
        ["parallel-second", 1],
        ["parallel-child", 1],
      ],
    );
  } finally {
    releaseFirst?.();
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

void test("Vite rejects a failed CSS load without waiting for or leaking late read failures", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".session-test-"));
  const css = path.join(root, "root.css");
  writeFileSync(css, "@import 'failed'; @import 'late';");
  const plugin = stillwind({ css, types: false });
  const buildStart = plugin.buildStart;
  if (typeof buildStart !== "function") throw new Error("expected buildStart hook");
  let releaseLate!: () => void;
  const late = new Promise<void>((resolve) => {
    releaseLate = resolve;
  });
  let lateFinished = false;
  try {
    await assert.rejects(
      async () =>
        buildStart.call(
          {
            async resolve(specifier: string) {
              if (specifier === "failed") throw new Error("first CSS resolution failed");
              await late;
              lateFinished = true;
              throw new Error("late CSS resolution failed");
            },
          } as unknown as ThisParameterType<typeof buildStart>,
          {} as Parameters<typeof buildStart>[0],
        ),
      /first CSS resolution failed/,
    );
    assert.equal(lateFinished, false);
    releaseLate();
    // Allow the late resolver rejection and its handler to run. Node's test runner
    // also reports any unhandled rejection, including one occurring after the test.
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.equal(lateFinished, true);
  } finally {
    releaseLate();
    rmSync(root, { recursive: true, force: true });
  }
});

void test("client and SSR resolve and reload independent CSS snapshots", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".session-test-"));
  const entry = path.join(root, "root.css");
  const clientCss = path.join(root, "client.css");
  const ssrCss = path.join(root, "ssr.css");
  writeFileSync(entry, '@import "environment-css"; @stillwind-slot utilities;');
  writeFileSync(clientCss, ".environment { color: red; } @stillwind-utility paint { color: red; }");
  writeFileSync(ssrCss, ".environment { color: blue; } @stillwind-utility paint { color: blue; }");
  const plugin = stillwind({ css: entry, types: false });
  let heldResolution: Promise<string> | undefined;
  let onHeldResolution: (() => void) | undefined;
  const server = await createServer({
    configFile: false,
    root,
    logLevel: "silent",
    server: { middlewareMode: true, watch: null },
    plugins: [
      {
        name: "environment-css",
        resolveId(id) {
          if (id === "environment-css" && heldResolution) {
            const held = heldResolution;
            heldResolution = undefined;
            onHeldResolution?.();
            return held;
          }
          if (id === "environment-css") return this.environment.name === "ssr" ? ssrCss : clientCss;
        },
      },
      plugin,
    ],
  });
  try {
    const client = server.environments.client;
    const ssr = server.environments.ssr;
    await Promise.all([client.pluginContainer.buildStart({}), ssr.pluginContainer.buildStart({})]);
    const load = plugin.load;
    const hot = plugin.hotUpdate;
    const transform =
      typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler;
    assert.ok(
      typeof load === "function" && typeof hot === "function" && typeof transform === "function",
    );
    const readCss = async (environment: typeof client) => {
      const css = await load.call(
        { environment } as ThisParameterType<typeof load>,
        "\0stillwind:snapshot.css",
      );
      assert.ok(css && typeof css === "object" && css.map);
      return (css as { code: string }).code;
    };
    assert.match(String(await readCss(client)), /color: red/);
    assert.match(String(await readCss(ssr)), /color: blue/);
    const sourcePath = path.join(root, "shared.ts");
    const source =
      'import { style } from "stillwind/react"; import { unrelated } from "./unrelated"; export const a = style("paint"); console.log(unrelated);';
    writeFileSync(sourcePath, source);
    const compile = (environment: typeof client) =>
      transform.call(
        {
          environment,
          resolve: (specifier: string, importer: string) => {
            assert.notEqual(
              specifier,
              "./unrelated",
              "a local style needs no semantic dependency reads",
            );
            return environment.pluginContainer.resolveId(specifier, importer);
          },
          addWatchFile: () => {},
          warn: () => {},
          error: (message: string) => {
            throw new Error(message);
          },
        } as unknown as ThisParameterType<typeof transform>,
        source,
        sourcePath,
      );
    const outputs = await Promise.all([compile(client), compile(ssr)]);
    assert.ok(outputs.every((output) => output && typeof output === "object" && output.code));
    const moduleCss = await Promise.all([readCss(client), readCss(ssr)]);
    assert.match(moduleCss[0], /red/);
    assert.match(moduleCss[1], /blue/);
    writeFileSync(
      clientCss,
      ".environment { color: green; } @stillwind-utility paint { color: green; }",
    );
    await hot.call(
      { environment: client, warn: () => {} } as unknown as ThisParameterType<typeof hot>,
      {
        type: "update",
        file: clientCss,
        timestamp: Date.now(),
        modules: [],
        read: async () => "",
        server,
      },
    );
    assert.match(String(await readCss(client)), /color: green/);
    assert.match(String(await readCss(ssr)), /color: blue/);
    let release!: (path: string) => void;
    heldResolution = new Promise<string>((resolve) => {
      release = resolve;
    });
    const started = new Promise<void>((resolve) => {
      onHeldResolution = resolve;
    });
    const reload = () =>
      hot.call(
        { environment: client, warn: () => {} } as unknown as ThisParameterType<typeof hot>,
        {
          type: "update",
          file: clientCss,
          timestamp: Date.now(),
          modules: [],
          read: async () => "",
          server,
        },
      );
    const older = reload();
    await started;
    await reload();
    release(ssrCss);
    await older;
    assert.match(String(await readCss(client)), /color: green/);
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});
