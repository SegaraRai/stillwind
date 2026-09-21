import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { resolveConfig } from "vite-plus";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");

for (const project of ["examples/react", "apps/play"]) {
  void test(`${project} keeps its project root when its config is loaded elsewhere`, async () => {
    const projectRoot = path.join(repositoryRoot, project);
    const config = await resolveConfig(
      { configFile: path.join(projectRoot, "vite.config.ts") },
      "build",
    );

    assert.equal(path.normalize(config.root), path.normalize(projectRoot));
  });
}
