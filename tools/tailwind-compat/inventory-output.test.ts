import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

import { expect, test } from "vite-plus/test";

import { createInventoryOutput } from "./inventory-output.ts";

test("inventory runs preserve independent reports under a shared output parent", () => {
  const root = mkdtempSync(join(tmpdir(), ".test-tailwind-inventory-"));
  try {
    const first = createInventoryOutput(root, "reports");
    const second = createInventoryOutput(root, "reports");
    expect(first).not.toBe(second);
    expect(dirname(first)).toBe(join(root, "reports"));
    expect(dirname(second)).toBe(join(root, "reports"));
    writeFileSync(join(first, "index.json"), "first report");
    writeFileSync(join(second, "index.json"), "second report");
    expect(readFileSync(join(first, "index.json"), "utf8")).toBe("first report");
    expect(readFileSync(join(second, "index.json"), "utf8")).toBe("second report");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
