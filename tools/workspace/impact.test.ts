import { execFileSync } from "node:child_process";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import { changedFiles, readWorkspacePackages, workspaceImpact } from "./impact.ts";

test("impact includes transitive consumers, generator inputs, and the closest package", () => {
  const packages = [
    { name: "core", directory: "packages/core", dependencies: [] },
    { name: "plugin", directory: "packages/plugin", dependencies: ["core"] },
    { name: "site", directory: "apps/site", dependencies: ["plugin"] },
    { name: "nested", directory: "packages/core/nested", dependencies: [] },
  ];
  expect(workspaceImpact(packages, ["packages/core/src/index.ts"]).affectedPackages).toEqual([
    "core",
    "plugin",
    "site",
  ]);
  expect(
    workspaceImpact(packages, ["crates/css/src/lib.rs"], { "crates/": ["core"] }).affectedPackages,
  ).toEqual(["core", "plugin", "site"]);
  expect(workspaceImpact(packages, ["packages/core/nested/src/index.ts"]).affectedPackages).toEqual(
    ["nested"],
  );
  expect(workspaceImpact(packages, []).affectedPackages).toEqual([]);
  expect(workspaceImpact(packages, ["Cargo.lock"]).full).toBe(true);
  expect(workspaceImpact(packages, ["removed/package/file.ts"]).full).toBe(true);
  expect(workspaceImpact(packages, null).full).toBe(true);
  expect(workspaceImpact(packages, ["crates/lib.rs"], { "crates/": ["missing"] }).full).toBe(true);
});

test("workspace manifests supply runtime, development, peer, and optional dependencies", async ({
  onTestFinished,
}) => {
  const root = await mkdtemp(path.join(tmpdir(), "stillwind-impact-"));
  onTestFinished(() => rm(root, { recursive: true, force: true }));
  const manifests = [
    { name: "core" },
    { name: "dev" },
    { name: "peer" },
    { name: "optional" },
    {
      name: "consumer",
      dependencies: { core: "workspace:*", external: "1" },
      devDependencies: { dev: "workspace:*" },
      peerDependencies: { peer: "workspace:*" },
      optionalDependencies: { optional: "workspace:*" },
    },
  ];
  const listing = await Promise.all(
    manifests.map(async (manifest) => {
      const directory = path.join(root, manifest.name);
      await mkdir(directory);
      await writeFile(path.join(directory, "package.json"), JSON.stringify(manifest));
      return { name: manifest.name, path: directory };
    }),
  );
  const packages = await readWorkspacePackages(root, listing);
  expect(packages.find((entry) => entry.name === "consumer")?.dependencies).toEqual([
    "core",
    "dev",
    "peer",
    "optional",
  ]);
  await writeFile(
    path.join(root, "core/package.json"),
    JSON.stringify({ name: "core", dependencies: { absent: "workspace:*" } }),
  );
  await expect(readWorkspacePackages(root, listing)).rejects.toThrow(
    "Unknown workspace dependency",
  );
});

test(
  "Git comparison includes every pushed commit and both paths of a rename",
  { timeout: 30_000 },
  async ({ onTestFinished }) => {
    const root = await mkdtemp(path.join(tmpdir(), "stillwind-impact-git-"));
    onTestFinished(() => rm(root, { recursive: true, force: true }));
    const git = (...args: string[]) =>
      execFileSync("git", args, {
        cwd: root,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      }).trim();
    git("init");
    git("config", "user.name", "Stillwind tests");
    git("config", "user.email", "tests@example.invalid");
    git("config", "commit.gpgsign", "false");
    await writeFile(path.join(root, "before.ts"), "export const value = 1;\n");
    git("add", ".");
    git("commit", "-m", "Initial input");
    const before = git("rev-parse", "HEAD");
    await writeFile(path.join(root, "middle.ts"), "export const middle = 2;\n");
    git("add", ".");
    git("commit", "-m", "First pushed commit");
    git("mv", "before.ts", "after.ts");
    git("commit", "-m", "Second pushed commit");
    const after = git("rev-parse", "HEAD");
    expect(changedFiles(root, "push", { before, after })).toEqual([
      "after.ts",
      "before.ts",
      "middle.ts",
    ]);
    expect(changedFiles(root, "pull_request", { pull_request: { base: { sha: before } } })).toEqual(
      ["after.ts", "before.ts", "middle.ts"],
    );
    expect(changedFiles(root, "push", { before: after, after }, before)).toEqual([
      "after.ts",
      "before.ts",
      "middle.ts",
    ]);
    expect(changedFiles(root, "push", { before, after }, "")).toBeNull();
    expect(changedFiles(root, "push", { before, after: before })).toBeNull();
    expect(changedFiles(root, "push", { before: "0".repeat(40), after })).toBeNull();
    expect(changedFiles(root, "workflow_dispatch", {})).toBeNull();
  },
);
