import { NativeDesignSystemLoader } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { documentationCompiler } from "../reference/compiler";
import { recordDefaultStylesheet } from "./record";
import { replayStylesheet } from "./stylesheet";

test("replays the recorded default stylesheet into the reference design system", async () => {
  const recorded = await recordDefaultStylesheet();
  const json = JSON.stringify(recorded);
  const sources = [recorded.root, ...Object.values(recorded.imports)];
  for (const name of [...sources.map((source) => source.name), ...Object.keys(recorded.imports)]) {
    expect(name).toMatch(/^(?:utilities\.css|stillwind\/)/);
  }
  expect(sources.every((source) => source.uri === undefined)).toBe(true);

  const loader = new NativeDesignSystemLoader({});
  replayStylesheet(loader, JSON.parse(json));
  const replayed = loader.editorQuery();
  const { query } = await documentationCompiler();
  for (const candidate of ["p-4", "bg-blue-500", "hover:underline", "[padding:16px]"]) {
    expect(replayed.previewCandidate(candidate)).toBe(query.previewCandidate(candidate));
    expect(replayed.canonicalCandidate(candidate)).toBe(query.canonicalCandidate(candidate));
  }
  expect(replayed.completeCandidate("flex")).toEqual(query.completeCandidate("flex"));
});

test("rejects an import that the recording does not contain", () => {
  const loader = new NativeDesignSystemLoader({});
  expect(() =>
    replayStylesheet(loader, {
      root: {
        name: "utilities.css",
        content: '@import "missing.css";',
      },
      imports: {},
    }),
  ).toThrow("missing.css");
});
