import path from "node:path";
import { pathToFileURL } from "node:url";

import { GenMapping, addSegment, toEncodedMap } from "@jridgewell/gen-mapping";
import { describe, expect, test } from "vite-plus/test";

import { publicCssSourceMap } from "./browser-css.ts";

describe("source names in public CSS maps", () => {
  const projectRoot = path.resolve("map-project");
  const otherDrive = path.parse(projectRoot).root.startsWith("C:") ? "D:" : "C:";

  test("a known CSS file inside the project uses a relative source name", () => {
    const sourceFile = path.join(projectRoot, "root.css");

    const publicMap = publishCssMap(projectRoot, sourceFile);

    expect(publicMap.sources).toEqual(["stillwind-source:/root.css"]);
    expect(publicMap).not.toHaveProperty("sourcesContent");
  });

  test("a known CSS file outside the project uses a hashed source name", () => {
    const sourceFile = path.resolve(projectRoot, "../private.css");

    const publicMap = publishCssMap(projectRoot, sourceFile);

    expect(publicMap.sources).toEqual([expect.stringMatching(/^stillwind-source:[a-f0-9]{64}$/)]);
  });

  test.skipIf(process.platform !== "win32")(
    "a CSS file on another drive uses a hashed source name",
    () => {
      const sourceFile = `${otherDrive}/private.css`;

      const publicMap = publishCssMap(projectRoot, sourceFile);

      expect(publicMap.sources).toEqual([expect.stringMatching(/^stillwind-source:[a-f0-9]{64}$/)]);
    },
  );
});

function publishCssMap(projectRoot: string, sourceFile: string) {
  const inputMap = new GenMapping();
  addSegment(inputMap, 0, 0, pathToFileURL(sourceFile).href, 0, 0);
  return publicCssSourceMap(
    { root: projectRoot, environment: "client", cssFiles: new Set([sourceFile]), inputs: [] },
    toEncodedMap(inputMap),
  );
}
