import path from "node:path";
import { fileURLToPath } from "node:url";

import { resolveConfig } from "vite";
import { expect, test } from "vite-plus/test";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");

for (const project of ["examples/react", "apps/play"]) {
  test(`${project} keeps its project root when its config is loaded elsewhere`, async () => {
    const projectRoot = path.join(repositoryRoot, project);
    const config = await resolveConfig(
      { configFile: path.join(projectRoot, "vite.config.ts") },
      "build",
    );

    expect(path.normalize(config.root)).toBe(path.normalize(projectRoot));
  });
}
