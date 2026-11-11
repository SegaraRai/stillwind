import { writeFile } from "node:fs/promises";
import path from "node:path";

import type { EnvironmentModuleNode } from "vite";
import { expect, test } from "vite-plus/test";

import { createTestServer, normalizeFile } from "./vite-server-fixture.ts";

test("an edit of a dependency updates only the producers whose JavaScript changes", async () => {
  const { root, project, server } = await createTestServer(
    {},
    "@utility red{color:red} @utility blue{color:blue} @stillwind-slot utilities;",
    // A watching server lets compilations share sources between changes. The test reports each
    // change itself, so the watcher ignores every file.
    { server: { watch: { ignored: () => true } } },
  );
  const dependency = normalizeFile(path.join(root, "base.ts"));
  const consumer = normalizeFile(path.join(root, "app.tsx"));
  await writeFile(
    dependency,
    'import { style } from "stillwind/react"; export const base = style("red");',
  );
  await writeFile(
    consumer,
    'import { style } from "stillwind/react"; import { base } from "./base"; export const props = style(base, "blue");',
  );
  const environment = server.environments.ssr;
  await environment.pluginContainer.buildStart({});
  await environment.transformRequest("/base.ts");
  await environment.transformRequest("/app.tsx");
  const state = project.environment("ssr");
  const compile = state.compile.bind(state);
  const compiled: string[] = [];
  state.compile = (code, id, context) => {
    compiled.push(normalizeFile(id));
    return compile(code, id, context);
  };
  const app = environment.moduleGraph.getModuleById(consumer);
  if (!app) {
    expect.fail("Expected the producer in the module graph");
  }
  const hot = environment.plugins.find((plugin) => plugin.hotUpdate)!.hotUpdate;
  if (typeof hot !== "function") {
    expect.fail("Expected the HMR hook");
  }
  const warnings: string[] = [];
  /** Writes the dependency and runs the hook as a watched server would after the change. */
  const edit = async (code: string) => {
    compiled.length = 0;
    await writeFile(dependency, code);
    project.environment("ssr").changed(dependency, false);
    const modules = await hot.call(
      {
        environment,
        warn: (message: string) => warnings.push(message),
      } as unknown as ThisParameterType<typeof hot>,
      {
        type: "update",
        file: dependency,
        timestamp: Date.now(),
        modules: [],
        read: async () => code,
        server,
      },
    );
    return (modules ?? []) as EnvironmentModuleNode[];
  };

  // Class names do not depend on the utilities of the declaring module, so the importer keeps its
  // output without being compiled again.
  expect(
    await edit('import { style } from "stillwind/react"; export const base = style("blue");'),
  ).not.toContain(app);
  expect(compiled).toStrictEqual([dependency]);
  expect(warnings).toStrictEqual([]);
  expect(
    await edit('import { style } from "stillwind/react"; export const other = style("blue");'),
  ).toContain(app);
  expect(compiled).toContain(consumer);
});
