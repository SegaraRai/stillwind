import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { build } from "vite-plus";

import { stillwind } from "../src/index.ts";

void test("build retains late and lazy producer CSS without collecting transforms in CSS load", async () => {
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
  const root = mkdtempSync(path.join(repositoryRoot, ".session-test-"));
  const put = (name: string, content: string) => writeFileSync(path.join(root, name), content);
  put("index.html", '<script type="module" src="/main.ts"></script>');
  put("root.css", ".document-only{color:purple;}");
  put(
    "main.ts",
    'import "virtual:stillwind.css"; import { a } from "./slow"; console.log(a); window.loadLazy = () => import("./lazy");',
  );
  put(
    "slow.ts",
    'import { style } from "stillwind/react"; export const a = style("[color:rgb(11,22,33)]");',
  );
  put(
    "lazy.ts",
    'import { style } from "stillwind/react"; export const b = style("[color:rgb(44,55,66)]");',
  );
  put(
    "unused.ts",
    'import { style } from "stillwind/react"; export const c = style("[color:rgb(77,88,99)]");',
  );
  try {
    const output = await build({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [
        {
          name: "delay-producer",
          enforce: "pre",
          async transform(_, id) {
            if (id.endsWith("/slow.ts")) await new Promise((resolve) => setTimeout(resolve, 200));
          },
        },
        stillwind({ css: path.join(root, "root.css"), types: false }),
      ],
      build: { write: false, minify: false, cssMinify: false },
    });
    if (!("output" in output)) throw new Error("expected a single build output");
    const assets = output.output
      .filter((asset) => asset.type === "asset")
      .filter((asset) => asset.fileName.endsWith(".css"));
    const css = assets.map((asset) => String(asset.source)).join("\n");
    assert.match(css, /11,\s*22,\s*33/);
    assert.match(css, /44,\s*55,\s*66/);
    assert.doesNotMatch(css, /77,\s*88,\s*99/);
    assert.equal(css.match(/\.document-only/g)?.length, 1);
    assert.ok(assets.some((asset) => asset.fileName.includes("lazy")));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
