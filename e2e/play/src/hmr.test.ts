import { expect, test } from "vite-plus/test";

import { replace, withPlayground } from "./playground.ts";

test("HMR preserves React state through errors, asynchronous configuration, and module CSS", async (context) => {
  await withPlayground(context, async ({ page, frame, task, buffers }) => {
    await replace(page, buffers, "App.tsx", ">Release checklist</h1>", ">Updated component</h1>");
    await frame.getByRole("heading", { name: "Updated component" }).waitFor();
    expect(await task.getAttribute("aria-checked")).toBe("true");
    console.info("Playground: recover from a build error");
    const component = buffers.get("/workspace/App.tsx")!;
    const invalidComponent = component + '\nconst broken = "unterminated;';
    await replace(page, buffers, "App.tsx", component, invalidComponent);
    await page.getByRole("status").filter({ hasText: "Build error" }).waitFor();
    await expect
      .poll(() => page.locator(".runtime-error").innerText(), { timeout: 30_000 })
      .toContain("JavaScript parse error");
    expect(await task.getAttribute("aria-checked")).toBe("true");
    await replace(page, buffers, "App.tsx", invalidComponent, component);
    await page.getByRole("status").filter({ hasText: "Ready" }).waitFor();
    expect(await page.locator(".runtime-error").count()).toBe(0);
    expect(await task.getAttribute("aria-checked")).toBe("true");
    console.info("Playground: load asynchronous configuration and module CSS");
    const config = buffers.get("/workspace/stillwind.config.ts")!;
    await replace(
      page,
      buffers,
      "stillwind.config.ts",
      config,
      config.replace("defineConfig({", "defineConfig(async () => ({").replace("});", "}));"),
    );
    await page.getByRole("status").filter({ hasText: "Ready" }).waitFor();
    const beforeModuleCss = buffers.get("/workspace/App.tsx")!;
    await replace(
      page,
      buffers,
      "App.tsx",
      beforeModuleCss,
      beforeModuleCss
        .replace(
          'import { style } from "stillwind/react";',
          'import { style } from "stillwind/react";\nimport moduleCss from "stillwind/module-css";',
        )
        .replace("<main", "<main data-css-length={moduleCss.length}"),
    );
    await expect
      .poll(() => frame.locator("main").getAttribute("data-css-length"), { timeout: 30_000 })
      .toMatch(/^\d{3,}$/);
    expect(await task.getAttribute("aria-checked")).toBe("true");
    console.info("Playground: update CSS and check language diagnostics");
    await replace(
      page,
      buffers,
      "stillwind.css",
      /--color-action: light-dark\([^;]+;/,
      "--color-action: red;",
    );
    await expect
      .poll(
        () =>
          frame
            .getByRole("button", { name: "All", exact: true })
            .evaluate((element) => getComputedStyle(element).backgroundColor),
        { timeout: 30_000 },
      )
      .toBe("rgb(255, 0, 0)");
    expect(await task.getAttribute("aria-checked")).toBe("true");
  });
});
