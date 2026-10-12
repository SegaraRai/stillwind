import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { createServer, type ViteDevServer } from "vite";
import { expect, onTestFinished, test } from "vite-plus/test";

import { createVitePlugins } from "../../src/vite/plugin.ts";
import { createViteProject } from "../../src/vite/project.ts";

test("closing SSR keeps client virtual modules available", async () => {
  const { server, producer } = await fixture();
  const id = await producer();
  await server.environments.ssr.transformRequest("/entry.ts");
  await server.environments.ssr.pluginContainer.close();
  expect(await server.environments.client.pluginContainer.load(id)).toBeTruthy();
});

test("a failed restart preserves the old server's compiler state and permits recovery", async () => {
  const { server, producer, css, root } = await fixture({
    failRestart: true,
  });
  const client = server.environments.client;
  const id = await producer();
  await server.restart();
  expect(server.httpServer!.listening).toBe(true);
  expect(await client.pluginContainer.load(id)).toBeTruthy();
  expect(await css()).toMatch(/color:\s*red/);
  await writeFile(
    path.join(root, "root.css"),
    "@utility paint{color:blue} @stillwind-slot utilities;",
  );
  await server.restart();
  expect(await css()).toMatch(/color:\s*blue/);
});

test("middleware mode releases its CSS watches when the server closes", async () => {
  const { server, project } = await fixture({ middleware: true });
  expect(project.settings.cssWatching).toBe(true);
  await server.close();
  expect(project.settings.cssWatching).toBe(false);
});

/**
 * Starts an isolated server and registers cleanup for every restart candidate.
 * @param options - Enables middleware mode or rejects the first restart during server setup.
 * @returns The server, project, temporary root, and operations for reading generated modules and CSS.
 * @throws If server configuration or startup fails.
 */
async function fixture(options: { middleware?: boolean; failRestart?: boolean } = {}) {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-lifecycle-"));
  const candidates = new Set<ViteDevServer>();
  onTestFinished(async () => {
    for (const candidate of [...candidates].reverse()) {
      await candidate.close();
    }
    await rm(root, { recursive: true, force: true });
  });
  await writeFile(
    path.join(root, "stillwind.config.ts"),
    'export default {projects:[{css:"root.css",files:["entry.ts"]}]};',
  );
  await writeFile(
    path.join(root, "root.css"),
    "@utility paint{color:red} @stillwind-slot utilities;",
  );
  await writeFile(
    path.join(root, "entry.ts"),
    'import {style} from "stillwind/react"; export const props=style("paint");',
  );
  const project = createViteProject();
  let configured = 0;
  const server = await createServer({
    root,
    configFile: false,
    logLevel: "silent",
    optimizeDeps: {
      noDiscovery: true,
      include: [],
    },
    plugins: [
      createVitePlugins(project),
      {
        name: "capture-candidates",
        configureServer(candidate) {
          candidates.add(candidate);
          if (++configured === 2 && options.failRestart) {
            throw new Error("Rejected candidate server");
          }
        },
      },
    ],
    server: {
      host: "127.0.0.1",
      port: 0,
      hmr: false,
      middlewareMode: options.middleware,
    },
  });
  if (!options.middleware) {
    await server.listen();
  }
  const origin = () => {
    const address = server.httpServer!.address();
    if (!address || typeof address === "string") {
      expect.fail("Expected a TCP server");
    }
    return `http://127.0.0.1:${address.port}`;
  };
  return {
    root,
    project,
    server,
    producer: async () => {
      const result = await server.transformRequest("/entry.ts");
      const id = /\/@id\/__x00__stillwind:[^"'\s]+/.exec(result!.code)?.[0];
      expect(id).toBeTruthy();
      return id!.replace("/@id/__x00__", "\0");
    },
    css: async () => {
      const entry = await fetch(`${origin()}/entry.ts`);
      expect(entry.status).toBe(200);
      const response = await fetch(`${origin()}/__stillwind/style.css?environment=client`);
      const css = await response.text();
      expect(response.status, css).toBe(200);
      return css;
    },
  };
}
