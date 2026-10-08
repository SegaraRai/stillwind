import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "vite-plus/test";

import { AnalysisWorkspace } from "./workspace.ts";

test.each([
  ["object", "settings"],
  ["function", "() => settings"],
  ["async function", "async () => settings"],
  ["promise", "Promise.resolve(settings)"],
])("workspace loads a %s configuration export", async (_kind, expression) => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-"));
  let workspace: AnalysisWorkspace | undefined;
  try {
    const file = path.join(root, "stillwind.config.ts");
    await writeFile(
      file,
      `import { defineConfig } from "stillwind/config";
const settings = { projects: [{ css: "theme.css", files: ["*.tsx"], typeFacts: false }] };
export default defineConfig(${expression});`,
    );
    workspace = await AnalysisWorkspace.open(file);
    expect(workspace.projects.map((project) => project.config)).toEqual([
      {
        css: "theme.css",
        files: ["*.tsx"],
        typeFacts: false,
      },
    ]);
    expect(workspace.projects[0].analysis.host.root).toBe(root);
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("workspace validates the result of an asynchronous JavaScript configuration", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-"));
  try {
    const file = path.join(root, "stillwind.config.js");
    await writeFile(file, "export default async () => ({ projects: [{ files: ['*.tsx'] }] });");
    await expect(AnalysisWorkspace.open(file)).rejects.toThrow("projects[0].css:");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
