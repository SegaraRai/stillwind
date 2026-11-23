import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import type { CliResult } from "../src/result.ts";
import { run } from "./run.ts";

test("info reports resolved projects, matched file counts, plugins, and active lint rules", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-info-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      `import { definePlugin, lintPresets } from "stillwind/config";
export default {
  projects: [
    {
      name: "app",
      css: "root.css",
      files: ["src/*.tsx"],
      exclude: ["src/*.skip.tsx"],
      typeFacts: false,
      plugins: [definePlugin({ name: "demo-plugin", css: () => "" })],
      lint: {
        extends: [lintPresets.recommended],
        rules: {
          "no-arbitrary-colors": "warn",
          "unknown-utility": "off",
        },
      },
    },
    {
      name: "docs",
      css: "root.css",
      files: ["docs/*.tsx"],
      sort: false,
      lint: false,
      typeFacts: false,
    },
  ],
};
`,
    );
    await writeFile(path.join(root, "root.css"), "@stillwind-slot utilities;");
    await writeFile(path.join(root, "src-placeholder.txt"), "");
    await import("node:fs/promises").then(({ mkdir }) => mkdir(path.join(root, "src")));
    await writeFile(path.join(root, "src", "a.tsx"), "export const a = 1;");
    await writeFile(path.join(root, "src", "b.skip.tsx"), "export const b = 2;");

    const json = await run(root, ["info", "--format", "json"]);
    expect(json.code, json.stderr).toBe(0);
    const parsed = JSON.parse(json.stdout) as CliResult;
    expect(parsed.command).toBe("info");
    expect(parsed.summary.projects).toBe(2);
    expect(parsed.projects).toHaveLength(2);
    expect(parsed.projects[0]).toMatchObject({
      project: 0,
      name: "app",
      files: ["src/*.tsx"],
      exclude: ["src/*.skip.tsx"],
      matchedFiles: 1,
      plugins: ["demo-plugin"],
      sort: true,
      typeFacts: false,
    });
    expect(path.normalize(parsed.projects[0].css)).toBe(path.join(root, "root.css"));
    expect(parsed.projects[0].lint["no-arbitrary-colors"]).toBe("warn");
    expect(parsed.projects[0].lint["unknown-utility"]).toBeUndefined();
    expect(parsed.projects[1]).toMatchObject({
      project: 1,
      name: "docs",
      matchedFiles: 0,
      plugins: [],
      sort: false,
      typeFacts: false,
      lint: {},
    });

    const text = await run(root, ["info", "--color", "never"]);
    expect(text.code, text.stderr).toBe(0);
    expect(text.stdout).toContain("project 1 (app)");
    expect(text.stdout).toContain("matched files: 1");
    expect(text.stdout).toContain("plugins: demo-plugin");
    expect(text.stdout).toContain("no-arbitrary-colors: warn");
    expect(text.stdout).toContain("project 2 (docs)");
    expect(text.stdout).toContain("lint: disabled");
    expect(text.stdout).toContain("2 projects.");

    const rejected = await run(root, ["info", "extra"]);
    expect(rejected.code).toBe(2);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
