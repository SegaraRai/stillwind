import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import { format, lint, type LintResult } from "../dist/node.js";

test("disjoint project groups run in separate workers with configuration project indices", async () => {
  const root = await mkdtemp(
    path.resolve(import.meta.dirname, "../../host/.test-project-workers-"),
  );
  try {
    const log = path.join(root, "setup.log");
    const css =
      "@utility block { display: block; } @utility flex { display: flex; } @stillwind-slot utilities;";
    await writeFile(path.join(root, "a.css"), css);
    await writeFile(path.join(root, "b.css"), css);
    const source = 'import {style} from "stillwind/react";const card=style("flex block missing");';
    const files = ["a/entry.tsx", "b/entry.tsx", "shared/entry.tsx"].map((file) =>
      path.join(root, file),
    );
    for (const file of files) {
      await mkdir(path.dirname(file), { recursive: true });
      await writeFile(file, source);
    }
    // Projects b and c share shared/entry.tsx and b.css, so one worker must answer for both.
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      `import { appendFileSync } from "node:fs";
const hooks = [{ name: "record", setup(ctx) { appendFileSync(${JSON.stringify(log)}, ctx.project + ":" + process.pid + "\\n"); } }];
const lint = { rules: { "unknown-utility": "deny" } };
export default {
  projects: [
    { name: "a", css: "a.css", files: ["a/*.tsx"], typeFacts: false, hooks, lint },
    { name: "b", css: "b.css", files: ["b/*.tsx", "shared/*.tsx"], typeFacts: false, hooks, lint },
    { name: "c", css: "b.css", files: ["shared/*.tsx"], typeFacts: false, hooks, lint },
  ],
};
`,
    );

    const summary = (result: LintResult) =>
      result.results.map(({ project, result }) => [
        project,
        path.relative(root, result.entry).replaceAll("\\", "/"),
        result.reports.map((report) => report.code),
      ]);
    const serial = await lint({ cwd: root, concurrency: 1 });
    const parallel = await lint({ cwd: root, concurrency: 2 });
    expect(parallel.exitCode).toBe(1);
    expect(summary(parallel)).toEqual(summary(serial));
    expect(summary(parallel).map(([project]) => project)).toEqual([0, 0, 1, 1, 1, 2, 2]);

    const formatted = await format({ cwd: root, concurrency: 2 });
    expect(formatted.failures).toEqual([]);
    expect(formatted.changed.map(path.normalize)).toEqual(files);
    for (const file of files) {
      expect(await readFile(file, "utf8")).toContain('style("block flex missing")');
    }
    const lines = (await readFile(log, "utf8"))
      .trim()
      .split("\n")
      .map((line) => line.split(":") as [string, string]);
    expect(lines.map(([project]) => project).sort()).toEqual(["a", "b", "c"]);
    const setups = new Map(lines);
    expect(setups.get("b")).toBe(setups.get("c"));
    expect(setups.get("a")).not.toBe(setups.get("b"));
  } finally {
    await rm(root, { recursive: true, force: true, maxRetries: 5 });
  }
});
