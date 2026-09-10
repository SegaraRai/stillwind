import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

import { resolveSourceMapFile } from "./verify-source-map.mjs";

const project = path.resolve("source-map-fixture");
const mapFile = path.join(project, "dist", "main.css.map");
const expected = path.join(project, "static.js");

void test("resolves webpack source URLs against the project rather than the output directory", () => {
  for (const source of ["webpack:///./static.js", "webpack:///static.js?b431"]) {
    assert.equal(resolveSourceMapFile(source, mapFile, project), expected);
  }
});

void test("retains standard relative and file URL source-map resolution", () => {
  assert.equal(resolveSourceMapFile("../static.js", mapFile, project), expected);
  assert.equal(resolveSourceMapFile(pathToFileURL(expected).href, mapFile, project), expected);
});

void test("decodes URL escapes after removing the source identity suffix", () => {
  assert.equal(
    resolveSourceMapFile("webpack:///./a%20b%23c.js?hash", mapFile, project),
    path.join(project, "a b#c.js"),
  );
});

void test("does not accept a matching basename from another source directory", () => {
  assert.notEqual(resolveSourceMapFile("webpack:///other/static.js", mapFile, project), expected);
  assert.notEqual(resolveSourceMapFile("./static.js", mapFile, project), expected);
});
