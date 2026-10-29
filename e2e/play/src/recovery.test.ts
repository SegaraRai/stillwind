import { resolve } from "node:path";

import { chromium, type Browser } from "playwright";
import { expect, test } from "vite-plus/test";

import {
  buildProbe,
  editedApp,
  editSource,
  failNextBuild,
  installBuildProbe,
  presetSource,
  waitForBuild,
} from "./browser.ts";
import { servePlayground } from "./server.ts";

test.each(["minimal", "release-checklist"])(
  "%s rebuilds source, CSS and settings and recovers after worker and initialization failures",
  async (preset) => {
    const server = await servePlayground(
      process.env.STILLWIND_PLAY_TEST_DIRECTORY ??
        resolve(import.meta.dirname, "../../../apps/play/dist/acceptance"),
    );
    let browser: Browser | undefined;
    let resumeInitialization: (() => void) | undefined;
    try {
      browser = await chromium.launch({
        channel: "chrome",
        headless: true,
      });
      const page = await browser.newPage();
      page.setDefaultTimeout(90_000);
      await installBuildProbe(page);
      await page.goto(`${server.origin}/#preset=${preset}`);
      await waitForBuild(page, 0);
      const main = page
        .frameLocator('iframe[title="Stillwind Playground Preview"]')
        .locator("main");
      await main.waitFor();
      const app = await presetSource(preset, "App.tsx");
      const css = await presetSource(preset, "stillwind.css");
      const config = await presetSource(preset, "stillwind.config.ts");
      for (const revision of [1, 2, 3]) {
        const before = await buildProbe(page);
        await editSource(page, "App.tsx", editedApp(app, revision));
        await waitForBuild(page, before.builds);
        await expect.poll(() => main.getAttribute("data-build")).toBe(String(revision));
        await expect
          .poll(() => main.evaluate((element) => getComputedStyle(element).gap))
          .toBe(`${4 * (revision + 1)}px`);
      }
      let before = await buildProbe(page);
      await editSource(
        page,
        "stillwind.css",
        `${css}\nmain { outline: 3px solid rgb(1, 2, 3); }\n`,
      );
      await waitForBuild(page, before.builds);
      await expect
        .poll(() => main.evaluate((element) => getComputedStyle(element).outlineColor))
        .toBe("rgb(1, 2, 3)");
      before = await buildProbe(page);
      await editSource(
        page,
        "stillwind.config.ts",
        config
          .replace("defineConfig({", "defineConfig(async () => ({")
          .replace(/\}\);\s*$/, "}));\n"),
      );
      await waitForBuild(page, before.builds);
      expect(await main.getAttribute("data-build")).toBe("3");
      before = await buildProbe(page);
      const wasmUrl = /\/_astro\/rolldown-binding.*\.wasm$/;
      const wasmRequest = page.waitForRequest(wasmUrl);
      const initialization = new Promise<void>((resolve) => {
        resumeInitialization = resolve;
      });
      await page.route(wasmUrl, async (route) => {
        await initialization;
        await route.continue();
      });
      await failNextBuild(page);
      await editSource(page, "App.tsx", editedApp(app, 4));
      await waitForBuild(page, before.builds, "Build error");
      expect(await page.locator(".runtime-error").innerText()).toContain(
        "Injected build worker failure",
      );
      expect(await main.getAttribute("data-build")).toBe("3");
      const failed = await buildProbe(page);
      await editSource(page, "App.tsx", editedApp(app, 5));
      await expect
        .poll(async () => (await buildProbe(page)).builds, { timeout: 90_000 })
        .toBeGreaterThan(failed.builds);
      await wasmRequest;
      resumeInitialization!();
      await waitForBuild(page, failed.builds);
      await expect.poll(() => main.getAttribute("data-build")).toBe("5");
      expect((await buildProbe(page)).workers).toBe(before.workers + 1);
      expect(await page.locator(".runtime-error").count()).toBe(0);
      expect(await main.evaluate((element) => getComputedStyle(element).outlineColor)).toBe(
        "rgb(1, 2, 3)",
      );
      await page.unroute(wasmUrl);
      before = await buildProbe(page);
      await failNextBuild(page);
      await editSource(page, "App.tsx", editedApp(app, 6));
      await waitForBuild(page, before.builds, "Build error");
      await page.route(wasmUrl, (route) => route.abort("failed"), { times: 1 });
      await editSource(page, "App.tsx", editedApp(app, 7));
      await expect
        .poll(() => page.locator(".runtime-error").innerText(), { timeout: 90_000 })
        .not.toContain("Injected build worker failure");
      await page.getByRole("status").filter({ hasText: "Build error" }).waitFor();
      const initializationFailed = await buildProbe(page);
      await editSource(page, "App.tsx", editedApp(app, 8));
      await waitForBuild(page, initializationFailed.builds);
      await expect.poll(() => main.getAttribute("data-build")).toBe("8");
      expect((await buildProbe(page)).workers).toBe(initializationFailed.workers + 1);
      expect(await page.locator(".runtime-error").count()).toBe(0);
    } finally {
      resumeInitialization?.();
      await browser?.close();
      await server.close();
    }
  },
);
