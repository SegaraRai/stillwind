import { access } from "node:fs/promises";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";

import { startNode } from "./process.ts";

const root = resolve(process.argv[2] || process.cwd());
const require = createRequire(join(root, "package.json"));

async function run(args: string[]) {
  const process = startNode(args, root);
  try {
    const code = await process.done;
    if (code !== 0) {
      throw new Error(process.logs());
    }
  } finally {
    await process.stop();
  }
}

let config = "tsconfig.json";
try {
  await access(join(root, "tsconfig.e2e.json"));
  config = "tsconfig.e2e.json";
} catch (error) {
  if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
    throw error;
  }
}

const compiler = require.resolve("typescript/bin/tsc");
await run([compiler, "--noEmit", "--project", config]);

for (const language of ["preact", "solid"]) {
  const config = join(root, "src/components", language, "tsconfig.json");
  try {
    await access(config);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      continue;
    }
    throw error;
  }
  await run([compiler, "--noEmit", "--project", config]);
}
