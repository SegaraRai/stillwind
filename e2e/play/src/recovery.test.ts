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
    const errors: string[] = [];
    try {
      browser = await chromium.launch({
        channel: "chrome",
        headless: true,
      });
      const page = await browser.newPage();
      page.on("pageerror", (error) => errors.push(String(error)));
      page.on("console", (message) => {
        if (message.type() === "error") {
          errors.push(message.text());
        }
      });
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
      const inventory = (await (
        await page.request.get(`${server.origin}/workspace.json`)
      ).json()) as [string, string][];
      const manifest = JSON.parse(
        inventory.find(([id]) => id === "/workspace/package.json")![1],
      ) as {
        devDependencies: Record<string, string>;
      };
      expect(Object.keys(manifest.devDependencies)).toEqual(["@iconify-json/lucide"]);
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
      const generation = before.snapshots.at(-1)!.generation;
      for (const revision of [1, 2, 3]) {
        before = await buildProbe(page);
        await editSource(
          page,
          "stillwind.css",
          `${css.replace(/--color-canvas:[^;]+;/, `--color-canvas: rgb(${revision}, 2, 3);`)}\nmain { outline: 3px solid rgb(1, 2, 3); }\n`,
        );
        await waitForBuild(page, before.builds);
        expect((await buildProbe(page)).snapshots.at(-1)!.generation).toBe(generation);
        await expect
          .poll(() => main.evaluate((element) => getComputedStyle(element).backgroundColor))
          .toBe(`rgb(${revision}, 2, 3)`);
      }
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
      expect((await buildProbe(page)).snapshots.at(-1)!.generation).toBeGreaterThan(generation);
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
      const transferred = await buildProbe(page);
      expect(transferred.snapshots.every((snapshot) => snapshot.deferred > 0)).toBe(true);
      expect(
        Math.max(...transferred.snapshots.map((snapshot) => snapshot.characters)),
      ).toBeLessThan(100_000);
      expect(await main.evaluate((element) => getComputedStyle(element).outlineColor)).toBe(
        "rgb(1, 2, 3)",
      );
      await page.unroute(wasmUrl);
      before = await buildProbe(page);
      const tsconfig = JSON.parse(await presetSource(preset, "tsconfig.json")) as {
        compilerOptions: Record<string, unknown>;
      };
      tsconfig.compilerOptions.resolveJsonModule = true;
      await editSource(page, "tsconfig.json", `${JSON.stringify(tsconfig, null, 2)}\n`);
      await waitForBuild(page, before.builds);
      expect((await buildProbe(page)).snapshots.at(-1)!.generation).toBeGreaterThan(
        before.snapshots.at(-1)!.generation,
      );
      expect(await main.getAttribute("data-build")).toBe("5");
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
    } catch (error) {
      const page = browser?.contexts().flatMap((context) => context.pages())[0];
      console.error(
        "Playground recovery failure",
        JSON.stringify({
          preset,
          errors,
          status: await page?.getByRole("status").allTextContents(),
          error: await page?.locator(".runtime-error").allTextContents(),
          diagnostics: await page?.locator(".diagnostics-output").allTextContents(),
          probe: page ? await buildProbe(page).catch(() => undefined) : undefined,
        }),
      );
      throw error;
    } finally {
      resumeInitialization?.();
      await browser?.close();
      await server.close();
    }
  },
);
