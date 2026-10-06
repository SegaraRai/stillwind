import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "vite-plus/test";

const main = path.resolve(import.meta.dirname, "../dist/main.js");

test("CLI checks unused files, keeps overlapping project results, and isolates config logs from JSON", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'import {defineConfig} from "stillwind/config";export default defineConfig(async () => {console.log("configuration log");return {projects:[{css:"root.css",files:["*.tsx"]},{css:"root.css",files:["unused.tsx"]}]};});',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red} @stillwind-slot utilities; @stillwind{unknown-utility:deny;}",
    );
    await writeFile(
      path.join(root, "entry.tsx"),
      'import {style} from "stillwind/react";export const props=style("red");',
    );
    await writeFile(
      path.join(root, "unused.tsx"),
      'import {style} from "stillwind/react";export const props=style("missing");',
    );
    const result = await run(root, ["lint", "--format", "json"]);
    expect(result.code, result.stderr).toBe(1);
    expect(result.stderr).toContain("configuration log");
    const json = JSON.parse(result.stdout) as {
      results: { project: number; result: { entry: string; reports: { code: string }[] } }[];
    };
    expect(json.results.map((entry) => [entry.project, path.basename(entry.result.entry)])).toEqual(
      [
        [0, "entry.tsx"],
        [0, "unused.tsx"],
        [1, "unused.tsx"],
      ],
    );
    expect(json.results[1].result.reports[0].code).toBe("unknown-utility");
    expect((await run(root, ["lint", "entry.tsx"])).code).toBe(0);
    expect((await run(root, ["lint", "absent.tsx"])).code).toBe(2);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("CLI reports unsupported targets and invalid project settings", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../integration/.test-cli-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.txt"]}]};',
    );
    await writeFile(path.join(root, "root.css"), "@stillwind-slot utilities;");
    await writeFile(path.join(root, "source.txt"), "plain text");
    const unsupported = await run(root, ["lint"]);
    expect(unsupported.code).toBe(2);
    expect(unsupported.stdout).toContain("incomplete analysis");
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{files:["*.tsx"]}]};',
    );
    const missingPlugin = await run(root, ["lint"]);
    expect(missingPlugin.code).toBe(2);
    expect(missingPlugin.stderr).toContain("Each Stillwind project must specify CSS");
  } finally {
    await rm(root, { recursive: true, force: true, maxRetries: 5 });
  }
});

async function run(
  cwd: string,
  args: string[],
): Promise<{ code: number | null; stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [main, ...args], { cwd, windowsHide: true });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (data: Buffer) => {
      stdout += data.toString();
    });
    child.stderr.on("data", (data: Buffer) => {
      stderr += data.toString();
    });
    child.on("error", reject);
    child.on("close", (code) => {
      resolve({ code, stdout, stderr });
    });
  });
}
