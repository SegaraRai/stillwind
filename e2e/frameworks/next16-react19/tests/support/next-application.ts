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
  const require = createRequire(import.meta.url);

  let dir!: string;
  let sources!: Awaited<ReturnType<typeof snapshotSources>>;
  const write = async (name: string, text: string) => {
    const file = path.join(dir, name);
    await fs.mkdir(path.dirname(file), { recursive: true });
    await fs.writeFile(file, text);
  };

  let initialTheme!: string;
  const theme = (color: string) => initialTheme.replace(/rgb\(11,\s*22,\s*33\)/, color);

  const mode = context.args[0] || "turbopack";
  const standalone = context.args[1] === "standalone";
  const label = standalone ? "turbopack-standalone" : mode;
  let port!: number;
  let url!: string;
  const results = {
    mode,
    standalone,
    next: require("next/package.json").version,
    checks: [] as {
      name: string;
      passed: boolean;
      [key: string]: unknown;
    }[],
  };
  const browserErrors: string[] = [];

  const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

  let browser!: Browser;
  let proc: ReturnType<typeof start> | undefined;
  function start(args: string[]) {
    return startNode(
      [require.resolve("next/dist/bin/next"), ...args],
      dir,
      {
        ...process.env,
        NODE_ENV: args[0] === "dev" ? "development" : "production",
        NEXT_TELEMETRY_DISABLED: "1",
        STILLWIND_E2E_STANDALONE: standalone ? "1" : "0",
      },
      600_000,
    );
  }

  async function ready() {
    if (!proc) {
      throw new Error("Application process has not started");
    }
    await waitForServer(url, proc);
  }

  async function stop() {
    const handle = proc;
    proc = undefined;
    if (handle) {
      // A development page can retry HMR requests while its server is stopped.
      if (page && !page.isClosed()) {
        await page.goto("about:blank");
      }
      await handle.stop();
      context.logs.push(handle.logs());
    }
  }

  let page!: Page;
  async function initialize() {
    port = await availablePort();
    url = "http://localhost:" + port;
    dir = context.directory;
    sources = await snapshotSources(dir, ["root.css", "app/page.jsx", "shared/derived.ts"]);
    initialTheme = await fs.readFile(path.join(dir, "root.css"), "utf8");
    proc = start(["dev", `--${mode}`, "-p", String(port)]);
    await ready();
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
    page = await browser.newPage();
    page.on("pageerror", (error) => browserErrors.push(String(error)));
    page.on("requestfailed", (request) => {
      const failure = request.failure()?.errorText;
      if (failure !== "net::ERR_ABORTED") {
        browserErrors.push(`${request.url()}: ${failure}`);
      }
    });
  }

  async function assertNoBrowserErrors() {
    expect(browserErrors).toEqual([]);
  }

  async function dispose() {
    await browser?.close();
    await stop();
    await fs.writeFile(
      path.join(context.resultsDirectory, `adapters-${label}.json`),
      JSON.stringify(results, null, 2),
    );
  }

  let built = false;
  async function compile() {
    proc = start(["build", `--${mode}`]);
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
    browserErrors.length = 0;
    if (temperature === "warm" && !built) {
      await compile();
    }
    await compile();
    built = true;
  }

  async function serve() {
    proc = start(["start", "-p", String(port)]);
    await ready();
  }

  /**
   * Restores fixture inputs and opens the initial page so development cases can run alone.
   */
  async function prepareDevelopmentCase(): Promise<void> {
    browserErrors.length = 0;
    await page.goto(url);
    await sources.reset();
    await waitForHydration(page);
    await expectCss(page, "#server", { color: "rgb(11, 22, 33)" });
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
    mode,
    standalone,
    get url() {
      return url;
    },
    results,
    delay,
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

export type NextApplication = ReturnType<typeof createApplication>;
