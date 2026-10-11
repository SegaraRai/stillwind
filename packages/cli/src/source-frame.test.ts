import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { fingerprint } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { SourceFrames, sourceFrame } from "./source-frame.ts";

function label(source: string, startColumn: number, endColumn: number, endLine = 0) {
  return {
    scope: "range" as const,
    location: {
      uri: "file:///source.tsx",
      revision: fingerprint(source),
      start: 0,
      end: 0,
      startUtf16: 0,
      endUtf16: 0,
      startLine: 0,
      startColumn,
      endLine,
      endColumn,
    },
  };
}

test("frames align UTF-16 ranges with tabs, wide characters, emoji, and combining marks", () => {
  const source = "\t日本🦀e\u0301 missing";
  const start = source.indexOf("missing");
  expect(sourceFrame(source, label(source, start, start + 7))).toBe(
    "> 1 |     日本🦀e\u0301 missing\n    |             ^^^^^^^",
  );
  expect(sourceFrame("日本", label("日本", 0, 2))).toContain("| ^^^^");
  expect(sourceFrame("🦀", label("🦀", 0, 2))).toContain("| ^^");
  expect(sourceFrame("abc", label("abc", 3, 3))).toContain("|    ^");
});

test("multiline ranges have exclusive ends and long spans have bounded excerpts", () => {
  const source = "abc\r\ndef\r\nnext";
  expect(sourceFrame(source, label(source, 1, 0, 2))).toBe(
    "> 1 | abc\n    |  ^^\n> 2 | def\n    | ^^^\n  3 | next",
  );
  const long = "x\n".repeat(20);
  const frame = sourceFrame(long, label(long, 0, 1, 19));
  expect(frame?.split("\n")).toHaveLength(9);
  expect(frame).toMatch(/…$/);
});

test("frames omit unreadable, stale, and document-scoped sources", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "stillwind-frame-"));
  const file = path.join(root, "entry.tsx");
  const source = "missing";
  const original = {
    ...label(source, 0, 7),
    location: { ...label(source, 0, 7).location, uri: pathToFileURL(file).href },
  };
  try {
    expect(new SourceFrames().render(original)).toBeUndefined();
    await writeFile(file, source);
    expect(new SourceFrames().render(original)).toContain("^^^^^^^");
    expect(new SourceFrames().render({ ...original, scope: "document" })).toBeUndefined();
    await writeFile(file, "changed");
    expect(new SourceFrames().render(original)).toBeUndefined();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
