import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import type { ScenarioContext } from "@stillwind/e2e-harness/scenario";
import { chromium, type Browser } from "playwright";
import { expect } from "vite-plus/test";

/**
 * Creates lifecycle and fixture operations; initialize must run before browser contracts.
 */
export function createApplication(context: ScenarioContext) {
  let state!: Awaited<ReturnType<typeof prepare>>;
  let browser!: Browser;
  async function initialize() {
    state = await prepare(context);
    browser = state.browser;
  }

  async function dispose() {
    await browser?.close();
  }

  return {
    get state() {
      return state;
    },
    initialize,
    dispose,
  };
}

export type RuntimeIfApplication = ReturnType<typeof createApplication>;

async function prepare(context: ScenarioContext) {
  const root = context.directory;
  expect(root, "Use the fixture e2e task to create its work directory").toBeTruthy();

  const output = path.join(root, "output");
  await fs.mkdir(output, { recursive: true });

  const dir = await fs.mkdtemp(path.join(output, "runtime-if-"));
  const core = path
    .relative(dir, fileURLToPath(import.meta.resolve("stillwind")))
    .replaceAll("\\", "/");
  await fs.writeFile(
    path.join(dir, "root.css"),
    `
@import ${JSON.stringify(core)};

.authored {
  color: black; width: 10px;
  @apply if(--enabled):[color:red];
  @variant if(--second) { width: 30px; }
  @variant if(--enabled) { @variant if(--second) { color: blue; } }
}

.authored-nested {
  color: black; width: 10px;
  @variant if(--enabled) {
    color: red;
    @media (min-width: 0px) {
      width: 30px;
      @variant if(--second) { color: blue; }
    }
  }
}

@stillwind-utility guarded-pseudo {
  content: "yes";
  @media (min-width: 0px) { color: red; }
}
`,
  );
  await fs.writeFile(
    path.join(dir, "index.html"),
    '<script type="module" src="/main.ts"></script>',
  );
  await fs.writeFile(
    path.join(dir, "main.ts"),
    `
import { style } from "stillwind/react";

const props = style(
  'grid if(--enabled):flex w-[--if(--enabled,120px,60px)] h-[--if(--enabled,--if(--second,30px,20px),10px)] opacity-100 if(--enabled):if(--second):opacity-50! before:if(--enabled):content-["yes"]',
  {
    "--enabled": false,
    "--second": true,
  }
);
const element = document.createElement("div");
element.id = "sample";
element.className = props.className;
for (const [name, value] of Object.entries(props.style)) {
  element.style.setProperty(name, String(value));
}
document.body.append(element);

const stacked = style("grid if(--enabled):block if(--second):flex");
const child = document.createElement('div');
child.id = 'stacked';
child.className = stacked.className;
element.append(child);

const inverted = style("__NEGATED_VARIANTS__ w-[--if(!--enabled,40px,80px)]");
const inverse = document.createElement('div');
inverse.id = 'inverse';
inverse.className = inverted.className;
element.append(inverse);

const compared = style("__COMPARISON_UTILITIES__");
const comparison = document.createElement('div');
comparison.id = "comparison";
comparison.className = compared.className;
element.append(comparison);

for (const item of [child, inverse, comparison]) {
  item.style.flexShrink = "0";
}
for (const name of ['authored', 'authored-nested']) {
  const item = document.createElement("div");
  item.id = name;
  item.className = name;
  item.style.flexShrink = "0";
  element.append(item);
}
const pseudo = style("before:if(--enabled):guarded-pseudo");
const pseudoElement = document.createElement('div');
pseudoElement.id = 'nested-pseudo';
pseudoElement.className = pseudo.className;
element.append(pseudoElement);
`,
  );
  const originalSource = await fs.readFile(path.join(dir, "main.ts"), "utf8");
  const browser = await chromium.launch({
    channel: "chrome",
    headless: true,
  });
  const page = await browser.newPage();
  const support = await page.evaluate(() => ({
    revertRule: CSS.supports("display", "revert-rule"),
    cssIf: CSS.supports("width", "if(style(--x):1px;else:2px)"),
  }));
  console.log(
    JSON.stringify({
      browser: browser.version(),
      support,
    }),
  );
  expect(support.revertRule, "Browser acceptance requires revert-rule support").toBeTruthy();
  expect(support.cssIf, "Browser acceptance requires CSS if() support").toBeTruthy();

  return {
    dir,
    originalSource,
    browser,
    page,
  };
}
