import { expect, test } from "vite-plus/test";

import { replace, withPlayground } from "./playground.ts";

test("shared links restore edited components, configuration, styles, and diagnostics", async (context) => {
  await withPlayground(context, async ({ page, frame, task, buffers, openPage }) => {
    await replace(page, buffers, "App.tsx", ">Release checklist</h1>", ">Updated component</h1>");
    await frame.getByRole("heading", { name: "Updated component" }).waitFor();
    expect(await task.getAttribute("aria-checked")).toBe("true");
    const config = buffers.get("/workspace/stillwind.config.ts")!;
    await replace(
      page,
      buffers,
      "stillwind.config.ts",
      config,
      config.replace("defineConfig({", "defineConfig(async () => ({").replace("});", "}));"),
    );
    await page.getByRole("status").filter({ hasText: "Ready" }).waitFor();
    await replace(
      page,
      buffers,
      "stillwind.css",
      /--color-action: light-dark\([^;]+;/,
      "--color-action: red;",
    );
    await replace(page, buffers, "App.tsx", '"min-h-dvh', '"unknown-utility min-h-dvh');
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("unknown-utility");
    console.info("Playground: share the workspace");
    await page.getByRole("button", { name: "Share" }).click();
    await page.getByRole("button", { name: "Link copied" }).waitFor();
    const link = page.url();
    expect(link).toMatch(/#code=[\w-]+$/);
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(link);
    await page.close();
    const restored = await openPage();
    console.info("Playground: restore the shared workspace");
    await restored.goto(link);
    await restored
      .frameLocator('iframe[title="Stillwind Playground Preview"]')
      .getByRole("heading", { name: "Updated component" })
      .waitFor({ timeout: 60_000 });
    await restored.getByRole("button", { name: "App.tsx", exact: true }).click();
    await expect
      .poll(() => restored.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("unknown-utility");
    expect(await restored.getByRole("combobox", { name: "Preset" }).inputValue()).toBe(
      "release-checklist",
    );
    await expect
      .poll(
        () =>
          restored
            .frameLocator('iframe[title="Stillwind Playground Preview"]')
            .getByRole("button", { name: "All", exact: true })
            .evaluate((element) => getComputedStyle(element).backgroundColor),
        { timeout: 30_000 },
      )
      .toBe("rgb(255, 0, 0)");
    await restored.getByRole("button", { name: "stillwind.config.ts", exact: true }).click();
    const input = restored
      .getByRole("region", { name: "Editor" })
      .locator(".monaco-editor textarea.inputarea");
    await input.focus();
    await restored.keyboard.press("Control+a");
    expect(await input.inputValue()).toContain("defineConfig(async () => ({");
  });
});
