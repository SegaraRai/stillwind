import path from "node:path";
import { fileURLToPath } from "node:url";

import { resolveConfig } from "vite";
import { expect, test } from "vite-plus/test";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");

test("apps/play keeps its project root when its config is loaded elsewhere", async () => {
  const projectRoot = path.join(repositoryRoot, "apps/play");
  const config = await resolveConfig(
    {
      configFile: path.join(projectRoot, "vite.config.ts"),
    },
    "build",
  );

  expect(path.normalize(config.root)).toBe(path.normalize(projectRoot));
});
