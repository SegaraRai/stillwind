import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import type { ScenarioContext } from "@stillwind-internal/e2e-harness/scenario";
import { stillwind } from "@stillwind/integration/vite";
import { chromium } from "playwright";
import { createServer, preview, type InlineConfig } from "vite-plus";
import { expect } from "vite-plus/test";

/**
 * Creates lifecycle and fixture operations; initialize must run before browser contracts.
 */
export function createApplication(context: ScenarioContext) {
  let state!: Awaited<ReturnType<typeof prepare>>;
  let server:
    | Awaited<ReturnType<typeof createServer>>
    | Awaited<ReturnType<typeof preview>>
    | undefined;
  async function initialize() {
    state = await prepare(context);
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
  }

  async function prepareProduction() {
    const { put, theme } = state;
    if (server && "close" in server) {
      await server.close();
    }
    server = undefined;
    await put("root.css", theme("rgb(11,22,33)"));
  }

  async function dispose() {
    const { browser } = state;
    await browser.close();
    if (server && "close" in server) {
      await server.close();
    } else if (server) {
      await new Promise<void>((resolve) => server!.httpServer!.close(() => resolve()));
    }
  }

  return {
    get state() {
      return state;
    },
    get server() {
      return server;
    },
    set server(value: typeof server) {
      server = value;
    },
    initialize,
    prepareProduction,
    dispose,
  };
}

export type EmissionApplication = ReturnType<typeof createApplication>;

async function prepare(context: ScenarioContext) {
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
        types: false,
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

  return {
    browser,
    directory,
    put,
    theme,
    record,
    config,
    checks,
  };
}
