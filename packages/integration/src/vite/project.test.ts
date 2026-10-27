import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import type { StillwindPlugin } from "@stillwind/config";
import { createServer, type ResolvedConfig } from "vite";
import { afterEach, describe, expect, onTestFinished, test, vi } from "vite-plus/test";

import { createVitePlugins } from "./plugin.ts";
import { createViteProject } from "./project.ts";

vi.mock("node:fs/promises", async (importOriginal) => {
  const original = await importOriginal<typeof import("node:fs/promises")>();
  return {
    ...original,
    readFile: vi.fn(original.readFile),
  };
});

afterEach(() => {
  vi.mocked(readFile).mockReset();
});

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");

/**
 * Creates CSS that exposes a revision through both an ordinary rule and a utility.
 * @param color - CSS color inserted directly into both declarations.
 * @returns CSS defining `.environment` and the `paint` utility with the supplied color.
 */
const definePaint = (color: string) =>
  `.environment { color: ${color}; } @utility paint { color: ${color}; }`;
const rootWithImport = "@import './part.css'; @stillwind-slot utilities;";

test("invalid emission settings fail before opening compiler environments", async () => {
  const project = createViteProject({
    configFile: false,
    emission: "unknown" as "single",
  });
  await expect(
    project.configure({
      root: repositoryRoot,
    } as ResolvedConfig),
  ).rejects.toThrow("config.emission: must be single or split");
});

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
    expect(fixture.takeWarnings()).toEqual([
      expect.stringContaining("Stillwind update failed; retaining published styles"),
    ]);

    vi.mocked(readFile).mockClear();
    await fixture.compileProducer("client", "// edit while CSS is invalid");
    expect(await fixture.getCss()).toBe(workingCss);
    expect(state.cssError).toBe(cssError);
    expect(cssFilesRead()).toEqual([]);

    await fixture.writeCss("part.css", definePaint("green"));
    expect(await fixture.getCss()).toMatch(/color:\s*green/);
    expect(state.cssError).toBeUndefined();
  });

  test("a missing CSS import is skipped with a warning until the file is created", async () => {
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
    expect(state.cssError).toBeUndefined();
    expect(state.cssFiles.has(addedFile)).toBe(false);
    expect(fixture.takeWarnings()).toEqual([
      `stillwind: invalid-css-import: \`./added.css\` cannot be resolved or read (${path.join(fixture.root, "root.css")}:1:23)`,
    ]);

    await fixture.writeCss("added.css", ".addition { width: 11px; }");
    expect(await fixture.getCss()).toMatch(/width:\s*11px/);
    expect(state.cssFiles.has(addedFile)).toBe(true);
    expect(fixture.takeWarnings()).toEqual([]);
  });

  test("adding or removing an entry of a directory that a plugin watches reloads CSS", async () => {
    const lister: StillwindPlugin = {
      name: "lister",
      async css(context) {
        const directory = path.join(context.root, "icons");
        context.watch(directory, "directory");
        return `.listed { width: ${(await context.list(directory)).length}px; }`;
      },
    };
    const fixture = await createWatchedProject(
      {
        "root.css": '@plugin "lister"; @stillwind-slot utilities;',
        "icons/a.svg": "<svg/>",
      },
      {
        plugins: [lister],
      },
    );
    await fixture.waitUntilWatched("a.svg");
    const state = fixture.project.environment("client").designSystem;
    expect(await fixture.getCss()).toMatch(/width:\s*1px/);

    const afterLoad = state.cssChange;
    writeFileSync(path.join(fixture.root, "icons/a.svg"), "<svg></svg>");
    writeFileSync(path.join(fixture.root, "icons/b.svg"), "<svg/>");
    await expect.poll(() => state.cssChange).toBeGreaterThan(afterLoad);
    expect(await fixture.getCss()).toMatch(/width:\s*2px/);

    const afterAddition = state.cssChange;
    rmSync(path.join(fixture.root, "icons/a.svg"));
    await expect.poll(() => state.cssChange).toBeGreaterThan(afterAddition);
    expect(await fixture.getCss()).toMatch(/width:\s*1px/);
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

  test("deleting an imported file removes its rules with a warning until the file is recreated", async () => {
    const fixture = await createWatchedProject({
      "root.css": rootWithImport,
      "part.css": definePaint("green"),
    });
    await fixture.waitUntilWatched("part.css");
    expect(await fixture.getCss()).toMatch(/color:\s*green/);
    const state = fixture.project.environment("client").designSystem;

    await fixture.changeCss("part.css", () => rmSync(path.join(fixture.root, "part.css")));
    expect(await fixture.getCss()).not.toMatch(/color:\s*green/);
    expect(state.cssError).toBeUndefined();
    expect(fixture.takeWarnings()).toEqual([
      expect.stringContaining("`./part.css` cannot be resolved or read"),
    ]);

    await fixture.writeCss("part.css", definePaint("yellow"));
    expect(await fixture.getCss()).toMatch(/color:\s*yellow/);
    expect(fixture.takeWarnings()).toEqual([]);
  });

  test("creating a missing import sends updated CSS through HMR without a CSS request", async () => {
    const fixture = await createWatchedProject({ "root.css": definePaint("red") }, { hmr: true });
    await fixture.waitUntilWatched("root.css");
    const state = fixture.project.environment("client").designSystem;
    await fixture.writeCss("root.css", "@import './added.css';");
    await expect
      .poll(() => fixture.takeWarnings())
      .toEqual([expect.stringContaining("`./added.css` cannot be resolved or read")]);
    expect(state.cssError).toBeUndefined();
    expect(state.cssFiles.has(path.join(fixture.root, "added.css"))).toBe(false);
    const hmrSend = vi.spyOn(fixture.server.environments.client.hot, "send");
    onTestFinished(() => hmrSend.mockRestore());

    await fixture.writeCss("added.css", definePaint("green"));

    await vi.waitFor(() => {
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

test("an accepted CSS reload invalidates SSR producers when generated names change", async () => {
  const fixture = await createWatchedProject({
    "root.css": definePaint("red") + " @stillwind-slot utilities;",
  });
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
  fixture.project.config.classNamespace = "updated";

  await fixture.writeCss("root.css", definePaint("green") + " @stillwind-slot utilities;");
  const css = await fixture.getCss("ssr");

  expect(css).toMatch(/color:\s*green/);
  expect(fixture.project.environment("ssr").designSystem.cssError).toBeUndefined();
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

/**
 * Replaces the readFile mock so the first read of one file pauses after capturing its contents.
 * @param file - Exact absolute filename to pause. Other reads continue through the real filesystem.
 * @returns A started signal and resume operation. Register resume for test cleanup to release pending work.
 */
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

/**
 * Starts a temporary Vite project and waits for its file watcher's initial scan.
 * @param files - Root-relative filenames and contents written into the temporary directory.
 * @param options - Optional environment-specific CSS resolution and HMR settings.
 * @returns Project, server, and helpers for compilation and observed CSS changes. Test cleanup closes and removes them.
 * @throws If file creation, server setup, or startup fails.
 */
async function createWatchedProject(
  files: Record<string, string>,
  options: {
    /** Whether environment-css resolves to separate client.css and ssr.css files. */
    environmentCss?: boolean;
    /** Whether Vite HMR is enabled for this fixture. Defaults to false. */
    hmr?: boolean;
    /** Plugins configured for the project. */
    plugins?: StillwindPlugin[];
  } = {},
) {
  const root = mkdtempSync(path.join(repositoryRoot, ".test-css-watch-"));
  for (const [filename, content] of Object.entries(files)) {
    mkdirSync(path.dirname(path.join(root, filename)), { recursive: true });
    writeFileSync(path.join(root, filename), content);
  }
  const project = createViteProject({
    css: "root.css",
    plugins: options.plugins,
  });
  const plugins = createVitePlugins(project);
  const watcherReady = Promise.withResolvers<void>();
  const server = await createServer({
    configFile: false,
    root,
    plugins: [
      {
        name: "environment-css",
        configureServer: {
          order: "pre",
          handler(server) {
            server.watcher.once("ready", watcherReady.resolve);
          },
        },
        /**
         * Resolves a fixture import to the current environment's stylesheet.
         * @param id - Requested specifier.
         * @returns An absolute client or SSR CSS path when enabled, or undefined for other requests.
         */
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
  // Development hooks report warnings through Vite's logger, and fixture requests report them
  // through `context`. Tests take expected warnings, and any other warning fails the test.
  const warnings: string[] = [];
  vi.spyOn(server.config.logger, "warn").mockImplementation((message) => {
    warnings.push(message);
  });
  onTestFinished(async () => {
    await server.close();
    rmSync(root, { recursive: true, force: true });
    expect(warnings).toEqual([]);
  });
  await server.listen();
  await watcherReady.promise;
  /**
   * Creates hook operations for a named server environment.
   * @param environmentName - Existing Vite environment, normally client or ssr.
   * @returns Resolution, watching, and warning operations. Warnings are recorded for `takeWarnings`.
   */
  const context = (environmentName: string) => {
    const environment = server.environments[environmentName];
    return {
      environment,
      resolve: (specifier: string, importer: string) =>
        environment.pluginContainer.resolveId(specifier, importer),
      addWatchFile: (file: string) => server.watcher.add(file),
      warn: (message: string) => {
        warnings.push(message);
      },
    };
  };
  return {
    root,
    project,
    server,
    /**
     * Removes and returns the warnings recorded since the previous call.
     * @returns Warning messages in the order they were reported.
     */
    takeWarnings: () => warnings.splice(0),
    /**
     * Plans current document CSS through a selected development environment.
     * @param environment - Existing environment name, defaulting to client.
     * @returns CSS after the environment's pending work and resource resolution.
     */
    getCss: async (environment = "client") =>
      (await project.documentPlan(environment, context(environment))).css,
    /**
     * Runs the producer fixture through the compiler's transform handler.
     * @param environment - Existing environment name, defaulting to client.
     * @param suffix - Extra source appended to distinguish later producer updates.
     * @returns The transform result after compilation and CSS publication.
     * @throws If the transform hook is unavailable or compilation fails.
     */
    async compileProducer(environment = "client", suffix = "") {
      const plugin = server.environments[environment].plugins.find(
        (plugin) => plugin.name === "stillwind:compiler",
      )!;
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
    /**
     * Waits until Vite's watcher includes the requested basename.
     * @param filename - Basename recorded in the watcher's directory inventory.
     * @returns Completion after the watch appears, or a rejected polling assertion after timeout.
     */
    async waitUntilWatched(filename: string) {
      await expect
        .poll(() => Object.values(server.watcher.getWatched()).flat())
        .toContain(filename);
    },
    /**
     * Performs a filesystem mutation and waits until an environment observes a CSS revision change.
     * @param filename - Root-relative CSS path expected in an environment's observed file set.
     * @param change - Synchronous filesystem mutation performed after capturing prior revision counters.
     * @returns Completion once at least one matching environment advances its CSS counter.
     * @throws If the mutation fails or watcher polling times out.
     */
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
    /**
     * Replaces a fixture stylesheet and waits for its watcher event.
     * @param filename - Root-relative CSS filename.
     * @param content - Complete replacement text written as UTF-8.
     * @returns Completion after an environment observes the change.
     * @throws If writing fails or watcher polling times out.
     */
    async writeCss(filename: string, content: string) {
      await this.changeCss(filename, () => {
        writeFileSync(path.join(root, filename), content);
      });
    },
  };
}

/**
 * Starts client and SSR producers with different environment-resolved paint utilities.
 * @returns A watched fixture with both CSS files observed and read counters cleared after initial compilation.
 * @throws If fixture setup, watching, or initial compilation fails.
 */
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

/**
 * Extracts CSS reads recorded since the filesystem mock was last cleared.
 * @returns String filenames ending in .css, retaining call order and duplicates.
 */
function cssFilesRead() {
  return vi
    .mocked(readFile)
    .mock.calls.map(([file]) => file)
    .filter((file): file is string => typeof file === "string" && file.endsWith(".css"));
}
