import { createHash } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { encodedMap, TraceMap } from "@jridgewell/trace-mapping";
import { describe, expect, onTestFinished, test } from "vite-plus/test";

import { resolveSourceMapFile, verifyMacroSourceMap } from "./source-map.ts";

const projectRoot = path.resolve("source-map-fixture");
const outputMapFile = path.join(projectRoot, "dist/main.css.map");
const sourceFile = path.join(projectRoot, "static.js");

describe("source file paths", () => {
  test.each([
    "webpack:///./static.js",
    "webpack:///static.js?b431",
    "webpack://fixture/./static.js",
    "webpack://@stillwind/fixture/./static.js",
  ])("%s resolves from the project root", (sourceUrl) => {
    expect(resolveSourceMapFile(sourceUrl, outputMapFile, projectRoot)).toBe(sourceFile);
  });

  test("a relative source path resolves from the map's directory", () => {
    expect(resolveSourceMapFile("../static.js", outputMapFile, projectRoot)).toBe(sourceFile);
  });

  test("a file URL resolves to its absolute path", () => {
    expect(resolveSourceMapFile(pathToFileURL(sourceFile).href, outputMapFile, projectRoot)).toBe(
      sourceFile,
    );
  });

  test("URL-escaped spaces and hash characters are part of the filename", () => {
    const sourceUrl = "webpack:///./a%20b%23c.js?hash";

    expect(resolveSourceMapFile(sourceUrl, outputMapFile, projectRoot)).toBe(
      path.join(projectRoot, "a b#c.js"),
    );
  });

  test.each([
    { sourceUrl: "webpack:///other/static.js", directory: "other" },
    { sourceUrl: "./static.js", directory: "dist" },
  ])(
    "static.js in $directory does not match static.js at the project root",
    ({ sourceUrl, directory }) => {
      expect(resolveSourceMapFile(sourceUrl, outputMapFile, projectRoot)).toBe(
        path.join(projectRoot, directory, "static.js"),
      );
    },
  );
});

describe("positions within a style call", () => {
  const source = 'style("x"); unrelated();';
  const macro = 'style("x")';
  const argumentColumn = source.indexOf("x");

  test("JavaScript checks require the start of the call by default", async () => {
    const directory = await createOutputDirectory();
    const argumentMap = mapOnePosition({
      sourceName: "../src/app.ts",
      originalColumn: argumentColumn,
    });
    await writeMappedFile(directory, "main.js", '"sw_abc"', argumentMap);

    await expect(verifyMacroSourceMap(directory, "src/app.ts", source, macro)).rejects.toThrow(
      "No generated class",
    );
  });

  test("a JavaScript check can allow a position inside the call's argument", async () => {
    const directory = await createOutputDirectory();
    const argumentMap = mapOnePosition({
      sourceName: "../src/app.ts",
      originalColumn: argumentColumn,
    });
    await writeMappedFile(directory, "main.js", '"sw_abc"', argumentMap);

    await expect(
      verifyMacroSourceMap(directory, "src/app.ts", source, macro, false, undefined, "call-span"),
    ).resolves.toMatchObject({ column: argumentColumn, position: "call-span" });
  });

  test("a JavaScript check rejects a position after the requested call", async () => {
    const directory = await createOutputDirectory();
    const outsideCallMap = mapOnePosition({
      sourceName: "../src/app.ts",
      originalColumn: macro.length,
    });
    await writeMappedFile(directory, "main.js", '"sw_abc"', outsideCallMap);

    await expect(
      verifyMacroSourceMap(directory, "src/app.ts", source, macro, false, undefined, "call-span"),
    ).rejects.toThrow("No generated class");
  });

  test("CSS checks reject a position inside the call's argument", async () => {
    const directory = await createOutputDirectory();
    const argumentMap = mapOnePosition({
      sourceName: "../src/app.ts",
      originalColumn: argumentColumn,
    });
    await writeMappedFile(directory, "main.css", ".sw_abc{}", argumentMap);

    await expect(
      verifyMacroSourceMap(directory, "src/app.ts", source, macro, true),
    ).rejects.toThrow("No generated class");
  });

  test("CSS checks cannot request the JavaScript call-span mode", async () => {
    const directory = await createOutputDirectory();

    await expect(
      verifyMacroSourceMap(directory, "src/app.ts", source, macro, true, undefined, "call-span"),
    ).rejects.toThrow("CSS checks require the start of the macro call");
  });
});

test("the final bundle map takes precedence over a dependency's map directive", async () => {
  const directory = await createOutputDirectory();
  const bundle =
    '// # dependency\n//# sourceMappingURL=missing.js.map\n"sw_abc"\n//# sourceMappingURL=main.js.map\n';
  const bundleMap = mapOnePosition({
    sourceName: "../src/app.ts",
    generatedLine: 3,
  });
  await writeMappedFile(directory, "main.js", bundle, bundleMap);

  await expect(
    verifyMacroSourceMap(directory, "src/app.ts", 'style("x")', 'style("x")'),
  ).resolves.toMatchObject({ line: 1, column: 0 });
});

describe("CSS maps with hashed source names", () => {
  test("the original source URI identifies the generated class", async () => {
    const fixture = await createMappedComponentCss();

    await expect(fixture.verify(fixture.sourceUri)).resolves.toMatchObject({ line: 1, column: 0 });
  });

  test("a different source URI cannot match even when the coordinates agree", async () => {
    const fixture = await createMappedComponentCss();
    const otherSourceUri = pathToFileURL(path.join(fixture.directory, "src/Other.vue")).href;

    await expect(fixture.verify(otherSourceUri)).rejects.toThrow("No generated class");
  });

  test("a map with source text fails the CSS privacy check", async () => {
    const fixture = await createMappedComponentCss();
    const mapWithPrivateText = { ...fixture.map, sourcesContent: ["private source"] };
    await writeFile(fixture.mapFile, JSON.stringify(mapWithPrivateText));

    await expect(fixture.verify(fixture.sourceUri)).rejects.toThrow(/sourcesContent|undefined/);
  });
});

async function createOutputDirectory() {
  const directory = await mkdtemp(path.join(tmpdir(), "stillwind-mapping-"));
  onTestFinished(() => rm(directory, { recursive: true, force: true }));
  return directory;
}

function mapOnePosition({
  sourceName,
  generatedLine = 1,
  originalColumn = 0,
}: {
  sourceName: string;
  generatedLine?: number;
  originalColumn?: number;
}) {
  const lines: number[][][] = Array.from({ length: generatedLine - 1 }, () => []);
  // A segment stores the generated column, source index, original line, and original column.
  lines.push([[1, 0, 0, originalColumn]]);
  return encodedMap(
    new TraceMap({
      version: 3,
      names: [],
      sources: [sourceName],
      mappings: lines,
    }),
  );
}

async function writeMappedFile(
  directory: string,
  filename: string,
  code: string,
  map: ReturnType<typeof mapOnePosition>,
) {
  await writeFile(path.join(directory, filename), code);
  await writeFile(path.join(directory, filename + ".map"), JSON.stringify(map));
}

async function createMappedComponentCss() {
  const directory = await createOutputDirectory();
  const sourceUri = pathToFileURL(path.join(directory, "src/App.vue")).href;
  const sourceName = `stillwind-source:${createHash("sha256").update(sourceUri).digest("hex")}`;
  const map = mapOnePosition({ sourceName });
  await writeMappedFile(directory, "main.css", ".sw_abc{}", map);
  return {
    directory,
    sourceUri,
    map,
    mapFile: path.join(directory, "main.css.map"),
    verify: (uri: string) =>
      verifyMacroSourceMap(
        directory,
        "src/App.vue",
        'style("x")',
        'style("x")',
        true,
        undefined,
        "start",
        uri,
      ),
  };
}
