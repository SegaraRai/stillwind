import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { createServer, type Plugin, type ViteDevServer } from "vite-plus";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { compilerPlugin } from "./plugin.ts";
import { createViteProject } from "./project.ts";

vi.mock("node:fs/promises", async (importOriginal) => {
  const original = await importOriginal<typeof import("node:fs/promises")>();
  return { ...original, readFile: vi.fn(original.readFile) };
});

afterEach(() => {
  vi.mocked(readFile).mockReset();
});

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
const definition = (color: string) =>
  `.environment {color:${color};} @utility paint {color:${color};}`;
const cssReads = () =>
  vi.mocked(readFile).mock.calls.filter(([file]) => String(file).endsWith(".css"));

async function fixture(root: string, plugins: Plugin[] = [], hmr = false) {
  const project = createViteProject({ css: "root.css", types: false });
  const plugin = compilerPlugin(project);
  const server = await createServer({
    configFile: false,
    root,
    plugins: [...plugins, plugin],
    logLevel: "silent",
    optimizeDeps: { noDiscovery: true, include: [] },
    server: { host: "127.0.0.1", port: 0, hmr },
  });
  await server.listen();
  const context = (name: string) => ({
    environment: server.environments[name],
    resolve: (specifier: string, importer: string) =>
      server.environments[name].pluginContainer.resolveId(specifier, importer),
    addWatchFile: (file: string) => server.watcher.add(file),
    warn: (message: string) => expect.fail(message),
  });
  const css = async (name = "client") => (await project.documentPlan(name, context(name))).css;
  const compile = async (name = "client", suffix = "") => {
    const transform = plugin.transform;
    if (!transform || typeof transform === "function" || !transform.handler) {
      expect.fail("expected a transform handler");
    }
    return transform.handler.call(
      context(name) as unknown as ThisParameterType<typeof transform.handler>,
      "import {style} from 'stillwind/react'; export const props=style('paint');" + suffix,
      path.join(root, "producer.ts"),
    );
  };
  return { project, server, css, compile };
}

async function watched(server: ViteDevServer, file: string) {
  await vi.waitFor(() => {
    expect(Object.values(server.watcher.getWatched()).flat()).toContain(path.basename(file));
  });
}

async function changed(f: Awaited<ReturnType<typeof fixture>>, file: string, write: () => void) {
  const states = ["client", "ssr"].map((name) => f.project.environmentState(name));
  const before = states.map((state) => state.cssChange);
  write();
  await vi.waitFor(() => {
    expect(
      states.some(
        (state, i) => state.cssObservedFiles.has(path.resolve(file)) && state.cssChange > before[i],
      ),
    ).toBe(true);
  });
}

test("watch events refresh independent client and SSR snapshots without HMR or unchanged CSS reads", async () => {
  const root = mkdtempSync(path.join(repositoryRoot, ".test-css-watch-"));
  const clientCss = path.join(root, "client.css");
  const ssrCss = path.join(root, "ssr.css");
  writeFileSync(
    path.join(root, "root.css"),
    '@import "environment-css"; @stillwind-slot utilities;',
  );
  writeFileSync(clientCss, definition("red"));
  writeFileSync(ssrCss, definition("blue"));
  const f = await fixture(root, [
    {
      name: "environment-css",
      resolveId(id) {
        if (id === "environment-css") {
          return this.environment.name === "ssr" ? ssrCss : clientCss;
        }
      },
    },
  ]);
  try {
    await f.server.environments.ssr.pluginContainer.buildStart({});
    await watched(f.server, clientCss);
    await watched(f.server, ssrCss);
    await Promise.all([f.compile("client"), f.compile("ssr")]);
    vi.mocked(readFile).mockClear();
    await f.compile("client", "// edit");
    expect(await f.css("client")).toMatch(/color:\s*red/);
    expect(await f.css("ssr")).toMatch(/color:\s*blue/);
    expect(cssReads()).toHaveLength(0);
    await changed(f, clientCss, () => writeFileSync(clientCss, definition("green")));
    const results = await Promise.all([f.css("client"), f.css("ssr"), f.css("client")]);
    expect(results[0]).toMatch(/color:\s*green/);
    expect(results[1]).toMatch(/color:\s*blue/);
    expect(results[2]).toBe(results[0]);
    expect(cssReads().filter(([file]) => String(file) === clientCss)).toHaveLength(1);
    expect(cssReads().filter(([file]) => String(file) === ssrCss)).toHaveLength(0);
    await changed(f, ssrCss, () => writeFileSync(ssrCss, definition("purple")));
    expect(await f.css("ssr")).toMatch(/color:\s*purple/);
    expect(await f.css("client")).toMatch(/color:\s*green/);
  } finally {
    await f.server.close();
    const state = f.project.environmentState("client");
    const revision = state.cssChange;
    f.server.watcher.emit("all", "change", clientCss);
    expect(state.cssChange).toBe(revision);
    rmSync(root, { recursive: true, force: true });
  }
});

test("failed revisions keep CSS and diagnostics until another event, including missing and removed imports", async () => {
  const root = mkdtempSync(path.join(repositoryRoot, ".test-css-watch-"));
  const entry = path.join(root, "root.css");
  const part = path.join(root, "part.css");
  const added = path.join(root, "added.css");
  writeFileSync(entry, "@import './part.css'; @stillwind-slot utilities;");
  writeFileSync(part, definition("red"));
  const f = await fixture(root);
  try {
    await watched(f.server, part);
    await f.compile();
    const initial = await f.css();
    const state = f.project.environmentState("client");
    await changed(f, part, () => writeFileSync(part, "@utility paint {"));
    expect(await f.css()).toBe(initial);
    const failure = state.cssError;
    expect(failure).toBeDefined();
    expect(state.cssApplied).toBeLessThan(state.cssChange);
    vi.mocked(readFile).mockClear();
    await f.compile("client", "// during invalid CSS");
    expect(await f.css()).toBe(initial);
    expect(state.cssError).toBe(failure);
    expect(cssReads()).toHaveLength(0);
    await changed(f, part, () => writeFileSync(part, definition("green")));
    expect(await f.css()).toMatch(/color:\s*green/);
    expect(state.cssError).toBeUndefined();

    await changed(f, entry, () =>
      writeFileSync(
        entry,
        "@import './part.css'; @import './added.css'; @stillwind-slot utilities;",
      ),
    );
    expect(await f.css()).toMatch(/color:\s*green/);
    expect(state.cssError).toBeDefined();
    expect(state.cssFiles.has(added)).toBe(false);
    await changed(f, added, () => writeFileSync(added, ".addition {width:11px;}"));
    expect(await f.css()).toMatch(/width:\s*11px/);
    expect(state.cssFiles.has(added)).toBe(true);
    expect(state.cssError).toBeUndefined();
    await changed(f, entry, () =>
      writeFileSync(entry, "@import './part.css'; @stillwind-slot utilities;"),
    );
    expect(await f.css()).not.toContain(".addition");
    const revision = state.cssChange;
    f.server.watcher.emit("all", "unlink", added);
    expect(state.cssChange).toBe(revision);
    await changed(f, part, () => rmSync(part));
    expect(await f.css()).toMatch(/color:\s*green/);
    expect(state.cssError).toBeDefined();
    await changed(f, part, () => writeFileSync(part, definition("yellow")));
    expect(await f.css()).toMatch(/color:\s*yellow/);
    expect(state.cssError).toBeUndefined();
  } finally {
    await f.server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test("a change to a newly watched import during reading retries before publishing to concurrent requests", async () => {
  const root = mkdtempSync(path.join(repositoryRoot, ".test-css-watch-"));
  const entry = path.join(root, "root.css");
  const added = path.join(root, "added.css");
  writeFileSync(entry, definition("red"));
  const f = await fixture(root);
  let release!: () => void;
  try {
    await watched(f.server, entry);
    writeFileSync(added, definition("blue"));
    await watched(f.server, added);
    const original = await vi.importActual<typeof import("node:fs/promises")>("node:fs/promises");
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    let started!: () => void;
    const reading = new Promise<void>((resolve) => {
      started = resolve;
    });
    let hold = true;
    vi.mocked(readFile).mockClear();
    vi.mocked(readFile).mockImplementation(async (...args) => {
      const content = await original.readFile(...args);
      if (String(args[0]) === added && hold) {
        hold = false;
        started();
        await gate;
      }
      return content;
    });
    await changed(f, entry, () => writeFileSync(entry, "@import './added.css';"));
    const older = f.css();
    await reading;
    const state = f.project.environmentState("client");
    expect(state.cssFiles.has(added)).toBe(false);
    expect(state.cssObservedFiles.has(added)).toBe(true);
    await changed(f, added, () => writeFileSync(added, definition("purple")));
    const newer = f.css();
    release();
    const results = await Promise.all([older, newer]);
    expect(results[0]).toMatch(/color:\s*purple/);
    expect(results[1]).toBe(results[0]);
    expect(cssReads().filter(([file]) => String(file) === added)).toHaveLength(2);
    expect(state.cssApplied).toBe(state.cssChange);
    const count = cssReads().length;
    expect(await f.css()).toBe(results[0]);
    expect(cssReads()).toHaveLength(count);
  } finally {
    release?.();
    await f.server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test("adding a missing CSS import publishes the recovered revision without a stylesheet request", async () => {
  const root = mkdtempSync(path.join(repositoryRoot, ".test-css-watch-"));
  const entry = path.join(root, "root.css");
  const added = path.join(root, "added.css");
  writeFileSync(entry, definition("red"));
  const f = await fixture(root, [], true);
  try {
    await watched(f.server, entry);
    const state = f.project.environmentState("client");
    await changed(f, entry, () => writeFileSync(entry, "@import './added.css';"));
    await vi.waitFor(() => expect(state.cssError).toBeDefined());
    expect(state.cssFiles.has(added)).toBe(false);
    const send = vi.spyOn(f.server.environments.client.hot, "send");
    try {
      await changed(f, added, () => writeFileSync(added, definition("green")));
      await vi.waitFor(() => {
        expect(state.cssError).toBeUndefined();
        expect(state.cssFiles.has(added)).toBe(true);
        expect(send).toHaveBeenCalledWith(
          expect.objectContaining({
            type: "custom",
            event: "stillwind:css",
            data: expect.objectContaining({
              parts: expect.arrayContaining([
                expect.objectContaining({ css: expect.stringMatching(/color:\s*green/) }),
              ]),
            }),
          }),
        );
      });
    } finally {
      send.mockRestore();
    }
  } finally {
    await f.server.close();
    rmSync(root, { recursive: true, force: true });
  }
});
