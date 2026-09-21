import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

import { stillwind } from "@stillwind/integration/vite";
import { compileSource } from "@stillwind/native";
import * as runtime from "stillwind/react/runtime";
import { build } from "vite";

void test("compiled custom properties preserve evaluation order and inactive branches", () => {
  const result = compileSource(
    `import {style} from "stillwind/react";
    const props = style({"--x": next("first")}, active && {"--x": next("second"), "data-x": next("third")}, {"--y": next("last")});`,
    "src/custom.ts",
    "@stillwind-slot utilities;",
  );
  assert.deepEqual(result.diagnostics, []);
  assert.deepEqual(result.resources, []);
  assert.ok(result.code);
  const body = result.code.replace(/import[^;]+;/g, "");
  for (const active of [false, true]) {
    const calls: string[] = [];
    const props = runInNewContext(`${body}; props;`, {
      ...runtime,
      active,
      next: (value: string) => {
        calls.push(value);
        return value;
      },
    });
    assert.deepEqual(calls, active ? ["first", "second", "third", "last"] : ["first", "last"]);
    assert.deepEqual(props.style, { "--x": active ? "second" : "first", "--y": "last" });
    assert.equal(props["data-x"], active ? "third" : undefined);
  }
});

void test("Vite owns imported image URLs passed through inline custom properties", async () => {
  const root = mkdtempSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), ".custom-properties-"),
  );
  const put = (name: string, text: string) => writeFileSync(path.join(root, name), text);
  put("index.html", '<script type="module" src="/entry.ts"></script>');
  put("root.css", '@import "stillwind";');
  put("image.svg", '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>');
  put(
    "entry.ts",
    `import {style} from "stillwind/react";
    import imageUrl from "./image.svg?url";
    console.log(style("bg-(image:--image) w-[calc(var(--progress)*100%)]", {
      "--image": 'url("' + imageUrl + '")', "--progress": 0.5
    }));`,
  );
  try {
    const result = await build({
      configFile: false,
      root,
      base: "./",
      logLevel: "silent",
      plugins: [stillwind({ css: path.join(root, "root.css"), types: false })],
      build: { write: false, minify: false, cssMinify: false, assetsInlineLimit: 0 },
    });
    assert.ok("output" in result);
    const image = result.output.find(
      (item) => item.type === "asset" && /image-.+\.svg$/.test(item.fileName),
    );
    assert.ok(image, "Vite emits a hashed image asset");
    const js = result.output
      .filter((item) => item.type === "chunk")
      .map((item) => item.code)
      .join("\n");
    const css = result.output
      .flatMap((item) =>
        item.type === "asset" && item.fileName.endsWith(".css") ? [String(item.source)] : [],
      )
      .join("\n");
    assert.ok(js.includes(path.basename(image.fileName)));
    assert.match(css, /background-image:\s*var\(--image\)/);
    assert.match(css, /width:\s*calc\(var\(--progress\)\s*\*\s*100%\)/);
    assert.doesNotMatch(css, /__stillwind_.*resource|image\.svg/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
