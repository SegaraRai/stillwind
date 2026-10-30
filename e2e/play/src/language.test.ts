import { expect, test } from "vite-plus/test";

import { exampleFiles, replace, withPlayground } from "./playground.ts";

test("language features provide formatting and completions", async (context) => {
  await withPlayground(context, async ({ page, frame, task, buffers }) => {
    console.info("Playground: format source");
    await replace(
      page,
      buffers,
      "App.tsx",
      exampleFiles.get("/workspace/App.tsx")!,
      `${exampleFiles.get("/workspace/App.tsx")!}\nexport const unsorted = style("p-4 flex");\n`,
    );
    await page.getByRole("button", { name: "Format", exact: true }).click();
    const input = page
      .getByRole("region", { name: "Editor" })
      .locator(".monaco-editor textarea.inputarea");
    await expect
      .poll(
        async () =>
          (
            await page
              .getByRole("region", { name: "Editor" })
              .locator(".monaco-editor .view-lines")
              .innerText()
          ).replaceAll("\u00a0", " "),
        { timeout: 10_000 },
      )
      .toContain('style("flex p-4")');
    await input.focus();
    await page.keyboard.press("Control+a");
    const formatted = await input.inputValue();
    expect(formatted).toContain('export const unsorted = style("flex p-4");');
    buffers.set("/workspace/App.tsx", formatted);
    await replace(page, buffers, "App.tsx", formatted, exampleFiles.get("/workspace/App.tsx")!);
    console.info("Playground: update the component and request completions");
    await replace(page, buffers, "App.tsx", ">Release checklist</h1>", ">Updated component</h1>");
    await frame.getByRole("heading", { name: "Updated component" }).waitFor();
    expect(await task.getAttribute("aria-checked")).toBe("true");
    const beforeProbe = buffers.get("/workspace/App.tsx")!;
    await replace(
      page,
      buffers,
      "App.tsx",
      beforeProbe,
      `${beforeProbe}\nexport const probe = style("flex gap-panel");`,
    );
    await page.keyboard.press("Control+End");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("Control+Space");
    const suggestions = page.locator(".suggest-widget .monaco-list-row .label-name");
    await expect
      .poll(() => suggestions.allInnerTexts(), { timeout: 30_000 })
      .toContain("gap-panel");
    expect(await suggestions.allInnerTexts()).not.toContain("flex gap-panel");
    await page.keyboard.press("Escape");
    await replace(page, buffers, "App.tsx", buffers.get("/workspace/App.tsx")!, beforeProbe);
    expect(await task.getAttribute("aria-checked")).toBe("true");
  });
});
