import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { createServer, resolveConfig } from "vite";
import { expect, test } from "vite-plus/test";

test("query modules retain independent CSS and deletion removes all derivatives of a file", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".test-session-"));
  const css = path.join(root, "root.css");
  writeFileSync(
    css,
    "@import '../packages/core/index.css'; @utility first { width: 13px; } @utility second { width: 17px; } @utility third { width: 19px; }",
  );
  const plugins = stillwind({
    css,
    types: false,
  });
  const [plugin] = plugins;
  const server = await createServer({
    configFile: false,
    root,
    plugins: [plugins],
    logLevel: "silent",
    optimizeDeps: {
      noDiscovery: true,
      include: [],
    },
    server: {
      host: "127.0.0.1",
      port: 0,
      watch: null,
    },
  });
  try {
    await server.listen();
    const transform =
      typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler;
    const remove = plugin.watchChange;
    if (typeof transform !== "function" || typeof remove !== "function") {
      expect.fail("expected transform and watchChange hooks");
    }
    const environment = server.environments.ssr;
    await environment.pluginContainer.buildStart({});
    const context = {
      environment,
      resolve: (specifier: string, importer: string) =>
        environment.pluginContainer.resolveId(specifier, importer),
      addWatchFile: () => {},
      warn: () => {},
    } as unknown as ThisParameterType<typeof transform> & ThisParameterType<typeof remove>;
    const file = path.join(root, "button.tsx");
    const code = (utility: string) =>
      `import {style} from 'stillwind/react'; export const props = style('${utility}');`;
    writeFileSync(file, code("first"));
    await transform.call(context, code("first"), file);
    await transform.call(context, code("second"), file + "?pick=default");
    await transform.call(context, code("third"), file + "?tsr-shared=1");
    const address = server.httpServer!.address();
    if (!address || typeof address === "string") {
      expect.fail("expected HTTP address");
    }
    const origin = `http://127.0.0.1:${address.port}`;
    const selected = await fetch(`${origin}/__stillwind/style.css?module=button.tsx`);
    expect(selected.status).toBe(200);
    expect(selected.headers.get("content-type")).toBe("text/css; charset=utf-8");
    expect(selected.headers.get("cache-control")).toBe("no-store");
    const initial = await selected.text();
    for (const width of [13, 17, 19]) {
      expect(initial).toContain(`width: ${width}px`);
    }
    const omitted = await fetch(`${origin}/__stillwind/style.css?module=other.tsx`);
    expect(await omitted.text()).not.toMatch(/width: (13|17|19)px/);
    await transform.call(context, "export const props = {};", file + "?pick=default");
    const current = await (await fetch(`${origin}/__stillwind/style.css`)).text();
    expect(current).not.toContain("width: 17px");
    expect(current).toContain("width: 13px");
    expect(current).toContain("width: 19px");
    await remove.call(context, file, { event: "delete" });
    const deleted = await (await fetch(`${origin}/__stillwind/style.css`)).text();
    expect(deleted).not.toMatch(/width: (13|17|19)px/);
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test("Vite refreshes imported CSS, retains a failed reload and discards removed imports", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".test-session-"));
  const css = path.join(root, "root.css");
  const tokens = path.join(root, "tokens.css");
  writeFileSync(
    css,
    "@import '../packages/core/index.css'; @import './tokens.css'; .base { color: var(--color-brand); }",
  );
  writeFileSync(tokens, "@theme { --color-brand: red; }");
  const plugins = stillwind({
    css,
    types: false,
  });
  const [plugin] = plugins;
  const development = plugins.find((plugin) => plugin.hotUpdate)!;
  const server = await createServer({
    configFile: false,
    root,
    plugins: [plugins],
    logLevel: "silent",
    server: {
      middlewareMode: true,
      watch: null,
    },
  });
  try {
    // React Router's route-export scanner can transform before buildStart.
    const sourcePath = path.join(root, "button.tsx");
    const source =
      "import { style } from 'stillwind/react'; export const props = style('bg-brand');";
    writeFileSync(sourcePath, source);
    const transform =
      typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler;
    if (typeof transform !== "function") {
      expect.fail("expected transform hook");
    }
    const transformed = await transform.call(
      {
        resolve: (specifier: string, importer: string) =>
          server.pluginContainer.resolveId(specifier, importer),
        addWatchFile: () => {},
        warn: () => {},
        error: (message: string) => {
          expect.fail(message);
        },
      } as unknown as ThisParameterType<typeof transform>,
      source,
      sourcePath,
    );
    if (!(typeof transformed === "object" && transformed?.code?.toString().includes("className"))) {
      expect.fail(
        'Unexpected value: typeof transformed === "object" && transformed?.code?.toString().includes("className")',
      );
    }
    const load = plugin.load;
    expect(typeof load).toBe("function");
    if (typeof load !== "function") {
      expect.fail("expected load hook");
    }
    const context = {
      environment: server.environments.client,
      resolve: (specifier: string, importer: string) =>
        server.environments.client.pluginContainer.resolveId(specifier, importer),
      addWatchFile: () => {},
    } as unknown as ThisParameterType<typeof load>;
    const before = await load.call(context, "\0stillwind:snapshot.css");
    // Fresh stylesheet requests observe edits even when this environment has no HMR connection.
    writeFileSync(tokens, "@theme { --color-brand: blue; }");
    expect(await load.call(context, "\0stillwind:snapshot.css")).not.toStrictEqual(before);
    const hot = development.hotUpdate;
    if (typeof hot !== "function") {
      expect.fail("expected HMR hook");
    }
    const hotContext = {
      file: tokens,
      timestamp: Date.now(),
      modules: [],
      read: async () => "",
      server,
    };
    await hot.call(
      {
        warn: () => {},
        environment: server.environments.client,
      } as unknown as ThisParameterType<typeof hot>,
      {
        ...hotContext,
        type: "update",
      },
    );
    const after = await load.call(context, "\0stillwind:snapshot.css");
    if (!(after && typeof after === "object" && after.map)) {
      expect.fail('Unexpected value: after && typeof after === "object" && after.map');
    }
    expect(after).not.toStrictEqual(before);
    writeFileSync(tokens, "@theme {");
    await hot.call(
      {
        warn: () => {},
        environment: server.environments.client,
      } as unknown as ThisParameterType<typeof hot>,
      { ...hotContext, type: "update" },
    );
    expect(await load.call(context, "\0stillwind:snapshot.css")).toStrictEqual(after);
    writeFileSync(css, ".base { color: green; } @stillwind-slot utilities;");
    rmSync(tokens);
    const withoutImport = await load.call(context, "\0stillwind:snapshot.css");
    expect(withoutImport).not.toStrictEqual(after);
    expect(typeof withoutImport === "string" ? withoutImport : withoutImport?.code).toContain(
      "green",
    );
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test("nested CSS imports resolve while a sibling import waits, and CSS keeps import order", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".test-session-"));
  const css = path.join(root, "root.css");
  const first = path.join(root, "first.css");
  const second = path.join(root, "second.css");
  const child = path.join(root, "child.css");
  const resolutions = new Map<string, number>();
  writeFileSync(css, "@import 'parallel-first'; @import 'parallel-second';");
  writeFileSync(first, ".first { color: red; }");
  writeFileSync(second, "@import 'parallel-child'; .second { color: blue; }");
  writeFileSync(child, ".child { color: green; }");
  const plugins = stillwind({
    css,
    types: false,
  });
  let releaseFirst: (() => void) | undefined;
  const server = await createServer({
    configFile: false,
    root,
    plugins: [
      {
        name: "parallel-css-test",
        enforce: "pre",
        async resolveId(specifier) {
          if (specifier.startsWith("parallel-")) {
            resolutions.set(specifier, (resolutions.get(specifier) ?? 0) + 1);
          }
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
          if (specifier === "parallel-second") {
            return second;
          }
          if (specifier === "parallel-child") {
            expect(releaseFirst).toBeTruthy();
            releaseFirst!();
            return child;
          }
          return null;
        },
      },
      plugins,
    ],
    logLevel: "silent",
    server: {
      middlewareMode: true,
      watch: null,
    },
  });
  try {
    await server.pluginContainer.buildStart({});
    const output = await server.environments.client.pluginContainer.load(
      "\0stillwind:snapshot.css",
    );
    if (!(output && typeof output === "object" && output.map)) {
      expect.fail("Expected the CSS snapshot with a source map");
    }
    expect(output.code).toContain(".first");
    expect(output.code.indexOf(".first")).toBeLessThan(output.code.indexOf(".child"));
    expect(output.code.indexOf(".child")).toBeLessThan(output.code.indexOf(".second"));
    expect(resolutions).toEqual(
      new Map([
        ["parallel-first", 1],
        ["parallel-second", 1],
        ["parallel-child", 1],
      ]),
    );
  } finally {
    releaseFirst?.();
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test("Vite rejects a failed CSS load without waiting for or leaking late read failures", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".test-session-"));
  const css = path.join(root, "root.css");
  writeFileSync(css, "@import 'failed'; @import 'late';");
  const plugins = stillwind({
    css,
    types: false,
  });
  await resolveConfig(
    {
      root,
      configFile: false,
      plugins,
      logLevel: "silent",
    },
    "build",
  );
  const [plugin] = plugins;
  const buildStart = plugin.buildStart;
  if (typeof buildStart !== "function") {
    expect.fail("expected buildStart hook");
  }
  let releaseLate!: () => void;
  const late = new Promise<void>((resolve) => {
    releaseLate = resolve;
  });
  let lateFinished = false;
  try {
    await expect(
      (async () =>
        buildStart.call(
          {
            addWatchFile: () => {},
            async resolve(specifier: string) {
              if (specifier === "failed") {
                throw new Error("first CSS resolution failed");
              }
              await late;
              lateFinished = true;
              throw new Error("late CSS resolution failed");
            },
          } as unknown as ThisParameterType<typeof buildStart>,
          {} as Parameters<typeof buildStart>[0],
        ))(),
    ).rejects.toThrow(/first CSS resolution failed/);
    expect(lateFinished).toBe(false);
    releaseLate();
    // Allow the late resolver rejection and its handler to run. Node's test runner
    // also reports any unhandled rejection, including one occurring after the test.
    await new Promise<void>((resolve) => setImmediate(resolve));
    expect(lateFinished).toBe(true);
  } finally {
    releaseLate();
    rmSync(root, { recursive: true, force: true });
  }
});

test("client and SSR resolve and reload independent CSS snapshots", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".test-session-"));
  const entry = path.join(root, "root.css");
  const clientCss = path.join(root, "client.css");
  const ssrCss = path.join(root, "ssr.css");
  writeFileSync(entry, '@import "environment-css"; @stillwind-slot utilities;');
  writeFileSync(clientCss, ".environment { color: red; } @stillwind-utility paint { color: red; }");
  writeFileSync(ssrCss, ".environment { color: blue; } @stillwind-utility paint { color: blue; }");
  const plugins = stillwind({
    css: entry,
    types: false,
  });
  const [plugin] = plugins;
  const development = plugins.find((plugin) => plugin.hotUpdate)!;
  let heldResolution: Promise<string> | undefined;
  let onHeldResolution: (() => void) | undefined;
  const server = await createServer({
    configFile: false,
    root,
    logLevel: "silent",
    server: {
      middlewareMode: true,
      watch: null,
    },
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
          if (id === "environment-css") {
            return this.environment.name === "ssr" ? ssrCss : clientCss;
          }
          return undefined;
        },
      },
      plugins,
    ],
  });
  try {
    const client = server.environments.client;
    const ssr = server.environments.ssr;
    await Promise.all([client.pluginContainer.buildStart({}), ssr.pluginContainer.buildStart({})]);
    const load = plugin.load;
    const hot = development.hotUpdate;
    const transform =
      typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler;
    if (
      !(typeof load === "function" && typeof hot === "function" && typeof transform === "function")
    ) {
      expect.fail(
        'Unexpected value: typeof load === "function" && typeof hot === "function" && typeof transform === "function"',
      );
    }
    const readCss = async (environment: typeof client) => {
      const css = await load!.call(
        {
          environment,
          resolve: (specifier: string, importer: string) =>
            environment.pluginContainer.resolveId(specifier, importer),
          addWatchFile: () => {},
        } as unknown as ThisParameterType<typeof load>,
        "\0stillwind:snapshot.css",
      );
      if (!(css && typeof css === "object" && css.map)) {
        expect.fail('Unexpected value: css && typeof css === "object" && css.map');
      }
      return (css as { code: string }).code;
    };
    expect(String(await readCss(client))).toMatch(/color: red/);
    expect(String(await readCss(ssr))).toMatch(/color: blue/);
    const sourcePath = path.join(root, "shared.ts");
    const source =
      'import { style } from "stillwind/react"; import { unrelated } from "./unrelated"; export const a = style("paint"); console.log(unrelated);';
    writeFileSync(sourcePath, source);
    const compile = (environment: typeof client) =>
      transform!.call(
        {
          environment,
          resolve: (specifier: string, importer: string) => {
            expect(specifier, "a local style needs no semantic dependency reads").not.toBe(
              "./unrelated",
            );
            return environment.pluginContainer.resolveId(specifier, importer);
          },
          addWatchFile: () => {},
          warn: () => {},
          error: (message: string) => {
            expect.fail(message);
          },
        } as unknown as ThisParameterType<typeof transform>,
        source,
        sourcePath,
      );
    const outputs = await Promise.all([compile(client), compile(ssr)]);
    if (!outputs.every((output) => output && typeof output === "object" && output.code)) {
      expect.fail(
        'Unexpected value: outputs.every((output) => output && typeof output === "object" && output.code)',
      );
    }
    const moduleCss = await Promise.all([readCss(client), readCss(ssr)]);
    expect(moduleCss[0]).toMatch(/red/);
    expect(moduleCss[1]).toMatch(/blue/);
    writeFileSync(
      clientCss,
      ".environment { color: green; } @stillwind-utility paint { color: green; }",
    );
    await hot!.call(
      {
        environment: client,
        warn: () => {},
      } as unknown as ThisParameterType<typeof hot>,
      {
        type: "update",
        file: clientCss,
        timestamp: Date.now(),
        modules: [],
        read: async () => "",
        server,
      },
    );
    expect(String(await readCss(client))).toMatch(/color: green/);
    expect(String(await readCss(ssr))).toMatch(/color: blue/);
    let release!: (path: string) => void;
    heldResolution = new Promise<string>((resolve) => {
      release = resolve;
    });
    const started = new Promise<void>((resolve) => {
      onHeldResolution = resolve;
    });
    const reload = () =>
      hot!.call(
        {
          environment: client,
          warn: () => {},
        } as unknown as ThisParameterType<typeof hot>,
        {
          type: "update",
          file: clientCss,
          timestamp: Date.now(),
          modules: [],
          read: async () => "",
          server,
        },
      );
    writeFileSync(
      clientCss,
      ".environment { color: purple; } @stillwind-utility paint { color: purple; }",
    );
    const older = reload();
    await started;
    writeFileSync(
      clientCss,
      ".environment { color: green; } @stillwind-utility paint { color: green; }",
    );
    const newer = reload();
    release(ssrCss);
    await Promise.all([older, newer]);
    expect(String(await readCss(client))).toMatch(/color: green/);
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});
