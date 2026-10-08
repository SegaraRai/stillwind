import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import { build } from "vite";
import { expect, test } from "vite-plus/test";

test("a configured generator names the classes the plugin emits", async () => {
  const root = mkdtempSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), ".test-naming-"),
  );
  const put = (name: string, text: string) => writeFileSync(path.join(root, name), text);
  put("index.html", '<script type="module" src="/entry.ts"></script>');
  put("root.css", '@import "stillwind"; @utility flex {display:flex} @stillwind-slot utilities;');
  put("entry.ts", 'import {style} from "stillwind/react"; console.log(style("flex"));');
  try {
    const result = await build({
      configFile: false,
      root,
      base: "./",
      logLevel: "silent",
      plugins: [
        stillwind({
          css: path.join(root, "root.css"),
          naming: {
            utilityClass: () => "app-class",
          },
          namingRevision: "test-naming",
        }),
      ],
      build: { write: false, minify: false, cssMinify: false },
    });
    if (!("output" in result)) {
      expect.fail('Unexpected value: "output" in result');
    }
    const code = result.output
      .filter((item) => item.type === "chunk")
      .map((item) => item.code)
      .join("\n");
    const css = result.output
      .flatMap((item) =>
        item.type === "asset" && item.fileName.endsWith(".css") ? [String(item.source)] : [],
      )
      .join("\n");
    // The class the host named is the one the markup carries and the stylesheet selects.
    expect(code).toMatch(/app-class/);
    expect(css).toMatch(/\.app-class\{/);
    expect(code).not.toMatch(/sw_/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a stylesheet the producer authors owns the names it writes", async () => {
  const root = mkdtempSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), ".test-authored-"),
  );
  const put = (name: string, text: string) => writeFileSync(path.join(root, name), text);
  put("index.html", '<script type="module" src="/entry.ts"></script>');
  put("root.css", '@import "stillwind"; @utility flex {display:flex} @stillwind-slot utilities;');
  put("app.css", ".app-class{color:red}");
  put(
    "entry.ts",
    'import "./app.css"; import {style} from "stillwind/react"; console.log(style("flex"));',
  );
  try {
    // The generated class and the authored selector would name two different things.
    await expect(
      build({
        configFile: false,
        root,
        base: "./",
        logLevel: "silent",
        plugins: [
          stillwind({
            css: path.join(root, "root.css"),
            naming: {
              utilityClass: () => "app-class",
            },
            namingRevision: "test-naming",
          }),
        ],
        build: {
          write: false,
          minify: false,
          cssMinify: false,
        },
      }),
    ).rejects.toThrow(/app-class/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
