import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { stillwind } from "@stillwind/integration/vite";
import { build, createServer } from "vite";
import { expect, test } from "vite-plus/test";

test("Vite compiles vanilla macros with DOM helpers in development and production", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-vanilla-"));
  const css = path.join(root, "root.css");
  try {
    await writeFile(css, "@utility tinted {color:var(--tone);}@stillwind-slot utilities;");
    await writeFile(
      path.join(root, "index.html"),
      '<button data-tone="red">Toggle</button><script type="module" src="/entry.ts"></script>',
    );
    await writeFile(
      path.join(root, "entry.ts"),
      `import { style, applyStyle, clearStyle } from "stillwind/vanilla";
const button = document.querySelector("button")!;
const tone = button.dataset.tone ?? "red";
applyStyle(button, style("tinted", {"--tone": tone}));
button.addEventListener("click", () => clearStyle(button));`,
    );
    const server = await createServer({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [stillwind({ css })],
      server: {
        middlewareMode: true,
        watch: null,
      },
    });
    try {
      const result = await server.transformRequest("/entry.ts");
      expect(result?.code).toContain("applyStyle(button,");
      expect(result?.code).toContain("clearStyle(button)");
      expect(result?.code).toMatch(/class:\s*["']sw_/);
      expect(result?.code).not.toContain('style("tinted"');
    } finally {
      await server.close();
    }

    const result = await build({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [stillwind({ css })],
      build: {
        write: false,
        minify: false,
        cssMinify: false,
      },
    });
    if (!("output" in result)) {
      expect.fail("Expected a finite Vite build");
    }
    const javascript = result.output
      .flatMap((item) => (item.type === "chunk" ? [item.code] : []))
      .join("\n");
    const stylesheet = result.output
      .flatMap((item) =>
        item.type === "asset" && item.fileName.endsWith(".css") ? [String(item.source)] : [],
      )
      .join("\n");
    expect(javascript).toContain("applyStyle(button,");
    expect(javascript).toContain("clearStyle(button)");
    expect(javascript).not.toContain("is a compile-time macro");
    expect(stylesheet).toMatch(/color:\s*var\(--tone\)/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
