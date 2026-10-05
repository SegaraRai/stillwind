import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { setTimeout as pause } from "node:timers/promises";
import { fileURLToPath } from "node:url";

import { resolveFixtureCss } from "@stillwind-internal/tools-fixture-css/stillwind-css";
import {
  NativeDesignSystemLoader,
  NativeProjectSession,
  type NativePreparedSource,
} from "@stillwind/native";
import { build, createServer, type Plugin, type ViteDevServer } from "vite-plus";
import { expect, inject, test, vi, type BenchRunOptions } from "vite-plus/test";

import { compilerPlugin, stillwind } from "../../packages/integration/src/vite/plugin.ts";
import { createViteProject } from "../../packages/integration/src/vite/project.ts";
import { assertBudget, budgets } from "./budgets.ts";

const full = inject("performanceFull");
const sizes = full ? ([10, 100, 1000] as const) : ([10, 100] as const);
const options: BenchRunOptions = {
  iterations: 3,
  time: 0,
  warmupIterations: 1,
  warmupTime: 0,
};

const directory = path.dirname(fileURLToPath(import.meta.url));

const minimal =
  "@utility flex {display:flex;} @utility block {display:block;} @stillwind-slot utilities;";
const systems = [
  ["minimal", minimal],
  ["full-registry", resolveFixtureCss('@import "stillwind";')],
] as const;
const source =
  "import {style} from 'stillwind/react';" +
  Array.from({ length: 8 }, (_, j) => `export const props${j}=style('flex');`).join("\n");

for (const count of sizes) {
  for (const [system, css] of systems) {
    test(`build/${system}/${count}`, async (context) => {
      const root = fixture(count, css);
      context.task.meta.input = {
        modules: count,
        callsPerModule: 8,
        system,
        diskOutput: true,
      };
      context.task.meta.measurement = { ...options, warmupIterations: 0 };
      try {
        let output: Awaited<ReturnType<typeof build>>;
        const result = await context
          .bench("full-build", { async: true }, async () => {
            output = await build({
              configFile: false,
              root,
              plugins: [stillwind({ css: "root.css", types: false })],
              logLevel: "silent",
              build: { write: true, sourcemap: false },
            });
          })
          .run(context.task.meta.measurement);
        expect(output!).toHaveProperty("output");
        await assertBudget(context, "full-build", result, budgets.build[count]);
      } finally {
        rmSync(root, { recursive: true, force: true });
      }
    });

    test(`watch/${system}/${count}`, async (context) => {
      const root = fixture(count, css);
      context.task.meta.input = {
        modules: count,
        callsPerModule: 8,
        system,
        fileWatcher: true,
        hmrConnection: false,
      };
      context.task.meta.measurement = options;
      let active: ViteDevServer | undefined;
      try {
        const startup = await context
          .bench(
            "watch-ready",
            {
              async: true,
              afterEach: async () => {
                await active?.close();
                active = undefined;
              },
            },
            async () => {
              active = await startServer(root, stillwind({ css: "root.css", types: false }));
            },
          )
          .run(options);
        await assertBudget(context, "watch-ready", startup, budgets.startup[count]);

        const project = createViteProject({ css: "root.css", types: false });
        const plugin = compilerPlugin(project);
        active = await startServer(root, plugin);
        const environment = active.environments.client;
        const hook =
          typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler;
        if (!hook) {
          expect.fail("Compiler transform hook is required");
        }
        const server = active;
        const host = {
          environment,
          resolve: (specifier: string, importer: string) =>
            environment.pluginContainer.resolveId(specifier, importer),
          addWatchFile: (file: string) => server.watcher.add(file),
          warn: (message: string) => expect.fail(message),
        };
        // Compile all producers before measuring edits. Publishing once keeps setup short.
        for (let i = 0; i < count; i++) {
          await project.compile("client", source, moduleFile(root, i), host);
        }
        await project.publish("client", host);
        await environment.transformRequest("/module-0.ts");
        let edit = 0;
        const transform = await context
          .bench("transform-to-css", { async: true }, async () => {
            const output = await hook.call(
              host as unknown as ThisParameterType<typeof hook>,
              source.replace("'flex'", ++edit % 2 ? "'block'" : "'flex'"),
              moduleFile(root, 0),
            );
            if (!output) {
              expect.fail("Edited producer must produce JavaScript");
            }
          })
          .run(options);
        await assertBudget(context, "transform-to-css", transform, budgets.transform[count]);

        const original = environment.hot.send.bind(environment.hot);
        let finished: (() => void) | undefined;
        const send = vi.spyOn(environment.hot, "send").mockImplementation((...args) => {
          original(...args);
          const payload: unknown = args[0];
          if (
            typeof payload === "object" &&
            payload !== null &&
            "type" in payload &&
            payload.type === "custom" &&
            "event" in payload &&
            payload.event === "stillwind:css"
          ) {
            finished?.();
          }
        });
        try {
          // Wait between samples so Chokidar does not merge writes. The wait is not timed.
          const filesystem = await context
            .bench(
              "file-write-to-css",
              {
                async: true,
                beforeEach: async () => {
                  await pause(75);
                },
              },
              async () => {
                let timeout: ReturnType<typeof setTimeout> | undefined;
                const published = new Promise<void>((resolve, reject) => {
                  timeout = setTimeout(() => reject(new Error("CSS publication timed out")), 5000);
                  finished = resolve;
                });
                try {
                  writeFileSync(
                    moduleFile(root, 0),
                    source.replace("'flex'", ++edit % 2 ? "'block'" : "'flex'"),
                  );
                  await published;
                } finally {
                  clearTimeout(timeout);
                  finished = undefined;
                }
              },
            )
            .run(options);
          await assertBudget(
            context,
            "file-write-to-css",
            filesystem,
            budgets.filesystemEdit[count],
          );
        } finally {
          send.mockRestore();
        }
      } finally {
        await active?.close();
        rmSync(root, { recursive: true, force: true });
      }
    });
  }
}

for (const count of sizes) {
  test(`css-files/${count}`, async (context) => {
    const root = fixture(
      1,
      minimal + Array.from({ length: count }, (_, i) => `@import './file-${i}.css';`).join("\n"),
    );
    for (let i = 0; i < count; i++) {
      writeFileSync(path.join(root, `file-${i}.css`), `.rule-${i}{color:red;}`);
    }
    context.task.meta.input = {
      importedCssFiles: count,
      requestedJsModules: 1,
    };
    context.task.meta.measurement = options;
    let active: ViteDevServer | undefined;
    try {
      const startup = await context
        .bench(
          "watch-ready",
          {
            async: true,
            afterEach: async () => {
              await active?.close();
              active = undefined;
            },
          },
          async () => {
            active = await startServer(root, stillwind({ css: "root.css", types: false }));
          },
        )
        .run(options);
      await assertBudget(context, "css-watch-ready", startup, budgets.cssStartup[count]);
      const plugin = stillwind({ css: "root.css", types: false });
      active = await startServer(root, plugin);
      const hook =
        typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler;
      if (!hook) {
        expect.fail("Compiler transform hook is required");
      }
      const server = active;
      const host = {
        environment: server.environments.client,
        resolve: (specifier: string, importer: string) =>
          server.environments.client.pluginContainer.resolveId(specifier, importer),
        addWatchFile: (file: string) => server.watcher.add(file),
        warn: (message: string) => expect.fail(message),
      } as unknown as ThisParameterType<typeof hook>;
      await hook.call(host, source, moduleFile(root, 0));
      let edit = 0;
      const result = await context
        .bench("transform-to-css", { async: true }, async () => {
          await hook.call(
            host,
            source.replace("'flex'", ++edit % 2 ? "'block'" : "'flex'"),
            moduleFile(root, 0),
          );
        })
        .run(options);
      await assertBudget(context, "css-transform-to-css", result, budgets.cssTransform[count]);
      const original = server.environments.client.hot.send.bind(server.environments.client.hot);
      let finished: (() => void) | undefined;
      let publishedPayload: unknown;
      const send = vi
        .spyOn(server.environments.client.hot, "send")
        .mockImplementation((...args) => {
          original(...args);
          const payload: unknown = args[0];
          if (
            typeof payload === "object" &&
            payload !== null &&
            "type" in payload &&
            payload.type === "custom" &&
            "event" in payload &&
            payload.event === "stillwind:css"
          ) {
            publishedPayload = payload;
            finished?.();
          }
        });
      try {
        const reload = await context
          .bench(
            "css-file-write-to-css",
            {
              async: true,
              beforeEach: async () => {
                await pause(75);
              },
              afterEach: () => {
                expect(JSON.stringify(publishedPayload)).toMatch(
                  new RegExp(`\\.rule-0\\s*\\{\\s*color:\\s*${edit % 2 ? "blue" : "red"}`),
                );
              },
            },
            async () => {
              let timeout: ReturnType<typeof setTimeout> | undefined;
              const published = new Promise<void>((resolve, reject) => {
                timeout = setTimeout(
                  () => reject(new Error("CSS reload publication timed out")),
                  5000,
                );
                finished = resolve;
              });
              try {
                writeFileSync(
                  path.join(root, "file-0.css"),
                  `.rule-0{color:${++edit % 2 ? "blue" : "red"};}`,
                );
                await published;
              } finally {
                clearTimeout(timeout);
                finished = undefined;
              }
            },
          )
          .run(options);
        await assertBudget(
          context,
          "css-file-write-to-css",
          reload,
          budgets.cssFilesystemEdit[count],
        );
      } finally {
        send.mockRestore();
      }
    } finally {
      await active?.close();
      rmSync(root, { recursive: true, force: true });
    }
  });
}

for (const count of (full ? [100, 1000, 10000] : [100, 1000]) as (100 | 1000 | 10000)[]) {
  test(`css-definitions/${count}`, async (context) => {
    const css =
      minimal +
      Array.from(
        { length: count },
        (_, i) => `@utility probe-${i} {color:rgb(${i % 256} 0 0);}`,
      ).join("\n");
    context.task.meta.input = {
      unusedUtilities: count,
      modules: 100,
      callsPerModule: 1,
    };
    context.task.meta.measurement = options;
    let loaded!: NativeDesignSystemLoader;
    const result = await context
      .bench("snapshot", { async: false }, () => {
        loaded = new NativeDesignSystemLoader();
        loaded.provide(0, "root.css", css, "untitled:root.css", "untitled:root.css");
      })
      .run(options);
    await assertBudget(context, "definitions-snapshot", result, budgets.definitions[count]);
    const input: NativePreparedSource[] = Array.from({ length: 100 }, (_, i) => ({
      kind: "direct",
      moduleId: `module-${i}.ts`,
      document: {
        uri: `untitled:module-${i}.ts`,
        sourceMapSource: `untitled:module-${i}.ts`,
        format: "jsx",
        code: "import {style} from 'stillwind/react'; export const props=style('flex');",
      },
      syntax: { language: "ts", moduleKind: "unambiguous" },
    }));
    const session = new NativeProjectSession(loaded, true);
    expect(session.compile(input).modules.every((module) => module.diagnostics.length === 0)).toBe(
      true,
    );
    let edit = 0;
    const edited = await context
      .bench("edit", { async: false }, () => {
        session.compile([
          {
            ...input[0],
            document: {
              ...input[0].document,
              code: input[0].document.code.replace("'flex'", ++edit % 2 ? "'block'" : "'flex'"),
            },
          },
        ]);
      })
      .run(options);
    expect(session.moduleCss(input[0].moduleId)).not.toBe("");
    await assertBudget(context, "definitions-edit", edited, budgets.definitionEdit[count]);
  });
}

function moduleFile(root: string, index: number): string {
  return path.join(root, `module-${index}.ts`).replaceAll("\\", "/");
}

/**
 * Creates real producer files and an entry that keeps every producer in the build.
 */
function fixture(count: number, css: string): string {
  const root = mkdtempSync(path.join(directory, "../.test-performance-"));
  writeFileSync(path.join(root, "root.css"), css);
  writeFileSync(
    path.join(root, "index.html"),
    '<html><body><script type="module" src="/entry.ts"></script></body></html>',
  );
  for (let i = 0; i < count; i++) {
    writeFileSync(moduleFile(root, i), source);
  }
  writeFileSync(
    path.join(root, "entry.ts"),
    Array.from({ length: count }, (_, i) => `import * as p${i} from './module-${i}.ts';`).join(
      "\n",
    ) + `\nglobalThis.benchmark=[${Array.from({ length: count }, (_, i) => `p${i}`).join(",")}];`,
  );
  return root;
}

/**
 * Returns a listening server after its initial filesystem scan, closing it on failure.
 */
async function startServer(root: string, plugin: Plugin): Promise<ViteDevServer> {
  let watched!: () => void;
  const ready = new Promise<void>((resolve) => {
    watched = resolve;
  });
  const server = await createServer({
    configFile: false,
    root,
    plugins: [
      {
        name: "watch-ready-probe",
        configureServer(server) {
          server.watcher.once("ready", watched);
        },
      },
      plugin,
    ],
    logLevel: "silent",
    optimizeDeps: { noDiscovery: true, include: [] },
    server: { host: "127.0.0.1", port: 0 },
  });
  try {
    await server.listen();
    await ready;
    return server;
  } catch (error) {
    await server.close();
    throw error;
  }
}
