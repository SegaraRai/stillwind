import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { createServer } from "node:http";
import path from "node:path";

import type { ScenarioContext } from "@stillwind-internal/e2e-harness/scenario";
import { chromium, type Browser, type Page } from "playwright";
import { expect } from "vite-plus/test";

export type RsbuildApplication = ReturnType<typeof createApplication>;

/** Creates lifecycle and fixture operations; initialize must run before browser contracts. */
export function createApplication(context: ScenarioContext) {
  const root = context.root;

  let dir!: string;
  const write = async (name: string, text: string) => {
    const file = path.join(dir, name);
    await fs.mkdir(path.dirname(file), { recursive: true });
    await fs.writeFile(file, text);
  };
  const put = write;

  let initialTheme!: string;
  const theme = (color: string) => initialTheme.replace(/rgb\(11,\s*22,\s*33\)/, color);

  const mode = "rsbuild";
  const results = {
    checks: [] as {
      name: string;
      passed: boolean;
      [key: string]: unknown;
    }[],
  };

  async function build() {
    const child = spawn(
      process.execPath,
      [path.join(root, "scripts/rsbuild-adapter-build.ts"), dir],
      {
        windowsHide: true,
        env: { ...process.env, NODE_ENV: "production" },
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    let logs = "";
    child.stdout.on("data", (data) => {
      logs += data;
    });
    child.stderr.on("data", (data) => {
      logs += data;
    });
    expect(await new Promise((done) => child.on("exit", done)), logs).toBe(0);
    const stats: {
      chunks: {
        initial: boolean;
        files: string[];
      }[];
      modules: {
        name: string;
        built: boolean;
      }[];
    } = JSON.parse(await fs.readFile(path.join(dir, "stats.json"), "utf8"));
    const initial = stats.chunks.filter((chunk) => chunk.initial).flatMap((chunk) => chunk.files);
    await fs.writeFile(
      path.join(dir, "dist/index.html"),
      initial
        .filter((file) => file.endsWith(".css"))
        .map((file) => `<link rel="stylesheet" href="/${file}">`)
        .join("") +
        initial
          .filter((file) => file.endsWith(".js"))
          .map((file) => `<script defer src="/${file}"></script>`)
          .join(""),
    );
    return stats;
  }

  const server = createServer(async (req, res) => {
    try {
      const name = new URL(req.url ?? "/", "http://localhost").pathname;
      const file = path.join(dir, "dist", name === "/" ? "index.html" : name);
      const body = await fs.readFile(file);
      res.setHeader("cache-control", "no-store");
      res.setHeader(
        "content-type",
        file.endsWith(".css")
          ? "text/css"
          : file.endsWith(".js")
            ? "text/javascript"
            : file.endsWith(".svg")
              ? "image/svg+xml"
              : "text/html",
      );
      res.end(body);
    } catch {
      res.statusCode = 404;
      res.end();
    }
  });

  let browser!: Browser;
  let page!: Page;
  let url!: string;
  async function initialize() {
    dir = context.directory;
    initialTheme = await fs.readFile(path.join(dir, "root.css"), "utf8");
    await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
    url = `http://127.0.0.1:${(server.address() as import("node:net").AddressInfo).port}`;
    browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });
    page = await browser.newPage();
  }

  async function dispose() {
    await browser?.close();
    await new Promise((done) => server.close(done));
    await fs.writeFile(
      path.join(context.resultsDirectory, `${mode}-real.json`),
      JSON.stringify(results, null, 2),
    );
  }

  return {
    get dir() {
      return dir;
    },
    put,
    theme,
    mode,
    results,
    build,
    get page() {
      return page;
    },
    get url() {
      return url;
    },
    initialize,
    dispose,
  };
}
