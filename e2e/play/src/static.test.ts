import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { resolve, sep, extname } from "node:path";

import { chromium, type Page } from "playwright";
import { expect, test } from "vite-plus/test";

import { exampleFiles } from "../../../apps/play/src/workspace/example.ts";

test("static files provide document formatting, semantic diagnostics, cross-module builds, and React/CSS HMR", async () => {
  const directory = resolve(import.meta.dirname, "../../../apps/play/dist/acceptance");
  const requests: string[] = [];
  const server = createServer(async (request, response) => {
    requests.push(request.method ?? "");
    const path = resolve(
      directory,
      "." +
        decodeURIComponent(new URL(request.url!, "http://localhost").pathname)
          .replace(/^\/play/, "")
          .replace(/\/$/, "/index.html"),
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
  try {
    const page = await browser.newPage();
    const buffers = new Map(exampleFiles);
    const errors: string[] = [];
    const remote: string[] = [];
    page.on("request", (request) => {
      if (/^https?:/.test(request.url()) && new URL(request.url()).hostname !== "127.0.0.1") {
        remote.push(request.url());
      }
    });
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(`http://127.0.0.1:${address.port}/play/`);
    const frame = page.frameLocator('iframe[title="Stillwind Play preview"]');
    await frame.getByRole("heading", { name: "Stillwind Play" }).waitFor({ timeout: 60_000 });
    await expect
      .poll(() =>
        frame.locator("main").evaluate((element) => getComputedStyle(element).backgroundColor),
      )
      .not.toBe("rgba(0, 0, 0, 0)");
    await frame.getByRole("button", { name: "Count: 0" }).click();
    await page.getByRole("button", { name: "Component.tsx", exact: true }).click();
    await page.getByRole("button", { name: "Format", exact: true }).click();
    const input = page.locator(".monaco-editor textarea.inputarea");
    await expect
      .poll(
        async () =>
          (await page.locator(".monaco-editor .view-lines").innerText()).replaceAll("\u00a0", " "),
        { timeout: 10_000 },
      )
      .toContain("return (");
    await input.focus();
    await page.keyboard.press("Control+a");
    const formatted = await input.inputValue();
    expect(formatted).toContain('style("px-4 py-2 text-white bg-accent rounded-md")');
    buffers.set("/workspace/Component.tsx", formatted);
    await replace(
      page,
      buffers,
      "Component.tsx",
      formatted,
      exampleFiles.get("/workspace/Component.tsx")!,
    );
    await replace(
      page,
      buffers,
      "Component.tsx",
      "<h1>Stillwind Play</h1>",
      "<h1>Updated component</h1>",
    );
    await frame.getByRole("heading", { name: "Updated component" }).waitFor();
    expect(await frame.getByRole("button").innerText()).toBe("Count: 1");
    const beforeProbe = buffers.get("/workspace/Component.tsx")!;
    await replace(
      page,
      buffers,
      "Component.tsx",
      beforeProbe,
      `${beforeProbe}\nexport const probe = style("flex gap-2");`,
    );
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("Control+Space");
    const suggestions = page.locator(".suggest-widget .monaco-list-row .label-name");
    await expect.poll(() => suggestions.allInnerTexts(), { timeout: 30_000 }).toContain("gap-2");
    expect(await suggestions.allInnerTexts()).not.toContain("flex gap-2");
    await page.keyboard.press("Escape");
    await replace(
      page,
      buffers,
      "Component.tsx",
      buffers.get("/workspace/Component.tsx")!,
      beforeProbe,
    );
    await replace(
      page,
      buffers,
      "Component.tsx",
      'style(card, "p-4 rounded-md")',
      'style(card, style("p-4 rounded-md"))',
    );
    await page.getByRole("button", { name: /^Diagnostics/ }).click();
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
    await replace(
      page,
      buffers,
      "Component.tsx",
      'style(card, style("p-4 rounded-md"))',
      'style(card, "p-4 rounded-md")',
    );
    await expect
      .poll(
        () => frame.locator("main").evaluate((element) => getComputedStyle(element).paddingTop),
        { timeout: 30_000 },
      )
      .toBe("16px");
    const component = buffers.get("/workspace/Component.tsx")!;
    const invalidComponent = component + '\nconst broken = "unterminated;';
    await replace(page, buffers, "Component.tsx", component, invalidComponent);
    await page.getByRole("status").filter({ hasText: "Build error" }).waitFor();
    await expect
      .poll(() => page.locator(".runtime-error").innerText(), { timeout: 30_000 })
      .toContain("JavaScript parse error");
    expect(await frame.getByRole("button").innerText()).toBe("Count: 1");
    await replace(page, buffers, "Component.tsx", invalidComponent, component);
    await page.getByRole("status").filter({ hasText: "Ready" }).waitFor();
    expect(await page.locator(".runtime-error").count()).toBe(0);
    expect(await frame.getByRole("button").innerText()).toBe("Count: 1");
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
      "Component.tsx",
      'import { useState } from "react";',
      'import { useState } from "react";\nimport moduleCss from "stillwind/module-css";',
    );
    await replace(
      page,
      buffers,
      "Component.tsx",
      "<main {...style",
      "<main data-css-length={moduleCss.length} {...style",
    );
    await expect
      .poll(() => frame.locator("main").getAttribute("data-css-length"), { timeout: 30_000 })
      .toMatch(/^\d{3,}$/);
    expect(await frame.getByRole("button").innerText()).toBe("Count: 1");
    await replace(page, buffers, "stillwind.css", "oklch(.6 .2 250)", "red");
    await expect
      .poll(
        () =>
          frame
            .getByRole("button")
            .evaluate((element) => getComputedStyle(element).backgroundColor),
        { timeout: 30_000 },
      )
      .toBe("rgb(255, 0, 0)");
    expect(await frame.getByRole("button").innerText()).toBe("Count: 1");
    await replace(
      page,
      buffers,
      "Component.tsx",
      "export function App()",
      'const wrong: number = "invalid";\nexport function App()',
    );
    await page.getByRole("button", { name: /^Diagnostics/ }).click();
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("2322");
    await replace(page, buffers, "Component.tsx", "p-4 rounded-md", "p-[16px] rounded-md");
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("prefer-canonical-utilities");
    await replace(page, buffers, "Component.tsx", "p-[16px] rounded-md", "p-4 rounded-md");
    await replace(
      page,
      buffers,
      "Component.tsx",
      "p-4 rounded-md",
      "unknown-utility p-4 rounded-md",
    );
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("unknown-utility");
    expect(await frame.getByRole("button").innerText()).toBe("Count: 1");
    expect(errors).toEqual([]);
    expect(remote).toEqual([]);
    expect(new Set(requests)).toEqual(new Set(["GET"]));
  } finally {
    await browser.close();
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});

/** Edits through Monaco's accessible textarea, exercising actual extension synchronization. */
async function replace(
  page: Page,
  buffers: Map<string, string>,
  file: string,
  from: string,
  to: string,
): Promise<void> {
  const key = `/workspace/${file}`;
  const text = buffers.get(key)!.replace(from, to);
  buffers.set(key, text);
  await page.getByRole("button", { name: file, exact: true }).click();
  const input = page.locator(".monaco-editor textarea.inputarea");
  await input.focus();
  await page.keyboard.press("Control+a");
  await page.keyboard.insertText(text);
}
