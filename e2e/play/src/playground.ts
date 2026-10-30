import { randomUUID } from "node:crypto";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { relative, resolve, sep, extname } from "node:path";

import { chromium, type Page, type FrameLocator, type Locator } from "playwright";
import { expect, type TestContext } from "vite-plus/test";

import { editSource } from "./browser.ts";

// The site embeds these files as the Release checklist preset. Vitest stubs CSS imports, so the test
// reads them from disk instead of importing the playground module.
const exampleDirectory = resolve(
  import.meta.dirname,
  "../../../apps/play/examples/release-checklist",
);
export const exampleFiles = new Map(
  await Promise.all(
    (await readdir(exampleDirectory, { recursive: true, withFileTypes: true }))
      .filter((entry) => entry.isFile())
      .map(async (entry) => {
        const path = resolve(entry.parentPath, entry.name);
        const name = relative(exampleDirectory, path).split(sep).join("/");
        return [`/workspace/${name}`, await readFile(path, "utf8")] as const;
      }),
  ),
);

interface Playground {
  page: Page;
  frame: FrameLocator;
  task: Locator;
  buffers: Map<string, string>;
  openPage: () => Promise<Page>;
}

/** Runs a scenario with its own browser, editable files, static server, and failure evidence. */
export async function withPlayground(
  { onTestFailed }: TestContext,
  run: (playground: Playground) => Promise<void>,
): Promise<void> {
  const directory = process.env.STILLWIND_PLAY_TEST_DIRECTORY
    ? resolve(process.env.STILLWIND_PLAY_TEST_DIRECTORY)
    : resolve(import.meta.dirname, "../../../apps/play/dist/acceptance");
  const requests: string[] = [];
  const server = createServer(async (request, response) => {
    requests.push(request.method ?? "");
    const path = resolve(
      directory,
      "." +
        decodeURIComponent(new URL(request.url!, "http://localhost").pathname).replace(
          /\/$/,
          "/index.html",
        ),
    );
    if (!path.startsWith(directory + sep)) {
      response.writeHead(403).end();
      return;
    }
    try {
      const contents = await readFile(path);
      response
        .writeHead(200, {
          "Content-Type":
            (
              {
                ".html": "text/html",
                ".js": "text/javascript",
                ".mjs": "text/javascript",
                ".css": "text/css",
                ".json": "application/json",
                ".wasm": "application/wasm",
              } as Record<string, string>
            )[extname(path)] ?? "application/octet-stream",
          "Cross-Origin-Opener-Policy": "same-origin",
          "Cross-Origin-Embedder-Policy": "require-corp",
        })
        .end(contents);
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") {
    expect.fail("Missing static server port");
  }
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  let pageForEvidence: Page | undefined;
  const errors: string[] = [];
  const logs: string[] = [];
  const failedRequests: string[] = [];
  let failureEvidence: Promise<void> | undefined;
  async function saveFailure(): Promise<void> {
    const evidence = resolve(import.meta.dirname, "../results", randomUUID());
    await mkdir(evidence, { recursive: true });
    const details = { errors, logs, failedRequests };
    console.error("Playground failure:", JSON.stringify(details, null, 2));
    await Promise.allSettled([
      (pageForEvidence?.locator("body").innerText({ timeout: 5_000 }) ?? Promise.resolve(""))
        .catch(() => "")
        .then((body) =>
          writeFile(
            resolve(evidence, "browser.json"),
            JSON.stringify({ ...details, body }, null, 2),
          ),
        ),
      pageForEvidence?.screenshot({
        path: resolve(evidence, "failure.png"),
        fullPage: true,
        timeout: 5_000,
      }),
      pageForEvidence?.context().tracing.stop({ path: resolve(evidence, "trace.zip") }),
    ]);
  }
  onTestFailed(() => (failureEvidence ??= saveFailure()));
  const remote: string[] = [];
  async function openPage(): Promise<Page> {
    const page = await browser.newPage({ permissions: ["clipboard-read", "clipboard-write"] });
    pageForEvidence = page;
    page.setDefaultTimeout(30_000);
    await page.context().tracing.start({ screenshots: true, snapshots: true, sources: true });
    page.on("console", (message) => logs.push(`${message.type()}: ${message.text()}`));
    page.on("requestfailed", (request) =>
      failedRequests.push(`${request.url()}: ${request.failure()?.errorText}`),
    );
    page.on("response", (response) => {
      if (response.status() >= 400) {
        failedRequests.push(`${response.url()}: HTTP ${response.status()}`);
      }
    });
    page.on("request", (request) => {
      if (/^https?:/.test(request.url()) && new URL(request.url()).hostname !== "127.0.0.1") {
        remote.push(request.url());
      }
    });
    page.on("pageerror", (error) => {
      // VS Code's background tokenizer races with edits that replace a whole model, which this test
      // makes repeatedly. The race throws in the TextMate worker (tokenizeEncoded) or while the
      // editor applies its results (setTokensAndStates), and it leaves the editor usable.
      if (!/\b(?:setTokensAndStates|tokenizeEncoded)\b/.test(error.stack ?? "")) {
        errors.push(error.message);
      }
    });
    return page;
  }
  try {
    const page = await openPage();
    const buffers = new Map(exampleFiles);
    console.info("Playground: load the preset");
    await page.goto(`http://127.0.0.1:${address.port}/#preset=release-checklist`);
    const frame = page.frameLocator('iframe[title="Stillwind Playground Preview"]');
    await frame.getByRole("heading", { name: "Release checklist" }).waitFor({ timeout: 60_000 });
    await expect
      .poll(() =>
        frame.locator("main").evaluate((element) => getComputedStyle(element).backgroundColor),
      )
      .not.toBe("rgba(0, 0, 0, 0)");
    const task = frame.getByRole("checkbox", { name: "Update the migration guide" });
    await task.click();
    expect(await task.getAttribute("aria-checked")).toBe("true");
    await run({ page, frame, task, buffers, openPage });
    expect(errors).toEqual([]);
    expect(remote).toEqual([]);
    expect(new Set(requests)).toEqual(new Set(["GET"]));
  } catch (error) {
    await (failureEvidence ??= saveFailure());
    throw error;
  } finally {
    console.info("Playground: close the browser and static server");
    await browser.close();
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
}

/** Replaces text in the tracked buffer and synchronizes it through the editor. */
export async function replace(
  page: Page,
  buffers: Map<string, string>,
  file: string,
  from: string | RegExp,
  to: string,
): Promise<void> {
  const key = `/workspace/${file}`;
  const text = buffers.get(key)!.replace(from, to);
  buffers.set(key, text);
  await editSource(page, file, text);
}
