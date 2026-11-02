import { expect, test } from "vite-plus/test";

import { withPlayground } from "./playground.ts";

test("the Output panel shows the Stillwind channel with the workspace that the server opened", async (context) => {
  await withPlayground(context, async ({ page }) => {
    console.info("Playground: open the Output panel");
    await page.getByRole("tab", { name: "Output", exact: true }).click();
    const channel = page.getByRole("combobox", { name: "Output channel" });
    await expect
      .poll(() => channel.evaluate((select: HTMLSelectElement) => select.selectedOptions[0]?.text))
      .toBe("Stillwind");
    await expect
      .poll(
        async () =>
          (await page.locator(".channel-output .view-lines").innerText()).replaceAll("\u00a0", " "),
        { timeout: 30_000 },
      )
      .toMatch(/Opened 1 Stillwind workspace in \d+ ms/);
  });
});
