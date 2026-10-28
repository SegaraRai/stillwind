import { expect, test } from "vite-plus/test";

import { decodeState, encodeState, readLocation } from "./permalink.ts";

test("a permalink restores files, preset, version, and the active file", async () => {
  const state = {
    preset: "minimal",
    stillwind: "1.2.3",
    files: {
      "App.tsx": "export const x = `ünïcödé ${1}`;\n",
      "lib/a.ts": "",
    },
    active: "lib/a.ts",
  };
  const data = await encodeState(state);
  expect(data).toMatch(/^[\w-]+$/);
  expect(readLocation(`#code=${data}`)).toEqual({
    kind: "permalink",
    data,
  });
  expect(await decodeState(data)).toEqual(state);
});

test("the location selects a preset or nothing", () => {
  expect(readLocation("#preset=release-checklist")).toEqual({
    kind: "preset",
    id: "release-checklist",
  });
  expect(readLocation("")).toBeUndefined();
  expect(readLocation("#other=1")).toBeUndefined();
});

test("decoding rejects data that is not a workspace", async () => {
  await expect(decodeState("not-base64!")).rejects.toThrow(
    "does not contain a playground workspace",
  );
  await expect(decodeState("broken")).rejects.toThrow("does not contain a playground workspace");
  const compressed = await new Response(
    new Blob([
      JSON.stringify({
        version: 2,
      }),
    ])
      .stream()
      .pipeThrough(new CompressionStream("deflate-raw")),
  ).arrayBuffer();
  const data = btoa(String.fromCharCode(...new Uint8Array(compressed)))
    .replaceAll("+", "-")
    .replaceAll("/", "_");
  await expect(decodeState(data)).rejects.toThrow("does not contain a playground workspace");
});
