import { readFile } from "node:fs/promises";

import type { Page } from "playwright";

interface BuildProbe {
  builds: number;
  workers: number;
  failNext: boolean;
}

type ProbedGlobal = typeof globalThis & {
  __stillwindBuildProbe: BuildProbe;
};

/**
 * Records real application builds and can terminate the next worker that receives a build.
 */
export async function installBuildProbe(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const probe = { builds: 0, workers: 0, failNext: false };
    (globalThis as ProbedGlobal).__stillwindBuildProbe = probe;
    const workers = new WeakSet<Worker>();
    const postMessage = Reflect.get(Worker.prototype, "postMessage") as Worker["postMessage"];
    Worker.prototype.postMessage = new Proxy(postMessage, {
      apply(target, worker: Worker, args) {
        if (args[0]?.kind === "build") {
          probe.builds++;
          if (!workers.has(worker)) {
            workers.add(worker);
            probe.workers++;
          }
          if (probe.failNext) {
            probe.failNext = false;
            worker.terminate();
            queueMicrotask(() =>
              worker.dispatchEvent(
                new ErrorEvent("error", {
                  message: "Injected build worker failure",
                }),
              ),
            );
            return;
          }
        }
        return Reflect.apply(target, worker, args);
      },
    });
  });
}

export function buildProbe(page: Page) {
  return page.evaluate(() => (globalThis as ProbedGlobal).__stillwindBuildProbe);
}

export async function failNextBuild(page: Page): Promise<void> {
  await page.evaluate(() => {
    (globalThis as ProbedGlobal).__stillwindBuildProbe.failNext = true;
  });
}

/**
 * Waits for a new build and its final UI status, including the editor's debounce interval.
 */
export async function waitForBuild(
  page: Page,
  previous: number,
  status = "Ready",
  timeout = 90_000,
): Promise<void> {
  await page.waitForFunction(
    (count) => (globalThis as ProbedGlobal).__stillwindBuildProbe.builds > count,
    previous,
    { timeout },
  );
  await page.getByRole("status").filter({ hasText: status }).waitFor({ timeout });
}

/**
 * Replaces an editable source through Monaco's accessible input.
 */
export async function editSource(page: Page, file: string, text: string): Promise<void> {
  await page.getByRole("button", { name: file, exact: true }).click();
  await page
    .getByRole("region", { name: "Editor" })
    .locator(".monaco-editor textarea.inputarea")
    .focus();
  await page.keyboard.press("Control+a");
  await page.keyboard.insertText(text);
}

export function presetSource(preset: string, file: string) {
  return readFile(
    new URL(`../../../apps/play/examples/${preset}/${file}`, import.meta.url),
    "utf8",
  );
}

export function editedApp(source: string, revision: number) {
  return source
    .replace("<main", `<main data-build="${revision}"`)
    .replace("text-foreground", `gap-${revision + 1} text-foreground`);
}
