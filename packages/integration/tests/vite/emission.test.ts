import { expect, test } from "vite-plus/test";

import { partitionStylesheet } from "../../src/vite/emission.ts";

test("partitions retain red-blue-red occurrences and shift UTF-8 source coordinates", () => {
  const part = (css: string, producers: string[]) => ({
    css,
    producers,
    banners: [],
    key: css,
    slot: 0,
    sequence: 0,
    sourceMap: {
      sources: [
        {
          sourceMapSource: "input.css",
          content: css,
        },
      ],
      entries: [
        {
          source: 0,
          generatedStart: 0,
          generatedEnd: Buffer.byteLength(css),
          originalStart: 0,
          originalEnd: Buffer.byteLength(css),
          exact: true,
        },
      ],
    },
  });
  const parts = [
    part(".é{color:red}", ["a"]),
    part(".b{padding:0}", ["a"]),
    part(".x{color:blue}", ["b"]),
    part(".é{color:red}", ["a"]),
  ];
  const output = partitionStylesheet(parts);
  expect(output.length).toBe(3);
  expect(output.map((part) => part.css).join("")).toBe(parts.map((part) => part.css).join(""));
  expect(output[0]!.sourceMap.entries[1]!.generatedStart).toBe(Buffer.byteLength(parts[0]!.css));
  expect(output[0]!.sourceMap.entries[1]!.source).toBe(1);
  expect(parts[0]!.sourceMap.entries.length).toBe(1);
});
