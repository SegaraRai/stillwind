import { randomUUID } from "node:crypto";
import { cp, mkdir, writeFile, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { afterAll, beforeAll, beforeEach, describe } from "vite-plus/test";

import { fixtureSupport, type FixtureManifest } from "./fixture.ts";
import { contractPolicy } from "./policy.ts";
import { fullCaseName } from "./suite.ts";
import { createWorkspace, fixturePath } from "./workspace.ts";

/**
 * Supplies a disposable application and artifact directory to a scenario's Vitest cases.
 */
export interface ScenarioContext {
  root: string;
  /**
   * Available in hooks and cases after the harness creates the workspace in beforeAll.
   */
  readonly directory: string;
  resultsDirectory: string;
  args: string[];
  logs: string[];
}

/**
 * Registers native Vitest cases against a disposable copy of packageUrl.
 * Call during test collection. register declares cases and hooks using the supplied context.
 * args configures the scenario; directory optionally selects a fixture-relative application subtree.
 * Rejects directory paths outside the fixture when the workspace is created.
 */
export function registerScenario(
  name: string,
  packageUrl: URL,
  register: (context: ScenarioContext) => void,
  args: string[] = [],
  directory?: string,
): void {
  const root = fileURLToPath(packageUrl);

  describe(name, () => {
    let workspace: Awaited<ReturnType<typeof createWorkspace>> | undefined;
    const results = join(root, "results", `${name.replace(/[^a-z0-9-]/gi, "-")}-${randomUUID()}`);
    const context: ScenarioContext = {
      root,
      get directory() {
        if (!workspace) {
          throw new Error("Scenario workspace has not been created");
        }
        return directory ? fixturePath(workspace.directory, directory) : workspace.directory;
      },
      resultsDirectory: results,
      args,
      logs: [],
    };
    let support = "independent";
    const checks: { name: string; status: string }[] = [];

    beforeAll(async () => {
      try {
        await readFile(join(root, "fixture.ts"));
        const manifest: FixtureManifest = (
          await import(pathToFileURL(join(root, "fixture.ts")).href)
        ).default;
        const profile = name.split("/").at(-1)!;
        support = fixtureSupport(manifest, profile);
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
          throw error;
        }
      }
      workspace = await createWorkspace(root);
      await mkdir(results, { recursive: true });
      await mkdir(join(workspace.directory, "output"), { recursive: true });
      await mkdir(join(context.directory, "results"), { recursive: true });
    });

    beforeEach(({ task, onTestFinished }) => {
      onTestFinished(() => {
        checks.push({
          name: fullCaseName(task),
          status:
            task.result?.state === (task.fails ? "fail" : "pass")
              ? task.fails
                ? "expected-failure"
                : "passed"
              : "failed",
        });
      });
    });

    afterAll(async () => {
      try {
        if (workspace) {
          await cp(join(context.directory, "results"), results, {
            recursive: true,
          });
        }
        await writeFile(join(results, "runner.log"), context.logs.join(""));
        await writeFile(join(results, "cases.json"), JSON.stringify(checks, null, 2));
        await writeFile(
          join(results, "policy.json"),
          JSON.stringify({ support, contracts: contractPolicy }, null, 2),
        );
      } finally {
        await workspace?.dispose();
      }
    });

    describe("contracts", () => {
      register(context);
    });
  });
}
