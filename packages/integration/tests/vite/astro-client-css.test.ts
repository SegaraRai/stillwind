import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { stillwind } from "@stillwind/integration/astro";
import { build } from "astro";
import { expect, test } from "vite-plus/test";

test("Astro delivers CSS generated only by an imported client script", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-astro-client-css-"));
  try {
    await mkdir(path.join(root, "src/pages"), { recursive: true });
    await writeFile(
      path.join(root, "root.css"),
      "@utility client-only { margin-left:123.456px; } @stillwind-slot utilities;",
    );
    await writeFile(
      path.join(root, "src/client.ts"),
      `import { style, applyStyle } from 'stillwind/vanilla';
applyStyle(document.querySelector('button')!, style('client-only'));`,
    );
    await writeFile(
      path.join(root, "src/pages/index.astro"),
      `<html><head><title>Client styles</title></head><body><button>Action</button>
<script>import '../client';</script></body></html>`,
    );
    await build({
      root,
      configFile: false,
      logLevel: "silent",
      integrations: [
        stillwind({
          css: path.join(root, "root.css"),
        }),
      ],
      build: {
        inlineStylesheets: "never",
      },
    });
    const dist = path.join(root, "dist");
    const files = (await readdir(dist, { recursive: true }))
      .map(String)
      .filter((file) => /\.(?:css|html|js)$/.test(file));
    const assets = await Promise.all(
      files.map(async (file) => ({
        file,
        text: await readFile(path.join(dist, file), "utf8"),
      })),
    );
    const stylesheet = assets.find(
      ({ file, text }) => file.endsWith(".css") && text.includes("123.456px"),
    );
    expect(stylesheet).toBeDefined();
    const pagesAndScripts = assets
      .filter(({ file }) => /\.(?:html|js)$/.test(file))
      .map(({ text }) => text)
      .join("\n");
    expect(pagesAndScripts).toContain(stylesheet!.file.replaceAll("\\", "/"));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
