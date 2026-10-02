import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { expect, test } from "vite-plus/test";

import { renderCssWithSourceMap } from "./runtime-source-map.ts";

test("combines mappings across columns and lines and preserves Unicode source content", () => {
  const css = renderCssWithSourceMap([
    {
      css: ".base{}",
      producers: [],
    },
    {
      css: ".first{}\n",
      producers: ["first.ts"],
      map: {
        version: 3,
        names: [],
        sources: ["first.ts"],
        sourcesContent: ["// 日本語 🧭"],
        mappings: "AAAA",
      },
    },
    {
      css: ".second{}",
      producers: ["second.ts"],
      map: { version: 3, names: [], sources: ["second.ts"], mappings: "AACA" },
    },
  ]);
  expect(css).toContain(".base{}.first{}\n.second{}\n/*# sourceMappingURL=");
  const encoded = css.match(/base64,([^ ]+)/)![1]!;
  const map = new TraceMap(Buffer.from(encoded, "base64").toString("utf8"));
  expect(originalPositionFor(map, { line: 1, column: 7 })).toMatchObject({
    source: "first.ts",
    line: 1,
    column: 0,
  });
  expect(originalPositionFor(map, { line: 2, column: 0 })).toMatchObject({
    source: "second.ts",
    line: 2,
    column: 0,
  });
  expect(map.sourcesContent).toContain("// 日本語 🧭");
});
