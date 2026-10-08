import { createHash } from "node:crypto";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { resolveFixtureCss } from "@stillwind-internal/tools-fixture-css/stillwind-css";
import { loadCssGraph } from "@stillwind/host";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import type { NativePreparedSource } from "@stillwind/native";
import { build, createServer } from "vite";
import { expect, test, vi, type BenchRunOptions } from "vite-plus/test";

import { stillwind } from "../../packages/integration/src/vite/plugin.ts";

declare module "vite-plus/test" {
  interface TaskMeta {
    input?: Record<string, string | number | boolean>;
    output?: Record<string, number>;
    memory?: ReturnType<typeof process.memoryUsage>;
    listenMs?: number[];
    watchReadyMs?: number[];
    watcherMs?: number[];
    hotUpdateMs?: number[];
  }
}

const options: BenchRunOptions = {
  iterations: 5,
  time: 0,
  warmupIterations: 1,
  warmupTime: 0,
};
const minimal =
  "@utility flex {display:flex;} @utility block {display:block;} @stillwind-slot utilities;";
const full = resolveFixtureCss('@import "stillwind";');

for (const kind of ["authored", "tokens", "utilities"] as const) {
  for (const count of [100, 1000, 10_000]) {
    const css = definitionCss(kind, count);
    test(`css-load/${kind}/${count}`, async ({ bench, task }) => {
      task.meta.input = { kind, count, cssBytes: Buffer.byteLength(css) };
      task.meta.measurement = options;
      let loaded!: NativeDesignSystemLoader;
      let session!: NativeProjectSession;
      await bench("snapshot", { async: false }, () => {
        loaded = loader(css);
      }).run(options);
      await bench("empty-session", { async: false }, () => {
        session = new NativeProjectSession(loaded, true);
      }).run(options);
      expect(session).toBeInstanceOf(NativeProjectSession);
      const output = session.compile(sources(1));
      expect(output.modules[0].diagnostics).toEqual([]);
    });
  }
}

for (const shape of ["fanout", "chain"] as const) {
  for (const count of [10, 100, 300, 1000]) {
    test(`css-imports/${shape}/${count}`, async ({ bench, task }) => {
      const files = new Map<string, string>();
      for (let i = 0; i < count; i++) {
        files.set(
          `file-${i}.css`,
          (shape === "chain" && i + 1 < count ? `@import 'file-${i + 1}.css';` : "") +
            `.rule-${i}{color:red;}`,
        );
      }
      files.set(
        "root.css",
        minimal +
          (shape === "fanout"
            ? Array.from({ length: count }, (_, i) => `@import 'file-${i}.css';`).join("\n")
            : "@import 'file-0.css';"),
      );
      task.meta.input = {
        shape,
        count,
        cssBytes: [...files.values()].reduce((n, css) => n + Buffer.byteLength(css), 0),
      };
      task.meta.measurement = options;
      let loaded!: Awaited<ReturnType<typeof loadCssGraph>>;
      let reads = 0;
      await bench("host-individual-responses", { async: true }, async () => {
        reads = 0;
        loaded = await loadCssGraph(
          "root.css",
          {},
          {
            resolve: async (specifier) => specifier,
            read: async (name) => {
              reads++;
              return files.get(name)!;
            },
            documentUri: (name) => `untitled:${name}`,
            sourceMapSource: (name) => `untitled:${name}`,
            watch: () => {},
          },
        );
      }).run(options);
      expect(reads).toBe(count + 1);
      expect(loaded.loader).toBeInstanceOf(NativeDesignSystemLoader);
      if (shape === "fanout") {
        let batched!: NativeDesignSystemLoader;
        let resultKind = "";
        await bench("native-batch-responses", { async: false }, () => {
          batched = new NativeDesignSystemLoader();
          const step = batched.provide(
            0,
            "root.css",
            files.get("root.css")!,
            "untitled:root.css",
            "untitled:root.css",
          );
          if (step.kind !== "needFile") {
            expect.fail("Expected import requests");
          }
          const result = batched.provideMany(
            step.requests.map((request) => ({
              id: request.id,
              name: request.specifier,
              content: files.get(request.specifier)!,
              uri: `untitled:${request.specifier}`,
              sourceMapSource: `untitled:${request.specifier}`,
            })),
          );
          resultKind = result.kind;
        }).run(options);
        expect(resultKind).toBe("complete");
        expect(new NativeProjectSession(batched, true).css()).toEqual(
          new NativeProjectSession(loaded.loader, true).css(),
        );
      }
    });
  }
}

for (const count of [100, 1000]) {
  test(`js-load/batch-versus-single/${count}`, async ({ bench, task }) => {
    const design = loader(minimal);
    const input = sources(count);
    const slowOptions = {
      ...options,
      iterations: count === 1000 ? 1 : 3,
      warmupIterations: 0,
    };
    task.meta.input = {
      count,
      callsPerModule: 1,
      cssBytes: Buffer.byteLength(minimal),
    };
    task.meta.measurement = slowOptions;
    let batch!: NativeProjectSession;
    let individual!: NativeProjectSession;
    await bench("batch", { async: false }, () => {
      batch = new NativeProjectSession(design, false);
      batch.compile(input);
    }).run(slowOptions);
    await bench("individual", { async: false }, () => {
      individual = new NativeProjectSession(design, false);
      for (const source of input) {
        individual.compile([source]);
      }
    }).run(slowOptions);
    expect(individual.css()).toEqual(batch.css());
    let edit = 0;
    await bench("edit-plus-document-css", { async: false }, () => {
      individual.compile([
        {
          ...input[0],
          document: {
            ...input[0].document,
            code: input[0].document.code.replace("'flex'", ++edit % 2 ? "'block'" : "'flex'"),
          },
        },
      ]);
      individual.css();
    }).run(options);
    expect(individual.moduleCss(input[0].moduleId)).not.toBe("");
  });
}

for (const utilities of [10, 100, 1000, 10_000]) {
  test(`joint/js100/utilities${utilities}`, async ({ bench, task }) => {
    const design = loader(definitionCss("utilities", utilities));
    const input = sources(100);
    task.meta.input = {
      modules: 100,
      callsPerModule: 1,
      unusedUtilities: utilities,
    };
    task.meta.measurement = { ...options, iterations: 3, warmupIterations: 0 };
    let session!: NativeProjectSession;
    await bench("fresh", { async: false }, () => {
      session = new NativeProjectSession(design, true);
      session.compile(input);
    }).run(task.meta.measurement as BenchRunOptions);
    let edit = 0;
    await bench("edit", { async: false }, () => {
      session.compile([
        {
          ...input[0],
          document: {
            ...input[0].document,
            code: input[0].document.code.replace("'flex'", ++edit % 2 ? "'block'" : "'flex'"),
          },
        },
      ]);
    }).run(options);
    task.meta.memory = process.memoryUsage();
    expect(session.moduleCss(input[0].moduleId)).not.toBe("");
  });
}

for (const [name, css] of [
  ["minimal", minimal],
  ["full-registry", full],
  ["utilities10000", definitionCss("utilities", 10_000)],
] as const) {
  test(`vite/watch-ready/${name}`, async ({ bench, task }) => {
    const root = mkdtempSync(
      path.join(path.dirname(fileURLToPath(import.meta.url)), "../.test-architecture-"),
    );
    writeFileSync(path.join(root, "root.css"), css);
    writeFileSync(path.join(root, "index.html"), "<html><body>ready</body></html>");
    for (let i = 0; i < 1000; i++) {
      writeFileSync(path.join(root, `module-${i}.ts`), sources(1)[0].document.code);
    }
    task.meta.input = {
      modulesOnDisk: 1000,
      requestedJsModules: 0,
      cssBytes: Buffer.byteLength(css),
      dependencyDiscovery: false,
    };
    task.meta.measurement = options;
    let startup = 0;
    const listenMs: number[] = [];
    const watchReadyMs: number[] = [];
    try {
      await bench("create-listen-close", { async: true }, async () => {
        const start = performance.now();
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
            stillwind({ css: "root.css" }),
          ],
          logLevel: "silent",
          optimizeDeps: { noDiscovery: true, include: [] },
          server: { host: "127.0.0.1", port: 0 },
        });
        try {
          await server.listen();
          startup = performance.now() - start;
          listenMs.push(startup);
          await ready;
          watchReadyMs.push(performance.now() - start);
        } finally {
          await server.close();
        }
      }).run(options);
      task.meta.listenMs = listenMs.slice(options.warmupIterations);
      task.meta.watchReadyMs = watchReadyMs.slice(options.warmupIterations);
      expect(startup).toBeGreaterThan(0);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
}

for (const [count, calls, name, css, watching] of [
  [100, 1, "minimal", minimal, false],
  [1000, 1, "minimal", minimal, false],
  [1000, 8, "minimal", minimal, false],
  [1000, 8, "full-registry", full, false],
  [1000, 8, "utilities10000", definitionCss("utilities", 10_000), false],
  [1000, 8, "minimal", minimal, true],
  [1000, 8, "full-registry", full, true],
  [1000, 8, "utilities10000", definitionCss("utilities", 10_000), true],
] as const) {
  const title =
    `vite/watch-edit/${count}/${calls}-calls` +
    (name === "minimal" ? "" : `/${name}`) +
    (watching ? "/watch-enabled" : "");
  test(title, async ({ bench, task }) => {
    const root = mkdtempSync(
      path.join(path.dirname(fileURLToPath(import.meta.url)), "../.test-watch-edit-"),
    );
    writeFileSync(path.join(root, "root.css"), css);
    const input = sources(count, calls);
    const plugins = stillwind({
      css: "root.css",
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
        watch: watching ? undefined : null,
      },
    });
    try {
      await server.listen();
      const transform =
        typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler;
      if (typeof transform !== "function") {
        expect.fail("Expected compiler transform hook");
      }
      const context = {
        environment: server.environments.client,
        resolve: (specifier: string, importer: string) =>
          server.environments.client.pluginContainer.resolveId(specifier, importer),
        addWatchFile: () => {},
        warn: (message: string) => {
          expect.fail(message);
        },
      } as unknown as ThisParameterType<typeof transform>;
      const preloadStart = performance.now();
      for (const source of input) {
        await transform.call(context, source.document.code, path.join(root, source.moduleId));
      }
      task.meta.input = {
        modules: count,
        callsPerModule: calls,
        cssBytes: Buffer.byteLength(css),
        designSystem: name,
        devSourceMaps: false,
        fileWatcher: watching,
        transport: "transform-and-hot-send",
      };
      task.meta.output = {
        preloadMs: performance.now() - preloadStart,
      };
      task.meta.measurement = options;
      let edit = 0;
      let result!: Awaited<ReturnType<typeof transform>>;
      await bench("edit-and-publish", { async: true }, async () => {
        result = await transform.call(
          context,
          input[0].document.code.replace("'flex'", ++edit % 2 ? "'block'" : "'flex'"),
          path.join(root, input[0].moduleId),
        );
      }).run(options);
      expect(result).toHaveProperty("code");
    } finally {
      await server.close();
      rmSync(root, { recursive: true, force: true });
    }
  });
}

test("vite/watch-event/1000/8-calls/full-registry", async ({ bench, task }) => {
  const root = mkdtempSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), "../.test-watch-event-"),
  );
  const input = sources(1000, 8);
  writeFileSync(path.join(root, "root.css"), full);
  for (const source of input) {
    writeFileSync(path.join(root, source.moduleId), source.document.code);
  }
  const plugins = stillwind({ css: "root.css" });
  const [plugin] = plugins;
  let written = 0;
  const watcherMs: number[] = [];
  const hotUpdateMs: number[] = [];
  const editedFile = path.join(root, input[0].moduleId).replaceAll("\\", "/");
  const server = await createServer({
    configFile: false,
    root,
    plugins: [
      {
        name: "watch-event-probe",
        enforce: "pre",
        hotUpdate(context) {
          if (this.environment.name === "client" && context.file === editedFile && written) {
            hotUpdateMs.push(performance.now() - written);
          }
        },
      },
      plugins,
    ],
    logLevel: "silent",
    optimizeDeps: {
      noDiscovery: true,
      include: [],
    },
    server: { host: "127.0.0.1", port: 0 },
  });
  try {
    await server.listen();
    server.watcher.on("change", (file) => {
      if (file.replaceAll("\\", "/") === editedFile && written) {
        watcherMs.push(performance.now() - written);
      }
    });
    const environment = server.environments.client;
    const transform =
      typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler;
    if (typeof transform !== "function") {
      expect.fail("Expected compiler transform hook");
    }
    const context = {
      environment,
      resolve: (specifier: string, importer: string) =>
        environment.pluginContainer.resolveId(specifier, importer),
      addWatchFile: (file: string) => server.watcher.add(file),
      warn: (message: string) => expect.fail(message),
    } as unknown as ThisParameterType<typeof transform>;
    const start = performance.now();
    for (const source of input) {
      await transform.call(
        context,
        source.document.code,
        path.join(root, source.moduleId).replaceAll("\\", "/"),
      );
    }
    await environment.transformRequest("/" + input[0].moduleId);
    task.meta.input = {
      modules: 1000,
      callsPerModule: 8,
      cssBytes: Buffer.byteLength(full),
      fileWatcher: true,
      hmrConnection: false,
      transport: "filesystem-event-to-css-hot-send",
    };
    task.meta.output = { preloadMs: performance.now() - start };
    task.meta.measurement = options;
    const originalSend = environment.hot.send.bind(environment.hot);
    let finished: (() => void) | undefined;
    const send = vi.spyOn(environment.hot, "send").mockImplementation((...args) => {
      originalSend(...args);
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
    let edit = 0;
    try {
      await bench("write-to-css-publish", { async: true }, async () => {
        const published = new Promise<void>((resolve, reject) => {
          const timeout = setTimeout(() => reject(new Error("CSS publication timed out")), 5000);
          finished = () => {
            clearTimeout(timeout);
            resolve();
          };
        });
        written = performance.now();
        writeFileSync(
          path.join(root, input[0].moduleId),
          input[0].document.code.replace("'flex'", ++edit % 2 ? "'block'" : "'flex'"),
        );
        await published;
        finished = undefined;
      }).run(options);
      expect(send).toHaveBeenCalled();
      task.meta.watcherMs = watcherMs.slice(options.warmupIterations);
      task.meta.hotUpdateMs = hotUpdateMs.slice(options.warmupIterations);
    } finally {
      send.mockRestore();
    }
  } finally {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

for (const [name, css, write] of [
  ["minimal", minimal, false],
  ["full-registry", full, false],
  ["utilities10000", definitionCss("utilities", 10_000), false],
  ["minimal", minimal, true],
  ["full-registry", full, true],
  ["utilities10000", definitionCss("utilities", 10_000), true],
] as const) {
  const title =
    "vite/full-build/1000/8-calls" +
    (name === "minimal" ? "" : `/${name}`) +
    (write ? "/disk-output" : "");
  test(title, async ({ bench, task }) => {
    const root = mkdtempSync(
      path.join(path.dirname(fileURLToPath(import.meta.url)), "../.test-full-build-"),
    );
    const input = sources(1000, 8);
    writeFileSync(path.join(root, "root.css"), css);
    writeFileSync(
      path.join(root, "index.html"),
      '<html><body><script type="module" src="/entry.ts"></script></body></html>',
    );
    for (const source of input) {
      writeFileSync(path.join(root, source.moduleId), source.document.code);
    }
    writeFileSync(
      path.join(root, "entry.ts"),
      input.map((source, i) => `import * as p${i} from './${source.moduleId}';`).join("\n") +
        `\nglobalThis.benchmark=[${input.map((_, i) => `p${i}`).join(",")}];`,
    );
    task.meta.input = {
      modules: 1000,
      callsPerModule: 8,
      cssBytes: Buffer.byteLength(css),
      designSystem: name,
      write,
      sourcemap: false,
      cssMinify: true,
    };
    const buildOptions = { ...options, iterations: 3, warmupIterations: 0 };
    task.meta.measurement = buildOptions;
    let result!: Awaited<ReturnType<typeof build>>;
    try {
      await bench("build", { async: true }, async () => {
        result = await build({
          configFile: false,
          root,
          plugins: [
            stillwind({
              css: "root.css",
            }),
          ],
          logLevel: "silent",
          build: {
            write,
            sourcemap: false,
          },
        });
      }).run(buildOptions);
      expect(result).toHaveProperty("output");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
}

for (const files of [100, 1000]) {
  test(`vite/css-files/${files}`, async ({ bench, task }) => {
    const root = mkdtempSync(
      path.join(path.dirname(fileURLToPath(import.meta.url)), "../.test-css-files-"),
    );
    writeFileSync(
      path.join(root, "root.css"),
      Array.from({ length: files }, (_, i) => `@import './file-${i}.css';`).join("\n") + minimal,
    );
    for (let i = 0; i < files; i++) {
      writeFileSync(path.join(root, `file-${i}.css`), `.rule-${i}{color:red;}`);
    }
    const listenMs: number[] = [];
    const watchReadyMs: number[] = [];
    const startServer = async () => {
      const start = performance.now();
      let watched!: () => void;
      const ready = new Promise<void>((resolve) => {
        watched = resolve;
      });
      const plugins = stillwind({
        css: "root.css",
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
          plugins,
        ],
        logLevel: "silent",
        optimizeDeps: {
          noDiscovery: true,
          include: [],
        },
        server: {
          host: "127.0.0.1",
          port: 0,
        },
      });
      await server.listen();
      listenMs.push(performance.now() - start);
      await ready;
      watchReadyMs.push(performance.now() - start);
      return { server, plugins };
    };
    task.meta.input = {
      cssFiles: files + 1,
      requestedJsModules: 1,
      callsPerModule: 1,
      dependencyDiscovery: false,
    };
    task.meta.measurement = options;
    try {
      await bench("create-listen-close", { async: true }, async () => {
        const { server } = await startServer();
        await server.close();
      }).run(options);
      task.meta.listenMs = listenMs.slice(options.warmupIterations);
      task.meta.watchReadyMs = watchReadyMs.slice(options.warmupIterations);
      const { server, plugins } = await startServer();
      const [plugin] = plugins;
      try {
        const transform =
          typeof plugin.transform === "function" ? plugin.transform : plugin.transform?.handler;
        if (typeof transform !== "function") {
          expect.fail("Expected compiler transform hook");
        }
        const context = {
          environment: server.environments.client,
          resolve: (specifier: string, importer: string) =>
            server.environments.client.pluginContainer.resolveId(specifier, importer),
          addWatchFile: () => {},
          warn: (message: string) => {
            expect.fail(message);
          },
        } as unknown as ThisParameterType<typeof transform>;
        const code = sources(1)[0].document.code;
        const id = path.join(root, "module.ts");
        await transform.call(context, code, id);
        let edit = 0;
        let result!: Awaited<ReturnType<typeof transform>>;
        await bench("edit-and-publish", { async: true }, async () => {
          result = await transform.call(
            context,
            code.replace("'flex'", ++edit % 2 ? "'block'" : "'flex'"),
            id,
          );
        }).run(options);
        expect(result).toHaveProperty("code");
      } finally {
        await server.close();
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
}

test("css-version-io/1000", async ({ bench, task }) => {
  const root = mkdtempSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), "../.test-css-version-"),
  );
  const files = Array.from({ length: 1000 }, (_, i) => path.join(root, `file-${i}.css`)).sort();
  for (const [i, file] of files.entries()) {
    writeFileSync(file, `.rule-${i}{color:red;}`);
  }
  task.meta.input = {
    cssFiles: files.length,
    contentHash: true,
    warmFilesystem: true,
  };
  task.meta.measurement = options;
  const fingerprints: string[] = [];
  try {
    for (const concurrency of [1, 16, 64, 256]) {
      let fingerprint = "";
      await bench(`read-concurrency-${concurrency}`, { async: true }, async () => {
        const hash = createHash("sha256");
        for (let start = 0; start < files.length; start += concurrency) {
          const batch = files.slice(start, start + concurrency);
          const contents = await Promise.all(batch.map((file) => readFile(file)));
          for (let i = 0; i < batch.length; i++) {
            hash.update(batch[i]).update("\0").update(contents[i]).update("\0");
          }
        }
        fingerprint = hash.digest("hex");
      }).run(options);
      fingerprints.push(fingerprint);
    }
    expect(new Set(fingerprints).size).toBe(1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function loader(css: string) {
  const loaded = new NativeDesignSystemLoader();
  loaded.provide(0, "root.css", css, "untitled:root.css", "untitled:root.css");
  return loaded;
}

function definitionCss(kind: "authored" | "tokens" | "utilities", count: number): string {
  const entries = Array.from({ length: count }, (_, i) =>
    kind === "authored"
      ? `.rule-${i}{color:rgb(${i % 256} 0 0);padding:${i % 32}px;}`
      : kind === "tokens"
        ? `--color-probe-${i}:rgb(${i % 256} 0 0);`
        : `@utility probe-${i} {color:rgb(${i % 256} 0 0);}`,
  );
  return minimal + (kind === "tokens" ? `@theme {${entries.join("\n")}}` : entries.join("\n"));
}

function sources(count: number, calls = 1): NativePreparedSource[] {
  return Array.from({ length: count }, (_, i) => ({
    kind: "direct",
    moduleId: `module-${i}.ts`,
    document: {
      uri: `untitled:module-${i}.ts`,
      sourceMapSource: `untitled:module-${i}.ts`,
      format: "jsx",
      code:
        "import {style} from 'stillwind/react';" +
        Array.from({ length: calls }, (_, j) => `export const props${j}=style('flex');`).join("\n"),
    },
    syntax: {
      language: "ts",
      moduleKind: "unambiguous",
    },
  }));
}
