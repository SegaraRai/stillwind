import { mkdir, mkdtemp, readdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { fixtureSupport, type FixtureManifest } from "./fixture.ts";
import { matrixOptions, selectedProfiles, supportDocument } from "./matrix-selection.ts";
import { contractPolicy } from "./policy.ts";
import { startNode } from "./process.ts";

const repo = fileURLToPath(new URL("../../../", import.meta.url));
const require = createRequire(import.meta.url);
const vp = join(dirname(require.resolve("vite-plus/package.json")), "bin/vp");
const options = matrixOptions(process.argv.slice(2));
const { typecheck, selected } = options;
const packages: { name: string; root: string; manifest?: FixtureManifest }[] = [
  { name: "css", root: join(repo, "e2e/css") },
];
for (const group of ["frameworks", "bundlers"]) {
  for (const entry of await readdir(join(repo, "e2e", group), { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.name === "node_modules") {
      continue;
    }
    const root = join(repo, "e2e", group, entry.name);
    try {
      await readFile(join(root, "package.json"));
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw error;
      }
      continue;
    }
    const manifest: FixtureManifest = (await import(pathToFileURL(join(root, "fixture.ts")).href))
      .default;
    for (const profile of manifest.profiles) {
      fixtureSupport(manifest, profile);
    }
    packages.push({ name: entry.name, root, manifest });
  }
}
for (const name of selected) {
  if (!packages.some((item) => item.name === name)) {
    throw new Error(`Unknown fixture: ${name}`);
  }
}

packages.sort((a, b) => a.name.localeCompare(b.name));
const support = options.support ?? process.env.STILLWIND_E2E_SUPPORT;
if (support !== undefined && support !== "primary" && support !== "compatibility") {
  throw new Error(`Invalid support tier: ${support}`);
}
const queue = packages.filter(
  (item) =>
    (!selected.length || selected.includes(item.name)) &&
    (!item.manifest ||
      selectedProfiles(item.manifest, support, process.env.STILLWIND_E2E_PROFILE).length),
);
if (options.docs) {
  if (selected.length || support) {
    throw new Error("Support documentation requires the full matrix");
  }
  await writeFile(
    join(repo, "e2e/support.md"),
    supportDocument(packages.flatMap((item) => (item.manifest ? [item.manifest] : []))),
  );
  process.stdout.write("Updated e2e/support.md\n");
}
if (options.list || options.docs) {
  if (options.list) {
    process.stdout.write(
      JSON.stringify(
        queue.map((item) => ({
          fixture: item.name,
          profiles: item.manifest
            ? selectedProfiles(item.manifest, support, process.env.STILLWIND_E2E_PROFILE).map(
                (profile) => ({ profile, support: fixtureSupport(item.manifest!, profile) }),
              )
            : [],
        })),
        null,
        2,
      ) + "\n",
    );
  }
  process.exit(0);
}
if (!queue.length) {
  throw new Error("No fixtures match the selection");
}
const results: {
  fixture: string;
  code: number | null;
  profiles: { profile: string; support: string }[];
  contracts: typeof contractPolicy;
}[] = [];
const resultsRoot = join(repo, "e2e/results");
await mkdir(resultsRoot, { recursive: true });
const resultRoot = await mkdtemp(join(resultsRoot, `${typecheck ? "typecheck" : "matrix"}-`));
process.stdout.write(`Results: ${resultRoot}\n`);

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
          ? [vp, "run", "typecheck"]
          : [vp, "test", "--run", "--config", "acceptance.config.ts"],
        fixture.root,
        support ? { STILLWIND_E2E_SUPPORT: support } : {},
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
        results.push({
          fixture: fixture.name,
          code,
          profiles: fixture.manifest
            ? selectedProfiles(fixture.manifest, support, process.env.STILLWIND_E2E_PROFILE).map(
                (profile) => ({ profile, support: fixtureSupport(fixture.manifest!, profile) }),
              )
            : [],
          contracts: contractPolicy,
        });
        process.stdout.write(`${code === 0 ? "Passed" : "Failed"} ${fixture.name}\n`);
      }
    }
  }),
);

await writeFile(
  join(resultRoot, `${typecheck ? "typecheck" : "matrix"}${support ? `-${support}` : ""}.json`),
  JSON.stringify(
    results.sort((a, b) => a.fixture.localeCompare(b.fixture)),
    null,
    2,
  ),
);

if (results.some((result) => result.code !== 0)) {
  process.exitCode = 1;
}
