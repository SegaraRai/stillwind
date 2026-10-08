import path from "node:path";
import { pathToFileURL } from "node:url";

import { GenMapping, addSegment, setSourceContent, toEncodedMap } from "@jridgewell/gen-mapping";
import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { expect, test } from "vite-plus/test";

import { composeCarrierSourceMap } from "./source-map.ts";

for (const kind of ["path", "url"] as const) {
  test(`CSS carrier composition follows a ${kind} back to authored CSS`, () => {
    const carrier = path.resolve("map sources", "carrier #component.css");
    const authored = pathToFileURL(path.resolve("map sources", "authored.css")).href;
    const source = kind === "url" ? pathToFileURL(carrier).href : carrier.replaceAll("\\", "/");
    const generated = new GenMapping();
    setSourceContent(generated, source, "transformed CSS");
    addSegment(generated, 0, 0, source, 0, 0);
    addSegment(generated, 0, 1, "other.css", 2, 3);
    const original = new GenMapping();
    setSourceContent(original, authored, "authored CSS");
    addSegment(original, 0, 0, authored, 4, 2);

    const result = composeCarrierSourceMap(
      JSON.stringify(toEncodedMap(generated)),
      JSON.stringify(toEncodedMap(original)),
      carrier.replaceAll("\\", "/"),
    );
    const traced = new TraceMap(JSON.stringify(result));
    expect(
      originalPositionFor(traced, {
        line: 1,
        column: 0,
      }),
    ).toEqual({
      source: authored,
      line: 5,
      column: 2,
      name: null,
    });
    expect(
      originalPositionFor(traced, {
        line: 1,
        column: 1,
      }),
    ).toEqual({
      source: "other.css",
      line: 3,
      column: 3,
      name: null,
    });
    expect(result.sourcesContent).toContain("authored CSS");
  });
}
