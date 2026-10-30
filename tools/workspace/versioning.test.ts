import { globSync, readFileSync } from "node:fs";
import path from "node:path";

import { expect, test } from "vite-plus/test";

const root = path.resolve(import.meta.dirname, "../..");
const workspace = readFileSync(path.join(root, "pnpm-workspace.yaml"), "utf8");

function quotedNames(text = ""): string[] {
  return [...text.matchAll(/"([^"]+)"/g)].flatMap((match) => match[1] ?? []).sort();
}

function workspaceManifests(): { name: string; private?: boolean }[] {
  const patterns = workspace.match(/^packages:\r?\n((?:  - .*\r?\n)+)/m)?.[1];
  if (!patterns) {
    expect.fail("pnpm-workspace.yaml has no packages list");
  }
  const directories = globSync(
    patterns
      .trim()
      .split(/\r?\n/)
      .map((line) => `${line.replace(/^\s*- /, "")}/package.json`),
    { cwd: root },
  );
  return ["package.json", ...directories].map((file) =>
    JSON.parse(readFileSync(path.join(root, file), "utf8")),
  );
}

test("versioning lists every workspace package by its exact name", () => {
  const manifests = workspaceManifests();
  const [fixed, ignored] =
    workspace.split(/^versioning:\r?\n/m)[1]?.split(/^  ignore:\r?\n/m) ?? [];

  expect(quotedNames(fixed)).toEqual(
    manifests
      .filter((manifest) => !manifest.private)
      .map(({ name }) => name)
      .sort(),
  );
  expect(quotedNames(ignored)).toEqual(
    manifests
      .filter((manifest) => manifest.private)
      .map(({ name }) => name)
      .sort(),
  );
});
