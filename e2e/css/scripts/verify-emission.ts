import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import type { ScenarioContext } from "@stillwind-internal/e2e-harness/scenario";
import { stillwind } from "@stillwind/integration/vite";
import { chromium } from "playwright";
import { stylesheetLinks } from "stillwind/css";
import { build, createServer, preview, type InlineConfig } from "vite";
import { afterAll, beforeAll, describe, test, expect } from "vite-plus/test";

export default function register(context: ScenarioContext): void {
  async function prepare() {
    const root = context.directory;
    expect(root, "Use the fixture e2e task to create its work directory").toBeTruthy();
    const directory = await fs.mkdtemp(path.join(root, "output/emission-"));
    const put = (name: string, source: string) => fs.writeFile(path.join(directory, name), source);
    const core = path
      .relative(directory, fileURLToPath(import.meta.resolve("stillwind")))
      .replaceAll("\\", "/");
    const theme = (color: string) =>
      `@import ${JSON.stringify(core)}; @utility paint{color:${color};} .common-marker{box-sizing:border-box;}`;
    await put("root.css", theme("rgb(11,22,33)"));
    await put(
      "index.html",
      '<link rel="icon" href="data:,">\n<div id="target" style="width:300px;height:300px">Target</div>\n<div id="component"></div>\n<script type="module" src="/main.ts"></script>\n',
    );
    await put(
      "base.ts",
      'import { style } from "stillwind/react";\n\nexport const base = style("m-10 hover:m-20");\n',
    );
    await put(
      "a.ts",
      'import { style } from "stillwind/react";\nimport { base } from "./base";\n\nexport const props=style(base,"m-3 paint");\n',
    );
    await put(
      "b.ts",
      'import { style } from "stillwind/react";\nimport { base } from "./base";\n\nexport const props=style(base,"m-5 hover:m-30");\n',
    );
    for (const name of ["one", "two"]) {
      await put(
        `${name}.ts`,
        'import { style } from "stillwind/react";\nimport { base } from "./base";\nimport { css, parts } from "stillwind/module-css";\n\nexport { css, parts };\nexport const props = style(base, "m-3 paint");\n',
      );
    }
    await put(
      "main.ts",
      `
import * as one from "./one";
import * as two from "./two";
import { styleSheets } from "stillwind/css";

const routes = {
  a: () => import("./a"),
  b: () => import("./b"),
};
window.goto = async (route) => {
  const { props } = await routes[route]();
  document.querySelector("#target").className = props.className;
};

const root = document.querySelector("#component").shadowRoot ?? document.querySelector("#component").attachShadow({ mode: "open" });
root.adoptedStyleSheets = styleSheets(one.parts, document);
const second = styleSheets(two.parts, document);
window.sharedSheet = one.parts.some((part, index) => part.css.includes(".common-marker") && second.includes(root.adoptedStyleSheets[index]));
root.innerHTML = '<div id="shadow">Shadow</div>';
root.querySelector('#shadow').className = one.props.className;
window.goto(new URL(location.href).searchParams.get("route") ?? "a");
if (import.meta.hot) {
  import.meta.hot.accept();
}
`,
    );
    const config = (emission: "single" | "split" = "single"): InlineConfig => ({
      configFile: false,
      root: directory,
      logLevel: "silent",
      plugins: [
        stillwind({
          css: path.join(directory, "root.css"),
          emission,
        }),
      ],
    });
    const browser = await chromium.launch({
      channel: "chrome",
      headless: true,
    });

    const checks: string[] = [];
    const record = (name: string) => {
      checks.push(name);
      console.log(name);
    };
    return { browser, directory, put, theme, record, config, checks };
  }
  let state!: Awaited<ReturnType<typeof prepare>>;
  let server:
    | Awaited<ReturnType<typeof createServer>>
    | Awaited<ReturnType<typeof preview>>
    | undefined;
  const check = async (url: string, mode: string) => {
    const { browser, record } = state;
    const classes: Record<string, string | null> = {};
    for (const first of ["a", "b"]) {
      const page = await browser.newPage();
      page.setDefaultTimeout(10000);
      const errors: string[] = [];
      page.on("pageerror", (error) => {
        errors.push(String(error));
        console.error(error);
      });
      page.on("console", (message) => {
        if (message.type() === "error") {
          console.error(message.text());
        }
      });
      await page.goto(`${url}?route=${first}`);
      await page.waitForFunction(() => typeof window.goto === "function" && window.sharedSheet);
      for (const route of [first, first === "a" ? "b" : "a"]) {
        await page.evaluate((route) => window.goto(route), route);
        await page.mouse.move(0, 0);
        await page.waitForFunction(
          (expected: string) =>
            getComputedStyle(document.querySelector<HTMLElement>("#target")!).marginLeft ===
            expected,
          route === "a" ? "12px" : "20px",
        );
        classes[route] = await page.locator("#target").getAttribute("class");
        await page.locator("#target").hover();
        await page.waitForFunction(
          (expected: string) =>
            getComputedStyle(document.querySelector<HTMLElement>("#target")!).marginLeft ===
            expected,
          route === "a" ? "80px" : "120px",
        );
      }
      await page.mouse.move(0, 0);
      const shadow = await page.evaluate(() => {
        const style = getComputedStyle(
          document
            .querySelector<HTMLElement>("#component")!
            .shadowRoot!.querySelector<HTMLElement>("#shadow")!,
        );
        return { margin: style.marginLeft, color: style.color };
      });
      expect(shadow).toEqual({ margin: "12px", color: "rgb(11, 22, 33)" });
      expect(errors).toEqual([]);
      await page.close();
    }
    record(`${mode}: reverse route order, hover, shadow closure and shared sheets`);
    return classes;
  };

  beforeAll(async () => {
    state = await prepare();
    const { config } = state;
    server = await createServer({
      ...config(),
      server: {
        port: 0,
      },
      css: {
        devSourcemap: true,
      },
    });
    await server.listen();
  });

  test("development: composes lazy CSS in both loading orders and shares Shadow DOM sheets", async () => {
    await check(server!.resolvedUrls!.local[0], "dev");
  });

  test("development: updates only CSS while retaining the style node and document", async () => {
    const { browser, put, theme, record } = state;
    const page = await browser.newPage();
    await page.goto(server!.resolvedUrls!.local[0]);
    await page.waitForFunction(
      () =>
        getComputedStyle(document.querySelector<HTMLElement>("#target")!).color ===
        "rgb(11, 22, 33)",
    );
    let reloads = 0;
    page.on("framenavigated", (frame) => {
      if (frame === page.mainFrame()) {
        reloads++;
      }
    });
    await page.evaluate(() => {
      window.originalStyle = document.querySelector<HTMLElement>("style[data-stillwind]")!;
    });
    try {
      await put("root.css", theme("rgb(44,55,66)"));
      await page.waitForFunction(
        () =>
          getComputedStyle(document.querySelector<HTMLElement>("#target")!).color ===
          "rgb(44, 55, 66)",
      );
      expect(reloads).toBe(0);
      expect(
        await page.evaluate(
          () =>
            window.originalStyle === document.querySelector<HTMLElement>("style[data-stillwind]")!,
        ),
      ).toBe(true);
      record("dev: CSS-only update retains style node and avoids reload");
    } finally {
      await put("root.css", theme("rgb(11,22,33)"));
      await page.close();
    }
  });

  test("development: retracts CSS when a module is deleted", async () => {
    const { browser, directory, put, record } = state;
    const page = await browser.newPage();
    await page.goto(server!.resolvedUrls!.local[0]);
    await page.waitForFunction(() => typeof window.goto === "function" && window.sharedSheet);
    try {
      await put(
        "added.ts",
        'import { style } from "stillwind/react";\n\nexport const props=style("m-7");\n',
      );
      await page.evaluate(`(async () => {
      const { props } = await import("/added.ts");
      document.querySelector("#target").className = props.className;
    })()`);
      await page.mouse.move(0, 0);
      await page.waitForFunction(
        () =>
          getComputedStyle(document.querySelector<HTMLElement>("#target")!).marginLeft === "28px",
      );
      await fs.unlink(path.join(directory, "added.ts"));
      await page.waitForFunction(
        () =>
          getComputedStyle(document.querySelector<HTMLElement>("#target")!).marginLeft !== "28px",
      );
      record("dev: module addition and deletion retract CSS");
    } finally {
      await fs.rm(path.join(directory, "added.ts"), { force: true });
      await page.close();
    }
  });

  describe("production", () => {
    beforeAll(async () => {
      const { put, theme } = state;
      if (server && "close" in server) {
        await server.close();
      }
      server = undefined;
      await put("root.css", theme("rgb(11,22,33)"));
    });

    for (const [emission, cssMinify] of [
      ["single", false],
      ["split", false],
      ["single", true],
      ["split", true],
    ] as const) {
      test(`${emission}, minify=${cssMinify}: emits CSS, deduplicates shared rules and styles SSR HTML`, async () => {
        const { browser, put, record, config } = state;

        const mode = `${emission}${cssMinify ? " minified" : ""}`;
        const result = await build({
          ...config(emission),
          build: { minify: false, cssMinify, sourcemap: true },
        });
        if (Array.isArray(result) || !("output" in result)) {
          expect.fail("Expected one build output");
        }
        const output = result.output;
        const css = output.filter(
          (item) => item.type === "asset" && item.fileName.endsWith(".css"),
        );
        expect(css.length > 0).toBeTruthy();
        if (emission === "single") {
          expect(css.length).toBe(1);
        } else {
          expect(
            css.length < 10,
            "split output packs adjacent rules into producer partitions",
          ).toBeTruthy();
        }
        const manifestAsset = output.find(
          (item) => item.type === "asset" && item.fileName === "stillwind-manifest.json",
        );
        if (!manifestAsset || manifestAsset.type !== "asset") {
          expect.fail("Missing manifest asset");
        }
        const manifest = JSON.parse(String(manifestAsset.source));
        expect(manifest.modules["a.ts"] && manifest.modules["b.ts"]).toBeTruthy();
        const code = output
          .filter((item) => item.type === "chunk")
          .map((item) => item.code)
          .join("\n");
        expect((code.match(/\.common-marker\s*\{/g) ?? []).length).toBe(1);
        server = await preview({
          ...config(emission),
          preview: {
            port: 0,
          },
        });
        const classes = await check(server!.resolvedUrls!.local[0], mode);
        for (const route of ["a", "b"]) {
          const links = stylesheetLinks(manifest, [`${route}.ts`]);
          await put(
            "dist/ssr.html",
            `${links.map((part) => `<link rel="stylesheet" href="${part.href}" data-stillwind-part="${part.id}" data-stillwind-order="${part.order}">`).join("")}<div id="target" class="${classes[route]}">SSR</div>`,
          );
          const page = await browser.newPage({ javaScriptEnabled: false });
          await page.goto(`${server!.resolvedUrls!.local[0]}ssr.html`);
          expect(
            await page
              .locator("#target")
              .evaluate((element) => getComputedStyle(element).marginLeft),
          ).toBe(route === "a" ? "12px" : "20px");
          await page.close();
        }
        record(`${mode}: SSR applies selected CSS without JavaScript`);
        await new Promise<void>((resolve) => server!.httpServer!.close(() => resolve()));
        server = undefined;
      });
    }
  });

  for (const mode of ["dev", "build"]) {
    test(`Web Components ${mode}: adopts explicit CSS without document styles`, async () => {
      const { browser, record, config } = state;

      const manual = {
        ...config(),
        plugins: [
          stillwind({
            css: "./root.css",
            automatic: false,
          }),
        ],
      };
      if (mode === "dev") {
        server = await createServer({
          ...manual,
          server: {
            port: 0,
          },
        });
        await server.listen();
      } else {
        const result = await build(manual);
        if (Array.isArray(result) || !("output" in result)) {
          expect.fail("Expected one build output");
        }
        const output = result.output;
        expect(!output.some((item) => item.fileName.endsWith(".css"))).toBeTruthy();
        server = await preview({
          ...manual,
          preview: {
            port: 0,
          },
        });
      }
      const page = await browser.newPage();
      await page.goto(server!.resolvedUrls!.local[0]);
      await page.waitForFunction(
        () =>
          window.sharedSheet &&
          getComputedStyle(
            document
              .querySelector<HTMLElement>("#component")!
              .shadowRoot!.querySelector<HTMLElement>("#shadow")!,
          ).marginLeft === "12px",
      );
      expect(
        await page.locator("#target").evaluate((element) => getComputedStyle(element).marginLeft),
      ).toBe("0px");
      expect(await page.locator("style[data-stillwind]").count()).toBe(0);
      await page.close();
      if ("close" in server) {
        await server.close();
      } else {
        await new Promise<void>((resolve) => server!.httpServer!.close(() => resolve()));
      }
      server = undefined;
      record(`Web Components ${mode}: explicit closure has no automatic document styles`);
    });
  }

  afterAll(async () => {
    const { browser } = state;
    await browser.close();
    if (server && "close" in server) {
      await server.close();
    } else if (server) {
      await new Promise<void>((resolve) => server!.httpServer!.close(() => resolve()));
    }
  });
}

declare global {
  interface Window {
    goto: (route: string) => Promise<void>;
    sharedSheet: boolean;
    originalStyle: Element | null;
  }
}
