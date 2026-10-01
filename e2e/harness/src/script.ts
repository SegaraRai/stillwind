import { randomUUID } from "node:crypto";
import { cp, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vite-plus/test";

import { startNode } from "./process.ts";
import { createWorkspace, fixturePath } from "./workspace.ts";

/** Runs a framework's specialized contracts against a disposable copy of its tracked application. */
export function registerScript(
  name: string,
  packageUrl: URL,
  script: string,
  args: string[] = [],
  directory?: string,
): void {
  test(
    name,
    async () => {
      const root = fileURLToPath(packageUrl);
      const workspace = await createWorkspace(root);
      const application = directory
        ? fixturePath(workspace.directory, directory)
        : workspace.directory;
      const results = join(root, "results", `${name}-${randomUUID()}`);
      await mkdir(results, { recursive: true });
      await mkdir(join(workspace.directory, "output"), { recursive: true });
      await mkdir(join(application, "results"), { recursive: true });
      const runner = startNode(
        [fixturePath(root, script), ...args],
        root,
        { STILLWIND_E2E_WORKDIR: application, STILLWIND_E2E_RESULTS: results },
        1_200_000,
      );
      try {
        expect(await runner.done, runner.logs()).toBe(0);
      } finally {
        await runner.stop();
        await cp(join(application, "results"), results, { recursive: true });
        await writeFile(join(results, "runner.log"), runner.logs());
        await workspace.dispose();
      }
    },
    1_200_000,
  );
}
