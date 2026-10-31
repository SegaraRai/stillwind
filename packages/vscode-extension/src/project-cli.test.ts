import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, expect, test } from "vite-plus/test";

import { findProjectCli } from "./project-cli.ts";

let root: string;

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), "stillwind-project-cli-"));
});

afterEach(async () => {
  await rm(root, { recursive: true, force: true });
});

async function install(project: string, manifest: string): Promise<void> {
  const directory = join(root, project, "node_modules/@stillwind/cli");
  await mkdir(directory, { recursive: true });
  await writeFile(join(directory, "package.json"), manifest);
}

const manifest = (version: string) =>
  JSON.stringify({
    version,
    bin: {
      stillwind: "dist/main.js",
    },
  });

test("selects the first directory that resolves the CLI", async () => {
  await install("second", manifest("0.2.0"));
  await install("third", manifest("0.3.0"));
  expect(findProjectCli(["first", "second", "third"].map((name) => join(root, name)))).toEqual({
    module: join(root, "second/node_modules/@stillwind/cli/dist/main.js"),
    version: "0.2.0",
  });
});

test("resolves an installation in a parent directory", async () => {
  await install("", manifest("0.1.0"));
  expect(findProjectCli([join(root, "packages/app")])?.version).toBe("0.1.0");
});

test("skips an installation without a usable manifest", async () => {
  await install("broken", "{");
  await install("library", JSON.stringify({ version: "0.1.0" }));
  await install("project", manifest("0.1.0"));
  expect(
    findProjectCli(["broken", "library", "project"].map((name) => join(root, name)))?.version,
  ).toBe("0.1.0");
  expect(findProjectCli([join(root, "broken")])).toBeUndefined();
});

test("returns undefined when no directory resolves the CLI", () => {
  expect(findProjectCli([root])).toBeUndefined();
});
