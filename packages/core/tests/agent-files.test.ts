import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vite-plus/test";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const skills = readdirSync(path.join(packageRoot, "skills"));

test("the package publishes the files for coding agents", () => {
  const manifest = JSON.parse(readFileSync(path.join(packageRoot, "package.json"), "utf8")) as {
    files: string[];
  };
  expect(manifest.files).toEqual(expect.arrayContaining(["AGENTS_TEMPLATE.md", "docs", "skills"]));
});

test.each(skills)("the %s skill names its directory", (skill) => {
  const text = readFileSync(path.join(packageRoot, "skills", skill, "SKILL.md"), "utf8");
  expect(/^---\nname: ([\w-]+)\n/.exec(text.replaceAll("\r\n", "\n"))?.[1]).toBe(skill);
});
