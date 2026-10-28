import { readdir, readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { relative, resolve, sep, extname } from "node:path";

import { chromium, type Page } from "playwright";
import { expect, test } from "vite-plus/test";

// The site embeds these files as the initial workspace. Vitest stubs CSS imports, so the test
// reads them from disk instead of importing the playground module.
const exampleDirectory = resolve(import.meta.dirname, "../../../apps/play/example");
const exampleFiles = new Map(
  await Promise.all(
    (await readdir(exampleDirectory, { recursive: true, withFileTypes: true }))
      .filter((entry) => entry.isFile())
      .map(async (entry) => {
        const path = resolve(entry.parentPath, entry.name);
        const name = relative(exampleDirectory, path).split(sep).join("/");
        return [`/workspace/${name}`, await readFile(path, "utf8")] as const;
      }),
  ),
);

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
    page.on("pageerror", (error) => {
      // VS Code's background tokenizer can apply worker results to a model that this test has just
      // replaced entirely. That race throws inside setTokensAndStates and leaves the editor usable.
      if (!error.stack?.includes("setTokensAndStates")) {
        errors.push(error.message);
      }
    });
    await page.goto(`http://127.0.0.1:${address.port}/play/`);
    const frame = page.frameLocator('iframe[title="Stillwind Play preview"]');
    await frame.getByRole("heading", { name: "Release checklist" }).waitFor({ timeout: 60_000 });
    await expect
      .poll(() =>
        frame.locator("main").evaluate((element) => getComputedStyle(element).backgroundColor),
      )
      .not.toBe("rgba(0, 0, 0, 0)");
    const task = frame.getByRole("checkbox", { name: "Update the migration guide" });
    await task.click();
    expect(await task.getAttribute("aria-checked")).toBe("true");
    await replace(
      page,
      buffers,
      "Component.tsx",
      exampleFiles.get("/workspace/Component.tsx")!,
      `${exampleFiles.get("/workspace/Component.tsx")!}\nexport const unsorted = style("p-4 flex");\n`,
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
      ">Release checklist</h1>",
      ">Updated component</h1>",
    );
    await frame.getByRole("heading", { name: "Updated component" }).waitFor();
    expect(await task.getAttribute("aria-checked")).toBe("true");
    const beforeProbe = buffers.get("/workspace/Component.tsx")!;
    await replace(
      page,
      buffers,
      "Component.tsx",
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
    await replace(
      page,
      buffers,
      "Component.tsx",
      buffers.get("/workspace/Component.tsx")!,
      beforeProbe,
    );
    const main = buffers.get("/workspace/Component.tsx")!;
    await replace(
      page,
      buffers,
      "Component.tsx",
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
    await replace(page, buffers, "Component.tsx", buffers.get("/workspace/Component.tsx")!, main);
    await expect
      .poll(
        () => frame.locator("main").evaluate((element) => getComputedStyle(element).paddingTop),
        { timeout: 30_000 },
      )
      .toBe("24px");
    const component = buffers.get("/workspace/Component.tsx")!;
    const invalidComponent = component + '\nconst broken = "unterminated;';
    await replace(page, buffers, "Component.tsx", component, invalidComponent);
    await page.getByRole("status").filter({ hasText: "Build error" }).waitFor();
    await expect
      .poll(() => page.locator(".runtime-error").innerText(), { timeout: 30_000 })
      .toContain("JavaScript parse error");
    expect(await task.getAttribute("aria-checked")).toBe("true");
    await replace(page, buffers, "Component.tsx", invalidComponent, component);
    await page.getByRole("status").filter({ hasText: "Ready" }).waitFor();
    expect(await page.locator(".runtime-error").count()).toBe(0);
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
    const beforeModuleCss = buffers.get("/workspace/Component.tsx")!;
    await replace(
      page,
      buffers,
      "Component.tsx",
      beforeModuleCss,
      beforeModuleCss
        .replace(
          'import { useState } from "react";',
          'import { useState } from "react";\nimport moduleCss from "stillwind/module-css";',
        )
        .replace("<main", "<main data-css-length={moduleCss.length}"),
    );
    await expect
      .poll(() => frame.locator("main").getAttribute("data-css-length"), { timeout: 30_000 })
      .toMatch(/^\d{3,}$/);
    expect(await task.getAttribute("aria-checked")).toBe("true");
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
    await replace(
      page,
      buffers,
      "Component.tsx",
      "export function App()",
      'const wrong: number = "invalid";\nexport function App()',
    );
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("2322");
    await replace(page, buffers, "Component.tsx", '"min-h-dvh p-gutter', '"min-h-dvh p-[24px]');
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("prefer-canonical-utilities");
    await replace(page, buffers, "Component.tsx", '"min-h-dvh p-[24px]', '"min-h-dvh p-gutter');
    await replace(page, buffers, "Component.tsx", '"min-h-dvh', '"unknown-utility min-h-dvh');
    await expect
      .poll(() => page.locator(".diagnostics-output").innerText(), { timeout: 30_000 })
      .toContain("unknown-utility");
    expect(await task.getAttribute("aria-checked")).toBe("true");
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
  from: string | RegExp,
  to: string,
): Promise<void> {
  const key = `/workspace/${file}`;
  const text = buffers.get(key)!.replace(from, to);
  buffers.set(key, text);
  await page.getByRole("button", { name: file, exact: true }).click();
  const input = page
    .getByRole("region", { name: "Editor" })
    .locator(".monaco-editor textarea.inputarea");
  await input.focus();
  await page.keyboard.press("Control+a");
  await page.keyboard.insertText(text);
}
