import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { createServer, type ViteDevServer } from "vite";
import { expect, onTestFinished, test } from "vite-plus/test";

import { createVitePlugins } from "../../src/vite/plugin.ts";
import { createViteProject } from "../../src/vite/project.ts";

test.each([
  { name: "React Router export scan", route: true, main: false, watched: false, compiled: false },
  { name: "React Router application", route: true, main: true, watched: false, compiled: true },
  {
    name: "React Router development export scan",
    route: true,
    main: false,
    watched: true,
    compiled: false,
  },
  { name: "ordinary unwatched host", route: false, main: false, watched: false, compiled: true },
])("$name preserves its style compilation contract", async ({ route, main, watched, compiled }) => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-route-exports-"));
  let server: ViteDevServer | undefined = undefined;
  onTestFinished(async () => {
    await server?.close();
    await rm(root, { recursive: true, force: true });
  });
  await writeFile(
    path.join(root, "root.css"),
    "@utility flex{display:flex} @stillwind-slot utilities;",
  );
  const code =
    'import {style} from "stillwind/react"; export const props=style("flex"); export function loader(){return 1;}';
  const file = path.join(root, "route.tsx");
  await writeFile(file, code);
  const project = createViteProject({
    css: "root.css",
    configFile: false,
  });
  server = await createServer({
    root,
    configFile: false,
    logLevel: "silent",
    server: {
      middlewareMode: true,
      watch: watched ? {} : null,
    },
    optimizeDeps: {
      noDiscovery: true,
      include: [],
    },
    plugins: [
      createVitePlugins(project),
      ...(route ? [{ name: "react-router:virtual-modules" }] : []),
      ...(main ? [{ name: "react-router" }] : []),
    ],
  });
  // React Router transforms route buffers directly and reads their exports without evaluating them.
  const container = server.environments.ssr.pluginContainer;
  await container.buildStart({});
  const module = await server.environments.ssr.moduleGraph.ensureEntryFromUrl("/route.tsx");
  const result = await container.transform(code, module.id!);
  expect(result?.code).toContain("loader");
  expect(project.inputs("ssr").size).toBe(compiled ? 1 : 0);
  expect(/\bstyle\(["']flex["']\)/.test(result?.code ?? "")).toBe(!compiled);
});
