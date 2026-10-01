import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { startNode } from "./process.ts";

const repo = fileURLToPath(new URL("../../../", import.meta.url));
const require = createRequire(import.meta.url);
const vp = join(dirname(require.resolve("vite-plus/package.json")), "bin/vp");
const typecheck = process.argv.includes("--typecheck");
const selected = process.argv.slice(2).filter((arg) => arg !== "--typecheck");
const packages: { name: string; root: string }[] = [{ name: "css", root: join(repo, "e2e/css") }];
for (const group of ["frameworks", "bundlers"]) {
  for (const entry of await readdir(join(repo, "e2e", group), { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.name === "node_modules") {
      continue;
    }
    const root = join(repo, "e2e", group, entry.name);
    try {
      await readFile(join(root, "package.json"));
      packages.push({ name: entry.name, root });
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw error;
      }
    }
  }
}
for (const name of selected) {
  if (!packages.some((item) => item.name === name)) {
    throw new Error(`Unknown fixture: ${name}`);
  }
}

const queue = packages.filter((item) => !selected.length || selected.includes(item.name));
const results: { fixture: string; code: number | null }[] = [];
const resultRoot = join(repo, "e2e/results");
await mkdir(resultRoot, { recursive: true });

const concurrency = Number(process.env.STILLWIND_E2E_CONCURRENCY || 3);
if (!Number.isInteger(concurrency) || concurrency < 1 || concurrency > 8) {
  throw new Error("E2E concurrency must be an integer from 1 to 8");
}

await Promise.all(
  Array.from({ length: concurrency }, async () => {
    for (let fixture = queue.shift(); fixture; fixture = queue.shift()) {
      process.stdout.write(`Starting ${fixture.name}\n`);
      const runner = startNode(
        typecheck
          ? [fileURLToPath(new URL("./typecheck.ts", import.meta.url)), fixture.root]
          : [vp, "test", "--run", "--config", "acceptance.config.ts"],
        fixture.root,
        {},
        1_800_000,
      );
      let code: number | null = null;
      try {
        code = await runner.done;
      } finally {
        await runner.stop();
        await writeFile(
          join(resultRoot, `${fixture.name}${typecheck ? "-typecheck" : ""}.log`),
          runner.logs(),
        );
        results.push({ fixture: fixture.name, code });
        process.stdout.write(`${code === 0 ? "Passed" : "Failed"} ${fixture.name}\n`);
      }
    }
  }),
);

await writeFile(
  join(resultRoot, typecheck ? "typecheck.json" : "matrix.json"),
  JSON.stringify(
    results.sort((a, b) => a.fixture.localeCompare(b.fixture)),
    null,
    2,
  ),
);

if (results.some((result) => result.code !== 0)) {
  process.exitCode = 1;
}
