import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import { documentationPage, stillwindRoot } from "./documentation.ts";

test("report codes link to the installed page and otherwise to the website", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "stillwind-docs-"));
  try {
    const stillwind = path.join(root, "node_modules/stillwind");
    await mkdir(path.join(stillwind, "docs/lints"), { recursive: true });
    await mkdir(path.join(stillwind, "docs/errors"), { recursive: true });
    await writeFile(path.join(stillwind, "package.json"), '{"name":"stillwind"}');
    await writeFile(path.join(stillwind, "docs/lints/unknown-utility.md"), "");
    await writeFile(path.join(stillwind, "docs/errors/SW01001.md"), "");

    const resolved = stillwindRoot(path.join(root, "stillwind.config.ts"));
    expect(resolved).toBeDefined();
    expect(documentationPage("unknown-utility", resolved)).toBe(
      path.join(resolved!, "docs/lints/unknown-utility.md"),
    );
    expect(documentationPage("SW01001", resolved)).toBe(
      path.join(resolved!, "docs/errors/SW01001.md"),
    );
    expect(documentationPage("SW01002", resolved)).toBe(
      "https://stillwind.dev/docs/errors/SW01002",
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
  expect(documentationPage("unknown-utility", undefined)).toBe(
    "https://stillwind.dev/docs/lints/unknown-utility",
  );
  expect(documentationPage("SW_UNVERIFIED", undefined)).toBeUndefined();
  expect(stillwindRoot(null)).toBeUndefined();
});
