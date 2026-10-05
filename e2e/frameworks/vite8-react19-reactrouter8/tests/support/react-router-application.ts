import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";

import { expectCss, waitForHydration } from "@stillwind-internal/e2e-harness/browser";
import { availablePort } from "@stillwind-internal/e2e-harness/port";
import { startNode, waitForServer } from "@stillwind-internal/e2e-harness/process";
import type { ScenarioContext } from "@stillwind-internal/e2e-harness/scenario";
import { snapshotSources } from "@stillwind-internal/e2e-harness/source";
import { chromium, type Browser, type Page } from "playwright";
import { expect } from "vite-plus/test";

/** Creates lifecycle and fixture operations; initialize must run before browser contracts. */
export function createApplication(context: ScenarioContext) {
  let dir!: string;
  let sources!: Awaited<ReturnType<typeof snapshotSources>>;
  const write = async (name: string, text: string) => {
    const file = path.join(dir, name);
    await fs.mkdir(path.dirname(file), { recursive: true });
    await fs.writeFile(file, text);
  };
  let initialTheme!: string;

  const theme = (color: string) => initialTheme.replace(/rgb\(11,\s*22,\s*33\)/, color);
  const require = createRequire(import.meta.url);
  let port!: number;
  let url!: string;
  const results = {
    framework: "react-router",
    version: require("@react-router/dev/package.json").version,
    checks: [] as {
      name: string;
      passed: boolean;
      [key: string]: unknown;
    }[],
  };

  let home!: string;
  let serverSource!: string;
  let processHandle: ReturnType<typeof start> | undefined;
  let browser!: Browser;
  function start(kind: string) {
    const executable = path.join(
      path.dirname(
        require.resolve(`@react-router/${kind === "start" ? "serve" : "dev"}/package.json`),
      ),
      "bin.cjs",
    );
    const args = kind === "start" ? ["build/server/index.js"] : [kind];
    return startNode(
      [executable, ...args],
      dir,
      {
        ...process.env,
        NODE_OPTIONS: kind === "dev" ? "--conditions=development" : "",
        NODE_ENV: kind === "dev" ? "development" : "production",
        PORT: String(port),
        STILLWIND_E2E_PORT: String(port),
      },
      600_000,
    );
  }

  async function stop() {
    const handle = processHandle;
    processHandle = undefined;
    if (handle) {
      await handle.stop();
      context.logs.push(handle.logs());
    }
  }

  async function ready() {
    if (!processHandle) {
      throw new Error("Application process has not started");
    }
    await waitForServer(url, processHandle);
  }

  let page!: Page;
  const errors: string[] = [];
  async function initialize() {
    port = await availablePort();
    url = "http://localhost:" + port;
    dir = context.directory;
    sources = await snapshotSources(dir, [
      "root.css",
      "app/routes/home.jsx",
      "app/server.server.js",
    ]);
    initialTheme = await fs.readFile(path.join(dir, "root.css"), "utf8");
    home = await fs.readFile(path.join(dir, "app/routes/home.jsx"), "utf8");
    serverSource = await fs.readFile(path.join(dir, "app/server.server.js"), "utf8");
    processHandle = start("dev");
    await ready();
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
    page = await browser.newPage();
    page.setDefaultTimeout(20000);
    errors.length = 0;
    page.on("pageerror", (error) => errors.push(error.message));
  }

  async function assertNoBrowserErrors() {
    expect(errors).toEqual([]);
  }

  async function dispose() {
    await browser?.close();
    await stop();
    await fs.writeFile(
      path.join(context.resultsDirectory, `react-router-real.json`),
      JSON.stringify(results, null, 2) + "\n",
    );
  }

  let built = false;
  async function compile() {
    processHandle = start("build");
    try {
      expect(await processHandle.done, processHandle.logs()).toBe(0);
    } finally {
      await stop();
    }
  }

  async function build(temperature: "cold" | "warm") {
    await stop();
    await sources.reset(["root.css"]);
    const productionTheme = theme("rgb(44,55,66)");
    if ((await fs.readFile(path.join(dir, "root.css"), "utf8")) !== productionTheme) {
      await write("root.css", productionTheme);
    }
    errors.length = 0;
    if (temperature === "warm" && !built) {
      await compile();
    }
    await compile();
    built = true;
  }

  async function serve() {
    processHandle = start("start");
    await ready();
  }

  /**
   * Restores fixture inputs and opens the initial page so development cases can run alone.
   */
  async function prepareDevelopmentCase(): Promise<void> {
    await sources.reset();
    errors.length = 0;
    await page.goto(url);
    await waitForHydration(page);
    await expectCss(page, "#client", { color: "rgb(11, 22, 33)" });
  }
  return {
    prepareDevelopmentCase,
    build,
    serve,
    get dir() {
      return dir;
    },
    write,
    theme,
    get url() {
      return url;
    },
    results,
    get home() {
      return home;
    },
    get serverSource() {
      return serverSource;
    },
    get browser() {
      return browser;
    },
    stop,
    get page() {
      return page;
    },
    context,
    initialize,
    assertNoBrowserErrors,
    dispose,
  };
}

export type ReactRouterApplication = ReturnType<typeof createApplication>;
