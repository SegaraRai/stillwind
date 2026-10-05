import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { createServer, type ViteDevServer } from "vite";
import { expect, onTestFinished } from "vite-plus/test";

import { compilerPlugin } from "../../src/vite/plugin.ts";
import { createViteProject } from "../../src/vite/project.ts";

export const packageRoot = fileURLToPath(new URL("../../", import.meta.url));
export const redStyleSource = `
import { style } from 'stillwind/react';
export const props = style('red');
`;

export async function createTestServer() {
  const root = await mkdtemp(path.join(packageRoot, ".test-security-"));
  let server: ViteDevServer | undefined;
  onTestFinished(async () => {
    try {
      await server?.close();
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
  await writeFile(
    path.join(root, "root.css"),
    "@utility red{color:red} @stillwind-slot utilities;",
  );
  const project = createViteProject({
    css: "root.css",
    types: false,
  });
  const plugin = compilerPlugin(project);
  server = await createServer({
    configFile: false,
    root,
    appType: "custom",
    plugins: [plugin],
    logLevel: "silent",
    css: {
      devSourcemap: true,
    },
    optimizeDeps: {
      noDiscovery: true,
      include: [],
    },
    server: {
      host: "127.0.0.1",
      port: 0,
      watch: null,
      hmr: false,
      fs: {
        allow: [root],
        deny: ["**/blocked.ts"],
      },
    },
  });
  await server.listen();
  const address = server.httpServer!.address();
  if (!address || typeof address === "string") {
    expect.fail("Expected Vite to listen on a TCP port");
  }
  return {
    root,
    project,
    server,
    origin: `http://127.0.0.1:${address.port}`,
  };
}

export function compileContext(server: ViteDevServer, environmentName: "client" | "ssr") {
  const environment = server.environments[environmentName];
  return {
    resolve: (specifier: string, importer: string) =>
      environment.pluginContainer.resolveId(specifier, importer),
    addWatchFile: (file: string) => {
      server.watcher.add(file);
    },
    warn: (message: string) => expect.fail(message),
  };
}

export function normalizeFile(file: string) {
  return file.replaceAll("\\", "/");
}

export function internalModuleId(reference: string) {
  return reference.replace("virtual:stillwind/", "\0stillwind:");
}
