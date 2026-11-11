import { expect, test } from "vite-plus/test";

import { InlineScriptError, minifyInlineScripts } from "./minify.ts";

test("minifies classic scripts and keeps their top-level names", async () => {
  const html = `<head><script>
  // Applies the stored theme.
  function applyTheme() {
    document.documentElement.dataset.theme = "dark";
  }
  applyTheme();
</script></head>`;

  const minified = await minifyInlineScripts(html, "index.html");

  expect(minified).toMatch(/^<head><script>[^\n]*\bapplyTheme\(\)[^\n]*<\/script><\/head>$/);
  expect(minified).not.toContain("Applies the stored theme");
});

test("leaves external scripts and data blocks unchanged", async () => {
  const html = [
    `<script type="module" src="/_astro/page.js"></script>`,
    `<script type="application/ld+json">{ "@type": "WebPage" }</script>`,
  ].join("\n");

  expect(await minifyInlineScripts(html, "index.html")).toBe(html);
});

test("rejects a script that does not parse", async () => {
  await expect(minifyInlineScripts("<script>const = ;</script>", "broken.html")).rejects.toThrow(
    InlineScriptError,
  );
});
