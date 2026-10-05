import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { expect, test } from "vite-plus/test";

import { browserSourceMap } from "./browser-source-map.ts";

test("publishing a map removes source text, file paths, and symbol names", () => {
  const privateMap = new GenMapping({ file: "private.css" });
  addSegment(privateMap, 0, 2, "server.ts", 3, 7, "privateSymbol");
  setSourceContent(privateMap, "server.ts", "PRIVATE_SOURCE_SENTINEL");
  const inputMap = {
    ...toEncodedMap(privateMap),
    sourceRoot: "file:///private/",
  };

  const publicMap = browserSourceMap(inputMap, () => "stillwind-source:opaque");

  expect(publicMap).not.toHaveProperty("sourcesContent");
  expect(publicMap.sourceRoot).toBeUndefined();
  expect(publicMap.file).toBeUndefined();
  expect(publicMap.names).toEqual([]);
  expect(publicMap.sources).toEqual(["stillwind-source:opaque"]);
});

test("renaming a source keeps its original line and column", () => {
  const inputMap = new GenMapping();
  addSegment(inputMap, 0, 2, "file:///private/server.ts", 3, 7, "privateSymbol");

  const publicMap = browserSourceMap(toEncodedMap(inputMap), () => "stillwind-source:opaque");

  expect(originalPositionFor(new TraceMap(publicMap), { line: 1, column: 2 })).toEqual({
    source: "stillwind-source:opaque",
    line: 4,
    column: 7,
    name: null,
  });
});
