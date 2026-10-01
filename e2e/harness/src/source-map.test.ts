import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { expect, test } from "vite-plus/test";

import { resolveSourceMapFile, verifyMacroSourceMap } from "./source-map.ts";

const project = path.resolve("source-map-fixture");
const mapFile = path.join(project, "dist", "main.css.map");
const expected = path.join(project, "static.js");

test("resolves webpack source URLs against the project rather than the output directory", () => {
  for (const source of [
    "webpack:///./static.js",
    "webpack:///static.js?b431",
    "webpack://fixture/./static.js",
    "webpack://@stillwind/fixture/./static.js",
  ]) {
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

test("limits argument mappings to the requested JS call span and keeps CSS positions exact", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "stillwind-mapping-"));
  const source = 'style("x"); unrelated();';
  const macro = 'style("x")';
  const map = { version: 3, sources: ["../src/app.ts"], names: [], mappings: "CAAO" };
  try {
    await writeFile(path.join(directory, "main.js"), '"sw_abc"');
    await writeFile(path.join(directory, "main.js.map"), JSON.stringify(map));
    await writeFile(path.join(directory, "main.css"), ".sw_abc{}");
    await writeFile(path.join(directory, "main.css.map"), JSON.stringify(map));
    await expect(verifyMacroSourceMap(directory, "src/app.ts", source, macro)).rejects.toThrow(
      "No generated class",
    );
    await expect(
      verifyMacroSourceMap(directory, "src/app.ts", source, macro, false, undefined, "call-span"),
    ).resolves.toMatchObject({ column: 7, position: "call-span" });
    await expect(
      verifyMacroSourceMap(directory, "src/app.ts", source, macro, true),
    ).rejects.toThrow("No generated class");
    await expect(
      verifyMacroSourceMap(directory, "src/app.ts", source, macro, true, undefined, "call-span"),
    ).rejects.toThrow("CSS checks require");
    await writeFile(
      path.join(directory, "main.js.map"),
      JSON.stringify({ ...map, mappings: "CAAU" }),
    );
    await expect(
      verifyMacroSourceMap(directory, "src/app.ts", source, macro, false, undefined, "call-span"),
    ).rejects.toThrow("No generated class");
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
