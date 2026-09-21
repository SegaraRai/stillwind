import assert from "node:assert/strict";
import test from "node:test";

import { partitionStylesheet } from "../src/emission.ts";

void test("partitions retain red-blue-red occurrences and shift UTF-8 source coordinates", () => {
  const part = (css: string, producers: string[]) => ({
    css,
    producers,
    key: css,
    slot: 0,
    sequence: 0,
    sourceMap: {
      sources: [{ path: "input.css", content: css }],
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
  assert.equal(output.length, 3);
  assert.equal(output.map((part) => part.css).join(""), parts.map((part) => part.css).join(""));
  assert.equal(output[0]!.sourceMap.entries[1]!.generatedStart, Buffer.byteLength(parts[0]!.css));
  assert.equal(output[0]!.sourceMap.entries[1]!.source, 1);
  assert.equal(parts[0]!.sourceMap.entries.length, 1);
});
