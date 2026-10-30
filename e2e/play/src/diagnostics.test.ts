import { expect, test } from "vite-plus/test";

import { replace, withPlayground } from "./playground.ts";

test("compiler and TypeScript diagnostics follow edits and restore generated CSS", async (context) => {
  await withPlayground(context, async ({ page, frame, task, buffers }) => {
    console.info("Playground: check compiler diagnostics");
    const main = buffers.get("/workspace/App.tsx")!;
    await replace(
      page,
      buffers,
      "App.tsx",
      main,
      main.replace(/style\(\s*("[^"]+")/, "style(style($1)"),
    );
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("nested style() calls are not supported");
    await expect
      .poll(
        () => frame.locator("main").evaluate((element) => getComputedStyle(element).paddingTop),
        { timeout: 30_000 },
      )
      .toBe("0px");
    await page.getByRole("status").filter({ hasText: "Ready" }).waitFor();
    expect(await page.locator(".runtime-error").count()).toBe(0);
    await replace(page, buffers, "App.tsx", buffers.get("/workspace/App.tsx")!, main);
    await expect
      .poll(
        () => frame.locator("main").evaluate((element) => getComputedStyle(element).paddingTop),
        { timeout: 30_000 },
      )
      .toBe("24px");
    await replace(
      page,
      buffers,
      "App.tsx",
      "export function App()",
      'const wrong: number = "invalid";\nexport function App()',
    );
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("2322");
    await replace(page, buffers, "App.tsx", '"min-h-dvh p-gutter', '"min-h-dvh p-[24px]');
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("prefer-canonical-utilities");
    await replace(page, buffers, "App.tsx", '"min-h-dvh p-[24px]', '"min-h-dvh p-gutter');
    await replace(page, buffers, "App.tsx", '"min-h-dvh', '"unknown-utility min-h-dvh');
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("unknown-utility");
    expect(await task.getAttribute("aria-checked")).toBe("true");
  });
});
