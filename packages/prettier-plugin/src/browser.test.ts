import { format, formatWithCursor } from "prettier";
import { expect, test } from "vite-plus/test";

import { createStillwindPlugin, type SortInput } from "./browser.ts";

const source =
  'import {style} from "stillwind/react";\n// prettier-ignore\nconst ignored=style("flex block");\nconst value=style("flex block");\n';

/**
 * Swaps every unprotected `flex block` in the input, as a host sorting service would.
 */
async function swap({ source, protectedRanges }: SortInput) {
  const edits = [...source.matchAll(/flex block/g)]
    .filter(({ index }) => !protectedRanges.some(({ start, end }) => index >= start && index < end))
    .map(({ index }) => ({ start: index, end: index + 10, newText: "block flex" }));
  let code = source;
  for (const edit of edits.toReversed()) {
    code = code.slice(0, edit.start) + edit.newText + code.slice(edit.end);
  }
  return { code, edits };
}

test("the browser plugin sorts with the host service and protects ignored nodes", async () => {
  const requests: string[] = [];
  const plugin = createStillwindPlugin((filepath) => {
    requests.push(filepath);
    return swap;
  });
  const output = await format(source, { filepath: "/workspace/entry.tsx", plugins: [plugin] });
  expect(output).toContain('const ignored=style("flex block");');
  expect(output).toContain('const value = style("block flex");');
  expect(requests).toEqual(["/workspace/entry.tsx"]);
});

test("the browser plugin keeps ordinary formatting when the host declines a file", async () => {
  const plugin = createStillwindPlugin(() => undefined);
  const output = await format(source, { filepath: "/workspace/entry.tsx", plugins: [plugin] });
  expect(output).toContain('const value = style("flex block");');
});

test("the browser plugin maps the cursor through sorting edits", async () => {
  const plugin = createStillwindPlugin(() => swap);
  const cursorOffset = source.lastIndexOf("block");
  const result = await formatWithCursor(source, {
    filepath: "/workspace/entry.tsx",
    plugins: [plugin],
    cursorOffset,
  });
  expect(result.formatted.slice(result.cursorOffset, result.cursorOffset + 5)).toBe("block");
});
