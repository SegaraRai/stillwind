import fs from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { stillwindRules } from "@stillwind/integration/turbopack";
import { expect, onTestFinished, test } from "vite-plus/test";
import type { LoaderContext } from "webpack";

import {
  compileGraph,
  createProject,
  type TurbopackLoaderOptions,
} from "../../src/turbopack/project.ts";

test("Turbopack plans document CSS without imports that cannot be resolved and warns about each", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-skipped-import-"));
  onTestFinished(() => rm(root, { recursive: true, force: true }));
  await writeFile(
    path.join(root, "root.css"),
    "@import './missing.css';\n@utility red{color:red} @stillwind-slot utilities;",
  );
  await writeFile(
    path.join(root, "entry.tsx"),
    'import {style} from "stillwind/react"; export const props = style("red");',
  );
  const rule = stillwindRules({
    css: "root.css",
    entries: ["entry.tsx"],
  })["*.{js,jsx,ts,tsx,mjs,cjs}"];
  if (!rule || Array.isArray(rule) || typeof rule !== "object" || !rule.loaders) {
    expect.fail("Expected a loader rule");
  }
  const loader = rule.loaders[0];
  if (typeof loader === "string") {
    expect.fail("Expected loader options");
  }
  const warnings: string[] = [];
  const context = {
    rootContext: root,
    getOptions: () => loader.options as TurbopackLoaderOptions,
    fs,
    addDependency: () => {},
    addMissingDependency: () => {},
    getResolve: () => (_directory: string, specifier: string, done: (error: Error) => void) =>
      done(new Error(`Cannot resolve ${specifier}`)),
    emitWarning: (error: Error) => warnings.push(error.message),
  } as unknown as LoaderContext<TurbopackLoaderOptions>;
  const project = await createProject(context);
  try {
    const plan = await compileGraph(["entry.tsx"], project);
    expect(plan.parts.map((part) => part.css).join("\n")).toMatch(/color:\s*red/);
    expect(warnings).toEqual([
      `stillwind: invalid-css-import: \`./missing.css\` cannot be resolved or read (${path.join(root, "root.css")}:1:1)`,
    ]);
  } finally {
    project.compiler.close();
  }
});
