import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { AnalysisWorkspace } from "@stillwind/host";
import { stillwind } from "@stillwind/integration/vite";
import { build, createServer } from "vite";
import { expect, test } from "vite-plus/test";

test("build shares CSS and naming with analysis while analysis enforces lint settings", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-shared-config-"));
  let workspace: AnalysisWorkspace | undefined;
  try {
    await writeFile(
      path.join(root, "stillwind.config.mts"),
      'import {lintPresets} from "stillwind/config"; export default async () => ({projects:[{css:"root.css",files:["entry.ts"],classNamespace:"shared-project",prefix:"--project-",lint:{extends:[lintPresets.strict]}}]});',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red} @stillwind-slot utilities; @stillwind-slot keyframes;",
    );
    await writeFile(
      path.join(root, "entry.ts"),
      'import {style} from "stillwind/react";import {frames} from "./Tokens";export const props=style("red bogus",frames(""));',
    );
    await writeFile(
      path.join(root, "Tokens.ts"),
      'import {style} from "stillwind/react";export const frames=style.keyframes({from:"red",to:"red"});',
    );
    workspace = await AnalysisWorkspace.open(path.join(root, "stillwind.config.mts"));
    const [analysis] = await workspace.lint();
    expect(analysis.result.status, JSON.stringify(analysis)).toBe("complete");
    expect(analysis.result.reports).toContainEqual(
      expect.objectContaining({
        code: "unknown-utility",
        severity: "error",
      }),
    );
    const compiled = analysis.result.graph!.modules.find(
      (module) => module.moduleId === "entry.ts",
    )!;
    expect(compiled.classes.length).toBeGreaterThan(0);
    expect(compiled.lintSettings["unknown-utility"]).toBe("deny");
    const compile = () =>
      build({
        root,
        configFile: false,
        plugins: [stillwind()],
        build: {
          write: false,
          minify: false,
          cssMinify: false,
          lib: {
            entry: path.join(root, "entry.ts"),
            formats: ["es"],
          },
        },
      });
    const result = await compile();
    const outputs = (Array.isArray(result) ? result : [result]).flatMap((result) => {
      if (!("output" in result)) {
        expect.fail("Expected build output");
      }
      return result.output;
    });
    const css = outputs
      .flatMap((output) =>
        output.type === "asset" && output.fileName.endsWith(".css") ? [String(output.source)] : [],
      )
      .join("\n");
    for (const generated of compiled.classes) {
      expect(css).toContain(`.${generated.className}`);
    }
    expect(compiled.symbols.length).toBeGreaterThan(0);
    for (const symbol of compiled.symbols) {
      expect(css).toContain(symbol.name);
    }
    expect(css).toMatch(/color:\s*red/);
    await writeFile(path.join(root, "root.css"), "@theme {");
    await expect(compile()).rejects.toThrow();
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true, maxRetries: 5 });
  }
});

test("production watch reloads an imported configuration helper", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-watch-"));
  const helper = path.join(root, "settings.ts");
  await writeFile(
    path.join(root, "stillwind.config.mts"),
    "import settings from './settings.ts'; export default () => settings;",
  );
  const settings = (css: string) =>
    `export default {projects:[{css:${JSON.stringify(css)},files:['*.ts']}]};`;
  await writeFile(helper, settings("red.css"));
  for (const color of ["red", "blue"]) {
    await writeFile(
      path.join(root, `${color}.css`),
      `@utility paint{color:${color}} @stillwind-slot utilities;`,
    );
  }
  await writeFile(
    path.join(root, "entry.ts"),
    'import css from "stillwind/module-css"; import {style} from "stillwind/react"; globalThis.props=style("paint");globalThis.css=css;',
  );
  const watcher = await build({
    root,
    configFile: false,
    plugins: [stillwind()],
    logLevel: "silent",
    build: {
      watch: {},
      minify: false,
      lib: {
        entry: path.join(root, "entry.ts"),
        formats: ["es"],
      },
    },
  });
  if (Array.isArray(watcher) || !("on" in watcher)) {
    expect.fail("Expected a build watcher");
  }
  let builds = 0;
  let error: Error | undefined;
  watcher.on("event", (event) => {
    if (event.code === "END") {
      builds++;
    }
    if (event.code === "ERROR") {
      error = event.error;
    }
  });
  const wait = (count: number) =>
    expect
      .poll(
        () => {
          if (error) {
            throw error;
          }
          return builds;
        },
        {
          timeout: 10_000,
        },
      )
      .toBeGreaterThanOrEqual(count);
  const output = async () => {
    const directory = path.join(root, "dist");
    const files = (await readdir(directory)).filter((file) => file.endsWith(".js"));
    return (
      await Promise.all(files.map((file) => readFile(path.join(directory, file), "utf8")))
    ).join("\n");
  };
  try {
    await wait(1);
    expect(await output()).toMatch(/color:\s*red/);
    await writeFile(helper, settings("blue.css"));
    await wait(2);
    expect(await output()).toMatch(/color:\s*blue/);
    await writeFile(helper, settings("red.css"));
    await wait(3);
    expect(await output()).toMatch(/color:\s*red/);
  } finally {
    await watcher.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("development reloads the project when an imported configuration helper changes", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-dev-"));
  const helper = path.join(root, "settings.ts");
  await writeFile(
    path.join(root, "stillwind.config.ts"),
    "import settings from './settings.ts'; export default settings;",
  );
  const settings = (css: string) =>
    `export default {projects:[{css:${JSON.stringify(css)},files:['*.ts']}]};`;
  await writeFile(helper, settings("red.css"));
  for (const color of ["red", "blue"]) {
    await writeFile(
      path.join(root, `${color}.css`),
      `@utility paint{color:${color}} @stillwind-slot utilities;`,
    );
  }
  await writeFile(
    path.join(root, "entry.ts"),
    'import {style} from "stillwind/react"; export const props=style("paint");',
  );
  const server = await createServer({
    root,
    configFile: false,
    plugins: [stillwind()],
    logLevel: "silent",
    optimizeDeps: {
      noDiscovery: true,
      include: [],
    },
    server: {
      host: "127.0.0.1",
      port: 0,
      hmr: false,
    },
  });
  try {
    await server.listen();
    const address = server.httpServer!.address();
    if (!address || typeof address === "string") {
      expect.fail("Expected a TCP server");
    }
    const origin = `http://127.0.0.1:${address.port}`;
    const css = async () => {
      await fetch(`${origin}/entry.ts`);
      return (await fetch(`${origin}/__stillwind/style.css?environment=client`)).text();
    };
    expect(await css()).toMatch(/color:\s*red/);
    for (const color of ["blue", "red"]) {
      await writeFile(helper, settings(`${color}.css`));
      await expect
        .poll(
          async () => {
            try {
              return await css();
            } catch {
              return "";
            }
          },
          {
            timeout: 10_000,
          },
        )
        .toMatch(new RegExp(`color:\\s*${color}`));
    }
  } finally {
    await server.close();
    await rm(root, { recursive: true, force: true });
  }
});
