import { expect, test } from "vite-plus/test";

import { extractTailwindBanner, rustString } from "./banner-extraction.ts";

test("extracts the original comment and rejects missing, ambiguous, or stale notices", () => {
  const banner = "/*! tailwindcss v4.3.3 | 日本語\n  notice */";
  expect(extractTailwindBanner(`${banner}\na{display:block}`, "4.3.3")).toBe(banner);
  for (const css of ["a{}", `${banner}${banner}`, banner.replace("4.3.3", "4.3.2")]) {
    expect(() => extractTailwindBanner(css, "4.3.3")).toThrow("Cannot uniquely extract");
  }
  expect(() => extractTailwindBanner(`a{content:'${banner}'}`, "4.3.3")).toThrow();
});

test("quotes Rust strings while preserving Unicode and escaping control characters", () => {
  expect(rustString('日本語 "\\\r\n\t\0')).toBe('"日本語 \\"\\\\\\u{d}\\u{a}\\u{9}\\u{0}"');
});
