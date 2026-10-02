import fs from "node:fs/promises";
import path from "node:path";

import type { ScenarioContext } from "@stillwind/e2e-harness/scenario";
import { stillwind } from "@stillwind/integration/vite";
import { chromium, type Browser, type Page } from "playwright";
import { createServer, preview, type InlineConfig } from "vite-plus";
import { expect } from "vite-plus/test";

/**
 * Creates lifecycle and fixture operations; initialize must run before browser contracts.
 */
export function createApplication(context: ScenarioContext) {
  let dir!: string;

  const write = async (name: string, text: string) => {
    const file = path.join(dir, name);
    await fs.mkdir(path.dirname(file), { recursive: true });
    await fs.writeFile(file, text);
  };
  const put = write;

  let flow!: string;
  const flowSource = (nested: number) => flow.replace('"m-4"', '"m-' + nested + '"');

  const writeSsrHtml = (html: string) =>
    put(
      "ssr.html",
      `<div id="root">${html}</div><script type="module" src="/hydrate.ts"></script>`,
    );

  const ssrClass = (html: string) => {
    const match = html.match(/id="sample"[^>]*class="([^"]+)"/);
    expect(match, `server html carries the sample class: ${html}`).toBeTruthy();
    return match![1];
  };

  const config = (): InlineConfig => ({
    configFile: false,
    root: dir,
    plugins: [
      stillwind({
        css: path.join(dir, "root.css"),
        types: false,
        emission: context.args[0] === "split" ? "split" : "single",
      }),
    ],
    logLevel: "silent",
  });

  const results = {
    checks: [] as {
      name: string;
      passed: boolean;
      [key: string]: unknown;
    }[],
    directory: dir,
  };
  const record = (name: string) => {
    results.checks.push({ name, passed: true });
    console.log(name);
  };

  const states: [
    {
      enabled: boolean;
      nested: boolean;
    },
    string,
  ][] = [
    [{ enabled: false, nested: false }, "40px"],
    [{ enabled: true, nested: false }, "24px"],
    [{ enabled: true, nested: true }, "16px"],
  ];
  async function applyState(
    page: Page,
    state: {
      enabled: boolean;
      nested: boolean;
    },
  ) {
    await page.evaluate((next) => window.setState(next), state);
  }

  let browser!: Browser;
  let server:
    | Awaited<ReturnType<typeof createServer>>
    | Awaited<ReturnType<typeof preview>>
    | undefined;
  let url!: string;
  async function initialize() {
    dir = context.directory;
    flow = await fs.readFile(path.join(dir, "flow.ts"), "utf8");
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
    server = await createServer({
      ...config(),
      server: {
        port: 0,
      },
    });
    await server.listen();
    url = server.resolvedUrls!.local[0];
  }

  async function dispose() {
    await server?.close();
    await browser?.close();
    await fs.writeFile(
      path.join(context.resultsDirectory, "conditional.json"),
      JSON.stringify(results, null, 2),
    );
  }

  return {
    get dir() {
      return dir;
    },
    put,
    flowSource,
    writeSsrHtml,
    ssrClass,
    config,
    record,
    states,
    applyState,
    get browser() {
      return browser;
    },
    get server() {
      return server;
    },
    set server(value: typeof server) {
      server = value;
    },
    get url() {
      return url;
    },
    initialize,
    dispose,
  };
}

export type ConditionalApplication = ReturnType<typeof createApplication>;
