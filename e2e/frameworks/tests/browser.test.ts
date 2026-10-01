import { spawn, spawnSync } from "node:child_process";
import { mkdir, readdir, rm } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vite-plus/test";

const root = fileURLToPath(new URL("../", import.meta.url));
const cases: [string, string, string[]][] = [
  ["composition", "verify-composition.mjs", []],
  ["emission", "verify-emission.mjs", []],
  ["runtime-if", "verify-runtime-if.mjs", []],
  ["conditional", "verify-conditional.mjs", []],
  ["animation", "verify-animation.mjs", []],
  ["view-transition", "verify-view-transition.mjs", []],
  ["next-turbopack", "verify-adapters.mjs", ["turbopack"]],
  ["next-standalone", "verify-adapters.mjs", ["turbopack", "standalone"]],
  ["next-webpack", "verify-adapters.mjs", ["webpack"]],
  ["rspack", "verify-rspack.mjs", []],
  ["rsbuild", "verify-rspack.mjs", ["rsbuild"]],
  ["react-router", "verify-react-router.mjs", []],
  ["solidstart", "verify-solidstart.mjs", []],
  ["tanstack-vite", "verify-tanstack.mjs", []],
  ["tanstack-rsbuild", "verify-tanstack.mjs", ["rsbuild"]],
];

for (const [name, script, args] of cases) {
  test(name, async (context) => {
    const directory = new URL("../output/", import.meta.url);
    await mkdir(directory, { recursive: true });
    await mkdir(new URL("../results/", import.meta.url), { recursive: true });
    const existing = new Set(await readdir(directory));
    let activeChild: ReturnType<typeof spawn> | undefined;
    context.onTestFinished(async () => {
      if (activeChild?.pid && activeChild.exitCode === null && activeChild.signalCode === null) {
        if (process.platform === "win32") {
          spawnSync("taskkill", ["/pid", String(activeChild.pid), "/t", "/f"], {
            windowsHide: true,
          });
        } else {
          activeChild.kill("SIGTERM");
        }
      }
      for (const name of await readdir(directory)) {
        if (!existing.has(name)) {
          await rm(join(fileURLToPath(directory), name), { recursive: true, force: true });
        }
      }
    });
    const output = await new Promise<{ code: number | null; log: string }>((resolve, reject) => {
      const child = spawn(
        process.execPath,
        [fileURLToPath(new URL(`../scripts/${script}`, import.meta.url)), ...args],
        {
          cwd: root,
          env: process.env,
          windowsHide: true,
          stdio: ["ignore", "pipe", "pipe"],
        },
      );
      activeChild = child;
      let log = "";
      const append = (data: Buffer) => {
        log = (log + data.toString()).slice(-30_000);
      };
      child.stdout.on("data", append);
      child.stderr.on("data", append);
      child.once("error", reject);
      child.once("close", (code) => resolve({ code, log }));
    });
    expect(output.code, output.log).toBe(0);
  });
}
