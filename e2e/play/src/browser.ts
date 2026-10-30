import { readFile } from "node:fs/promises";

import type { Page } from "playwright";

interface BuildProbe {
  builds: number;
  workers: number;
  failNext: boolean;
  snapshots: {
    generation: number;
    characters: number;
    deferred: number;
  }[];
  reads: number[];
}

type ProbedGlobal = typeof globalThis & {
  __stillwindBuildProbe: BuildProbe;
};

/**
 * Records real application builds and can terminate the next worker that receives a build.
 */
export async function installBuildProbe(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const probe: BuildProbe = { builds: 0, workers: 0, failNext: false, snapshots: [], reads: [] };
    (globalThis as ProbedGlobal).__stillwindBuildProbe = probe;
    const workers = new WeakSet<Worker>();
    const postMessage = Reflect.get(Worker.prototype, "postMessage") as Worker["postMessage"];
    Worker.prototype.postMessage = new Proxy(postMessage, {
      apply(target, worker: Worker, args) {
        if (args[0]?.kind === "build") {
          const files = args[0].files as [string, string | null][];
          probe.snapshots.push({
            generation: args[0].generation,
            characters: files.reduce((sum, [, text]) => sum + (text?.length ?? 0), 0),
            deferred: files.filter(([, text]) => text === null).length,
          });
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
        if (args[0]?.kind === "read-reply" && typeof args[0].value === "string") {
          probe.reads.push(args[0].value.length);
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
  await page.waitForFunction(
    (expected) => {
      const actual = document.querySelector('[role="status"]')?.textContent?.trim();
      return (
        actual === expected ||
        (expected === "Ready" && (actual === "Build error" || actual === "Runtime error"))
      );
    },
    status,
    { timeout },
  );
  if ((await page.getByRole("status").innerText()).trim() !== status) {
    throw new Error(await page.locator(".runtime-error").innerText());
  }
}

/**
 * Replaces an editable source through Monaco's paste handling.
 */
export async function editSource(page: Page, file: string, text: string): Promise<void> {
  await page.getByRole("button", { name: file, exact: true }).click();
  await page
    .getByRole("region", { name: "Editor" })
    .locator(".monaco-editor textarea.inputarea")
    .focus();
  await page.keyboard.press("Control+a");
  await page
    .getByRole("region", { name: "Editor" })
    .locator(".monaco-editor textarea.inputarea")
    .evaluate((input, source) => {
      const clipboardData = new DataTransfer();
      clipboardData.setData("text/plain", source);
      input.dispatchEvent(
        new ClipboardEvent("paste", {
          clipboardData,
          bubbles: true,
          cancelable: true,
        }),
      );
    }, text);
}

export function presetSource(preset: string, file: string) {
  return readFile(
    new URL(`../../../apps/play/examples/${preset}/${file}`, import.meta.url),
    "utf8",
  );
}

export function editedApp(source: string, revision: number) {
  const gap = [2, 3, 4][(revision - 1) % 3];
  return source
    .replace("<main", `<main data-build="${revision}"`)
    .replace("text-foreground", `gap-${gap} text-foreground`);
}
