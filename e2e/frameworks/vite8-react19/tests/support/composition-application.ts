import fs from "node:fs/promises";
import path from "node:path";

import type { ScenarioContext } from "@stillwind-internal/e2e-harness/scenario";
import { stillwind } from "@stillwind/integration/vite";
import { chromium, type Browser } from "playwright";
import { createServer, preview, type InlineConfig } from "vite";

/** Creates lifecycle and fixture operations; initialize must run before browser contracts. */
export function createApplication(context: ScenarioContext) {
  let dir!: string;

  const write = async (name: string, text: string) => {
    const file = path.join(dir, name);
    await fs.mkdir(path.dirname(file), { recursive: true });
    await fs.writeFile(file, text);
  };
  const put = write;

  const baseSource = (value: number) =>
    'import { style } from "stillwind/react";export const base=style(\'m-' +
    value +
    " hover:m-20');";

  const routeSource = (base: string, delta: number) =>
    'import { style } from "stillwind/react";import {' +
    base +
    "} from './shared';export const props=style(" +
    base +
    ",'m-" +
    delta +
    "');";

  const writeSsrHtml = (html: string) =>
    put(
      "ssr.html",
      '<div id="root">' + html + '</div><script type="module" src="/hydrate.ts"></script>',
    );

  const config = (): InlineConfig => ({
    configFile: false,
    root: dir,
    plugins: [
      stillwind({
        css: path.join(dir, "root.css"),
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

  let browser!: Browser;
  let server:
    | Awaited<ReturnType<typeof createServer>>
    | Awaited<ReturnType<typeof preview>>
    | undefined;
  let url!: string;
  async function initialize() {
    dir = context.directory;
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
    await browser?.close();
    if (server && "close" in server) {
      await server.close();
    } else {
      await new Promise<void>((resolve) => server?.httpServer?.close(() => resolve()) ?? resolve());
    }
    await fs.writeFile(
      path.join(context.resultsDirectory, "composition.json"),
      JSON.stringify(results, null, 2),
    );
  }

  return {
    get dir() {
      return dir;
    },
    put,
    baseSource,
    routeSource,
    writeSsrHtml,
    config,
    record,
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

export type CompositionApplication = ReturnType<typeof createApplication>;
