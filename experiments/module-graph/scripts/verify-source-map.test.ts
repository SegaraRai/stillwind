import path from "node:path";
import { pathToFileURL } from "node:url";

import { expect, test } from "vite-plus/test";

import { resolveSourceMapFile } from "./verify-source-map.mjs";

const project = path.resolve("source-map-fixture");
const mapFile = path.join(project, "dist", "main.css.map");
const expected = path.join(project, "static.js");

test("resolves webpack source URLs against the project rather than the output directory", () => {
  for (const source of ["webpack:///./static.js", "webpack:///static.js?b431"]) {
    expect(resolveSourceMapFile(source, mapFile, project)).toBe(expected);
  }
});

test("retains standard relative and file URL source-map resolution", () => {
  expect(resolveSourceMapFile("../static.js", mapFile, project)).toBe(expected);
  expect(resolveSourceMapFile(pathToFileURL(expected).href, mapFile, project)).toBe(expected);
});

test("decodes URL escapes after removing the source identity suffix", () => {
  expect(resolveSourceMapFile("webpack:///./a%20b%23c.js?hash", mapFile, project)).toBe(
    path.join(project, "a b#c.js"),
  );
});

test("does not accept a matching basename from another source directory", () => {
  expect(resolveSourceMapFile("webpack:///other/static.js", mapFile, project)).not.toBe(expected);
  expect(resolveSourceMapFile("./static.js", mapFile, project)).not.toBe(expected);
});
