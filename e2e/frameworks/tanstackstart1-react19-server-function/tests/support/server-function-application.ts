import fs from "node:fs/promises";
import path from "node:path";

import { expectCss, waitForHydration } from "@stillwind/e2e-harness/browser";
import { availablePort } from "@stillwind/e2e-harness/port";
import { startNode, waitForServer } from "@stillwind/e2e-harness/process";
import type { ScenarioContext } from "@stillwind/e2e-harness/scenario";
import { snapshotSources } from "@stillwind/e2e-harness/source";
import { chromium, type Browser, type Page } from "playwright";
import { expect } from "vite-plus/test";

/** Creates lifecycle and fixture operations; initialize must run before browser contracts. */
export function createApplication(context: ScenarioContext) {
  const root = context.root;

  let dir!: string;
  let sources!: Awaited<ReturnType<typeof snapshotSources>>;
  const write = async (name: string, text: string) => {
    const file = path.join(dir, name);
    await fs.mkdir(path.dirname(file), { recursive: true });
    await fs.writeFile(file, text);
  };

  let initialTheme!: string;
  const theme = (color: string) => initialTheme.replace(/rgb\(11,\s*22,\s*33\)/, color);

  const bundler = context.args[0] || "vite";
  let port!: number;
  let url!: string;
  const results = {
    framework: "TanStack Start",
    bundler,
    checks: [] as {
      name: string;
      passed: boolean;
      [key: string]: unknown;
    }[],
  };

  let home!: string;
  let proc: ReturnType<typeof start> | undefined;
  let browser!: Browser;
  function start(mode: string) {
    return startNode(
      [path.join(root, "scripts/tanstack-runner.ts"), bundler, mode, dir],
      dir,
      {
        ...process.env,
        NODE_ENV: mode === "dev" ? "development" : "production",
        PORT: String(port),
        STILLWIND_E2E_PORT: String(port),
        NITRO_PORT: String(port),
      },
      600_000,
    );
  }

  async function stop() {
    const handle = proc;
    proc = undefined;
    if (handle) {
      await handle.stop();
      context.logs.push(handle.logs());
    }
  }

  async function ready() {
    if (!proc) {
      throw new Error("Application process has not started");
    }
    await waitForServer(url, proc);
  }

  let page!: Page;
  const errors: string[] = [];
  async function initialize() {
    port = await availablePort();
    url = "http://localhost:" + port;
    dir = context.directory;
    sources = await snapshotSources(dir, [
      "root.css",
      "src/routes/index.tsx",
      "src/server-style.ts",
    ]);
    initialTheme = await fs.readFile(path.join(dir, "root.css"), "utf8");
    home = await fs.readFile(path.join(dir, "src/routes/index.tsx"), "utf8");
    proc = start("dev");
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
      path.join(context.resultsDirectory, `tanstack-${bundler}-real.json`),
      JSON.stringify(results, null, 2) + "\n",
    );
  }

  let built = false;
  async function compile() {
    proc = start("build");
    try {
      expect(await proc.done, proc.logs()).toBe(0);
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
    proc = start("start");
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
    await expectCss(page, "#static", { color: "rgb(11, 22, 33)" });
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
    bundler,
    get url() {
      return url;
    },
    results,
    get home() {
      return home;
    },
    get browser() {
      return browser;
    },
    stop,
    get page() {
      return page;
    },
    initialize,
    assertNoBrowserErrors,
    dispose,
  };
}

export type ServerFunctionApplication = ReturnType<typeof createApplication>;
