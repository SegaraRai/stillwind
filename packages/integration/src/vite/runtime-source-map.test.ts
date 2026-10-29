import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import { TraceMap, encodedMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { decodeCssSourceMap } from "@stillwind/host";
import { cssText } from "stillwind/css";
import { expect, test } from "vite-plus/test";

import { renderCssWithSourceMap, replaceCssResources } from "./runtime-source-map.ts";

test("joined parts and constructed sheets retain mappings after adding unique notices", () => {
  const banners = ["/*! 作者\n notice */"];
  const part = (source: string, css: string) => {
    const map = new GenMapping();
    addSegment(map, 0, 0, source, 0, 0);
    return {
      css,
      banners,
      producers: [],
      map: toEncodedMap(map),
    };
  };
  const joined = decodeCssSourceMap(
    renderCssWithSourceMap([part("first.css", ".first{}"), part("second.css", ".second{}")]),
  );
  expect(joined.code).toBe("/*! 作者\n notice */\n.first{}.second{}");
  const trace = new TraceMap(joined.map!);
  expect(originalPositionFor(trace, { line: 3, column: 0 }).source).toBe("first.css");
  expect(originalPositionFor(trace, { line: 3, column: 8 }).source).toBe("second.css");
  // A development fragment embeds its own map, and `cssText` shifts it below the notices.
  const fragment = renderCssWithSourceMap([
    {
      ...part("first.css", ".first{}"),
      banners: [],
    },
  ]);
  const sheet = decodeCssSourceMap(
    cssText([
      {
        css: fragment,
        banners,
      },
    ]),
  );
  expect(sheet.code).toBe("/*! 作者\n notice */\n.first{}");
  expect(originalPositionFor(new TraceMap(sheet.map!), { line: 3, column: 0 }).source).toBe(
    "first.css",
  );
});

test("joining CSS parts shifts their generated positions and keeps their original positions", () => {
  const firstMap = new GenMapping();
  addSegment(firstMap, 0, 0, "first.ts", 0, 0);
  const secondMap = new GenMapping();
  addSegment(secondMap, 0, 0, "second.ts", 1, 0);

  const renderedCss = renderCssWithSourceMap([
    {
      css: ".base{}",
      producers: [],
    },
    {
      css: ".first{}\n",
      producers: ["first.ts"],
      map: encodedMap(new TraceMap(toEncodedMap(firstMap))),
    },
    {
      css: ".second{}",
      producers: ["second.ts"],
      map: encodedMap(new TraceMap(toEncodedMap(secondMap))),
    },
  ]);
  const { code, map } = decodeCssSourceMap(renderedCss);
  const trace = new TraceMap(map!);

  expect(code.trimEnd()).toBe(".base{}.first{}\n.second{}");
  expect(originalPositionFor(trace, { line: 1, column: ".base{}".length })).toMatchObject({
    source: "first.ts",
    line: 1,
    column: 0,
  });
  expect(originalPositionFor(trace, { line: 2, column: 0 })).toMatchObject({
    source: "second.ts",
    line: 2,
    column: 0,
  });
});

test("joining CSS parts does not copy source text into the public map", () => {
  const inputMap = new GenMapping();
  addSegment(inputMap, 0, 0, "first.ts", 0, 0);
  setSourceContent(inputMap, "first.ts", "// 日本語 🧭");

  const renderedCss = renderCssWithSourceMap([
    {
      css: ".first{}",
      producers: ["first.ts"],
      map: encodedMap(new TraceMap(toEncodedMap(inputMap))),
    },
  ]);
  const { map } = decodeCssSourceMap(renderedCss);

  expect(map).not.toHaveProperty("sourcesContent");
});

test("a longer image URL keeps the mapping for CSS after it, including UTF-16 columns", () => {
  const originalCss = '.a{content:"🧭";background:url(IMAGE_PLACEHOLDER);color:red}';
  const originalMap = new GenMapping();
  for (let column = 0; column < originalCss.length; column++) {
    addSegment(originalMap, 0, column, "stillwind-source:/input.css", 0, column);
  }
  setSourceContent(originalMap, "stillwind-source:/input.css", "PRIVATE_SOURCE_SENTINEL");
  const encodedMap = Buffer.from(JSON.stringify(toEncodedMap(originalMap))).toString("base64");
  const inputCss = `${originalCss}\n/*# sourceMappingURL=data:application/json;base64,${encodedMap} */`;
  const imageUrl = "assets/a-much-longer-url.svg";

  const outputCss = replaceCssResources(inputCss, [["IMAGE_PLACEHOLDER", imageUrl]]);
  const { code, map } = decodeCssSourceMap(outputCss);

  expect(code).toContain(imageUrl);
  expect(
    originalPositionFor(new TraceMap(map!), { line: 1, column: code.indexOf("color") }),
  ).toMatchObject({
    source: "stillwind-source:/input.css",
    line: 1,
    column: originalCss.indexOf("color"),
  });
  expect(map).not.toHaveProperty("sourcesContent");
});
