import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { createServer } from "vite";
import { afterEach, describe, expect, onTestFinished, test, vi } from "vite-plus/test";

import { writeVocabulary } from "../options.ts";
import { createVitePlugins } from "./plugin.ts";
import { createViteProject } from "./project.ts";

vi.mock("node:fs/promises", async (importOriginal) => {
  const original = await importOriginal<typeof import("node:fs/promises")>();
  return {
    ...original,
    readFile: vi.fn(original.readFile),
  };
});

vi.mock("../options.ts", async (importOriginal) => {
  const original = await importOriginal<typeof import("../options.ts")>();
  return {
    ...original,
    writeVocabulary: vi.fn(original.writeVocabulary),
  };
});

afterEach(() => {
  vi.mocked(readFile).mockReset();
  vi.mocked(writeVocabulary).mockReset();
});

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
const definePaint = (color: string) =>
  `.environment { color: ${color}; } @utility paint { color: ${color}; }`;
const rootWithImport = "@import './part.css'; @stillwind-slot utilities;";

describe("CSS updates without HMR", () => {
  test("editing JavaScript does not reread unchanged client or SSR CSS", async () => {
    const fixture = await createEnvironmentProject();

    await fixture.compileProducer("client", "// JavaScript edit");

    expect(await fixture.getCss("client")).toMatch(/color:\s*red/);
    expect(await fixture.getCss("ssr")).toMatch(/color:\s*blue/);
    expect(cssFilesRead()).toEqual([]);
  });

  test.each([
    { environment: "client", updatedColor: "green", otherEnvironment: "ssr", otherColor: "blue" },
    { environment: "ssr", updatedColor: "purple", otherEnvironment: "client", otherColor: "red" },
  ])(
    "editing $environment CSS updates only that environment and shares one read between requests",
    async ({ environment, updatedColor, otherEnvironment, otherColor }) => {
      const fixture = await createEnvironmentProject();

      await fixture.writeCss(`${environment}.css`, definePaint(updatedColor));
      const [updatedCss, unchangedCss, concurrentCss] = await Promise.all([
        fixture.getCss(environment),
        fixture.getCss(otherEnvironment),
        fixture.getCss(environment),
      ]);

      expect(updatedCss).toMatch(new RegExp(`color:\\s*${updatedColor}`));
      expect(unchangedCss).toMatch(new RegExp(`color:\\s*${otherColor}`));
      expect(concurrentCss).toBe(updatedCss);
      expect(
        cssFilesRead().filter((file) => file === path.join(fixture.root, `${environment}.css`)),
      ).toHaveLength(1);
      expect(cssFilesRead()).not.toContain(path.join(fixture.root, `${otherEnvironment}.css`));
    },
  );

  test("closing the server stops CSS watch events from changing the project", async () => {
    const fixture = await createWatchedProject({ "root.css": definePaint("red") });
    const state = fixture.project.environment("client").designSystem;
    await fixture.server.close();
    const revisionAfterClose = state.cssChange;

    fixture.server.watcher.emit("all", "change", path.join(fixture.root, "root.css"));

    expect(state.cssChange).toBe(revisionAfterClose);
  });
});

describe("invalid or missing CSS imports", () => {
  test("invalid CSS keeps the last working CSS and error until the file is fixed", async () => {
    const fixture = await createWatchedProject({
      "root.css": rootWithImport,
      "part.css": definePaint("red"),
    });
    await fixture.waitUntilWatched("part.css");
    await fixture.compileProducer();
    const workingCss = await fixture.getCss();
    const state = fixture.project.environment("client").designSystem;

    await fixture.writeCss("part.css", "@utility paint {");
    expect(await fixture.getCss()).toBe(workingCss);
    const cssError = state.cssError;
    expect(cssError).toBeDefined();
    expect(state.cssApplied).toBeLessThan(state.cssChange);

    vi.mocked(readFile).mockClear();
    await fixture.compileProducer("client", "// edit while CSS is invalid");
    expect(await fixture.getCss()).toBe(workingCss);
    expect(state.cssError).toBe(cssError);
    expect(cssFilesRead()).toEqual([]);

    await fixture.writeCss("part.css", definePaint("green"));
    expect(await fixture.getCss()).toMatch(/color:\s*green/);
    expect(state.cssError).toBeUndefined();
  });

  test("creating a missing CSS import clears its error and adds its rules", async () => {
    const fixture = await createWatchedProject({
      "root.css": rootWithImport,
      "part.css": definePaint("green"),
    });
    await fixture.waitUntilWatched("root.css");
    const state = fixture.project.environment("client").designSystem;
    const addedFile = path.join(fixture.root, "added.css");

    await fixture.writeCss(
      "root.css",
      "@import './part.css'; @import './added.css'; @stillwind-slot utilities;",
    );
    expect(await fixture.getCss()).toMatch(/color:\s*green/);
    expect(state.cssError).toBeDefined();
    expect(state.cssFiles.has(addedFile)).toBe(false);

    await fixture.writeCss("added.css", ".addition { width: 11px; }");
    expect(await fixture.getCss()).toMatch(/width:\s*11px/);
    expect(state.cssFiles.has(addedFile)).toBe(true);
    expect(state.cssError).toBeUndefined();
  });

  test("removing a CSS import removes its rules and stops observing its file", async () => {
    const fixture = await createWatchedProject({
      "root.css": "@import './part.css'; @import './added.css'; @stillwind-slot utilities;",
      "part.css": definePaint("green"),
      "added.css": ".addition { width: 11px; }",
    });
    await fixture.waitUntilWatched("root.css");
    const state = fixture.project.environment("client").designSystem;
    const addedFile = path.join(fixture.root, "added.css");
    expect(await fixture.getCss()).toContain(".addition");

    await fixture.writeCss("root.css", rootWithImport);
    expect(await fixture.getCss()).not.toContain(".addition");
    const revisionAfterRemovingImport = state.cssChange;
    fixture.server.watcher.emit("all", "unlink", addedFile);
    expect(state.cssChange).toBe(revisionAfterRemovingImport);
  });

  test("deleting an imported file keeps working CSS until the file is recreated", async () => {
    const fixture = await createWatchedProject({
      "root.css": rootWithImport,
      "part.css": definePaint("green"),
    });
    await fixture.waitUntilWatched("part.css");
    const workingCss = await fixture.getCss();
    const state = fixture.project.environment("client").designSystem;

    await fixture.changeCss("part.css", () => rmSync(path.join(fixture.root, "part.css")));
    expect(await fixture.getCss()).toBe(workingCss);
    expect(state.cssError).toBeDefined();

    await fixture.writeCss("part.css", definePaint("yellow"));
    expect(await fixture.getCss()).toMatch(/color:\s*yellow/);
    expect(state.cssError).toBeUndefined();
  });

  test("creating a missing import sends updated CSS through HMR without a CSS request", async () => {
    const fixture = await createWatchedProject({ "root.css": definePaint("red") }, { hmr: true });
    await fixture.waitUntilWatched("root.css");
    const state = fixture.project.environment("client").designSystem;
    await fixture.writeCss("root.css", "@import './added.css';");
    await expect.poll(() => state.cssError).toBeDefined();
    expect(state.cssFiles.has(path.join(fixture.root, "added.css"))).toBe(false);
    const hmrSend = vi.spyOn(fixture.server.environments.client.hot, "send");
    onTestFinished(() => hmrSend.mockRestore());

    await fixture.writeCss("added.css", definePaint("green"));

    await vi.waitFor(() => {
      expect(state.cssError).toBeUndefined();
      expect(state.cssFiles.has(path.join(fixture.root, "added.css"))).toBe(true);
      expect(hmrSend).toHaveBeenCalledWith(
        expect.objectContaining({
          type: "custom",
          event: "stillwind:css",
          data: expect.objectContaining({
            parts: expect.arrayContaining([
              expect.objectContaining({
                css: expect.stringMatching(/color:\s*green/),
              }),
            ]),
          }),
        }),
      );
    });
  });
});

test("a CSS edit during a file read makes both waiting requests receive the newest CSS", async () => {
  const fixture = await createWatchedProject({ "root.css": definePaint("red") });
  await fixture.waitUntilWatched("root.css");
  writeFileSync(path.join(fixture.root, "added.css"), definePaint("blue"));
  await fixture.waitUntilWatched("added.css");
  const pausedRead = await pauseFirstRead(path.join(fixture.root, "added.css"));
  onTestFinished(pausedRead.resume);

  await fixture.writeCss("root.css", "@import './added.css';");
  const firstRequest = fixture.getCss();
  await pausedRead.started;
  const state = fixture.project.environment("client").designSystem;
  expect(state.cssFiles.has(path.join(fixture.root, "added.css"))).toBe(false);
  expect(state.cssObservedFiles.has(path.join(fixture.root, "added.css"))).toBe(true);

  await fixture.writeCss("added.css", definePaint("purple"));
  const secondRequest = fixture.getCss();
  pausedRead.resume();
  const [firstCss, secondCss] = await Promise.all([firstRequest, secondRequest]);

  expect(firstCss).toMatch(/color:\s*purple/);
  expect(secondCss).toBe(firstCss);
  expect(
    cssFilesRead().filter((file) => file === path.join(fixture.root, "added.css")),
  ).toHaveLength(2);
  expect(state.cssApplied).toBe(state.cssChange);
  const readsAfterUpdate = cssFilesRead().length;
  expect(await fixture.getCss()).toBe(firstCss);
  expect(cssFilesRead()).toHaveLength(readsAfterUpdate);
});

test("an accepted CSS reload invalidates SSR producers even when writing vocabulary fails", async () => {
  const fixture = await createWatchedProject(
    { "root.css": definePaint("red") + " @stillwind-slot utilities;" },
    { types: "generated.d.ts" },
  );
  await fixture.server.environments.ssr.pluginContainer.buildStart({});
  await fixture.waitUntilWatched("root.css");
  const producer = path.join(fixture.root, "producer.ts").replaceAll("\\", "/");
  writeFileSync(
    producer,
    "import { style } from 'stillwind/react'; export const props = style('paint');",
  );
  await fixture.server.environments.ssr.transformRequest("/producer.ts");
  const graph = fixture.server.environments.ssr.moduleGraph;
  const module = graph.getModuleById(producer);
  expect(module).toBeDefined();
  const invalidate = vi.spyOn(graph, "invalidateModule");
  onTestFinished(() => invalidate.mockRestore());
  vi.mocked(writeVocabulary).mockImplementationOnce(() => {
    throw new Error("Cannot write vocabulary");
  });
  fixture.project.options.classNamespace = "updated";

  await fixture.writeCss("root.css", definePaint("green") + " @stillwind-slot utilities;");
  const css = await fixture.getCss("ssr");

  expect(css).toMatch(/color:\s*green/);
  expect(fixture.project.environment("ssr").designSystem.cssError).toBeDefined();
  expect(invalidate).toHaveBeenCalledWith(module);
  expect(fixture.project.environment("ssr").takeThemeChanges()).toContain("producer.ts");
});

test("deleting a producer while its transform waits for CSS removes the completed input", async () => {
  const fixture = await createWatchedProject({
    "root.css": "@utility paint { color: red; } @stillwind-slot utilities;",
  });
  await fixture.waitUntilWatched("root.css");
  await fixture.compileProducer();
  expect(await fixture.getCss()).toMatch(/color:\s*red/);
  await fixture.writeCss("root.css", "@utility paint { color: green; } @stillwind-slot utilities;");
  const pausedRead = await pauseFirstRead(path.join(fixture.root, "root.css"));
  onTestFinished(pausedRead.resume);

  const transform = fixture.compileProducer();
  await pausedRead.started;
  const removal = fixture.project.environment().removeFile(path.join(fixture.root, "producer.ts"));
  pausedRead.resume();
  await Promise.all([transform, removal]);

  expect(fixture.project.inputs().size).toBe(0);
  expect(await fixture.getCss()).not.toMatch(/color:\s*(red|green)/);
});

async function pauseFirstRead(file: string) {
  const original = await vi.importActual<typeof import("node:fs/promises")>("node:fs/promises");
  const started = Promise.withResolvers<void>();
  const resume = Promise.withResolvers<void>();
  let shouldPause = true;
  vi.mocked(readFile).mockClear();
  vi.mocked(readFile).mockImplementation(async (...args) => {
    const content = await original.readFile(...args);
    if (args[0] === file && shouldPause) {
      shouldPause = false;
      started.resolve();
      await resume.promise;
    }
    return content;
  });
  return {
    started: started.promise,
    resume: () => resume.resolve(),
  };
}

async function createWatchedProject(
  files: Record<string, string>,
  options: {
    environmentCss?: boolean;
    hmr?: boolean;
    types?: false | string;
  } = {},
) {
  const root = mkdtempSync(path.join(repositoryRoot, ".test-css-watch-"));
  for (const [filename, content] of Object.entries(files)) {
    writeFileSync(path.join(root, filename), content);
  }
  const project = createViteProject({ css: "root.css", types: options.types ?? false });
  const plugins = createVitePlugins(project);
  const [plugin] = plugins;
  const server = await createServer({
    configFile: false,
    root,
    plugins: [
      {
        name: "environment-css",
        resolveId(id) {
          if (options.environmentCss && id === "environment-css") {
            return path.join(root, this.environment.name === "ssr" ? "ssr.css" : "client.css");
          }
          return undefined;
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
      hmr: options.hmr ?? false,
    },
  });
  onTestFinished(async () => {
    await server.close();
    rmSync(root, { recursive: true, force: true });
  });
  await server.listen();
  const context = (environmentName: string) => {
    const environment = server.environments[environmentName];
    return {
      environment,
      resolve: (specifier: string, importer: string) =>
        environment.pluginContainer.resolveId(specifier, importer),
      addWatchFile: (file: string) => server.watcher.add(file),
      warn: (message: string) => expect.fail(message),
    };
  };
  return {
    root,
    project,
    server,
    getCss: async (environment = "client") =>
      (await project.documentPlan(environment, context(environment))).css,
    async compileProducer(environment = "client", suffix = "") {
      const transform = plugin.transform;
      if (!transform || typeof transform === "function" || !transform.handler) {
        expect.fail("Expected the compiler's transform handler");
      }
      return transform.handler.call(
        context(environment) as unknown as ThisParameterType<typeof transform.handler>,
        "import { style } from 'stillwind/react'; export const props = style('paint');" + suffix,
        path.join(root, "producer.ts"),
      );
    },
    async waitUntilWatched(filename: string) {
      await expect
        .poll(() => Object.values(server.watcher.getWatched()).flat())
        .toContain(filename);
    },
    async changeCss(filename: string, change: () => void) {
      const file = path.join(root, filename);
      const states = [
        project.environment("client").designSystem,
        project.environment("ssr").designSystem,
      ];
      const previousRevisions = states.map((state) => state.cssChange);
      change();
      await expect
        .poll(() =>
          states.some(
            (state, index) =>
              state.cssObservedFiles.has(file) && state.cssChange > previousRevisions[index],
          ),
        )
        .toBe(true);
    },
    async writeCss(filename: string, content: string) {
      await this.changeCss(filename, () => {
        writeFileSync(path.join(root, filename), content);
      });
    },
  };
}

async function createEnvironmentProject() {
  const fixture = await createWatchedProject(
    {
      "root.css": '@import "environment-css"; @stillwind-slot utilities;',
      "client.css": definePaint("red"),
      "ssr.css": definePaint("blue"),
    },
    {
      environmentCss: true,
    },
  );
  await fixture.server.environments.ssr.pluginContainer.buildStart({});
  await fixture.waitUntilWatched("client.css");
  await fixture.waitUntilWatched("ssr.css");
  await Promise.all([fixture.compileProducer("client"), fixture.compileProducer("ssr")]);
  vi.mocked(readFile).mockClear();
  return fixture;
}

function cssFilesRead() {
  return vi
    .mocked(readFile)
    .mock.calls.map(([file]) => file)
    .filter((file): file is string => typeof file === "string" && file.endsWith(".css"));
}
