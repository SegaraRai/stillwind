import fs from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { withStillwind } from "@stillwind/integration/next";
import { stillwindRules } from "@stillwind/integration/turbopack";
import type { NextConfig } from "next";
import { expect, onTestFinished, test } from "vite-plus/test";
import type { LoaderContext } from "webpack";

import { createProject, type TurbopackLoaderOptions } from "../../src/turbopack/project.ts";

test.each(["next", "turbopack"])(
  "%s loaders evaluate file callbacks without serializing them",
  async (backend) => {
    const root = await mkdtemp(path.join(import.meta.dirname, ".test-config-"));
    onTestFinished(() => rm(root, { recursive: true, force: true }));
    const configFile = path.join(root, "stillwind.config.ts");
    await writeFile(
      configFile,
      `export default async () => ({ projects: [{
    css: 'root.css', files: ['*.tsx'], naming: { utilityClass: () => 'configured-class' }, namingRevision: 'test'
  }] });`,
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red} @stillwind-slot utilities;",
    );
    const inline = { configFile };
    const rules =
      backend === "next"
        ? withStillwind<NextConfig>({}, inline).turbopack!.rules!
        : stillwindRules({ ...inline, entries: ["entry.tsx"] });
    const rule = rules["*.{js,jsx,ts,tsx,mjs,cjs}"];
    if (!rule || Array.isArray(rule) || typeof rule !== "object" || !rule.loaders) {
      expect.fail("Expected a loader rule");
    }
    const loader = rule.loaders[0];
    if (typeof loader === "string") {
      expect.fail("Expected loader options");
    }
    const options = loader.options as TurbopackLoaderOptions;
    expect(JSON.parse(JSON.stringify(options))).toEqual(options);
    const dependencies = new Set<string>();
    const context = {
      getOptions: () => options,
      fs,
      addDependency: (file: string) => dependencies.add(file),
      addMissingDependency: () => {},
      getResolve: () => (_directory: string, _specifier: string, done: (error: Error) => void) =>
        done(new Error("Unexpected CSS import")),
      emitWarning: (error: Error) => expect.fail(error.message),
    } as unknown as LoaderContext<TurbopackLoaderOptions>;
    const project = await createProject(context);
    try {
      expect(project.options.root).toBe(root);
      expect(project.options.compiler.naming?.utilityClass).toBeTypeOf("function");
      expect(dependencies).toContain(configFile);
    } finally {
      project.compiler.close();
    }
  },
);
