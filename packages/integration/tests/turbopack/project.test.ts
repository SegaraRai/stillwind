import { subscribe, unsubscribe } from "node:diagnostics_channel";
import fs from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
    getResolve:
      (settings: { extensions?: string[] }) =>
      (_directory: string, specifier: string, done: (error: Error | null, id?: string) => void) =>
        settings.extensions?.includes(".css")
          ? done(new Error(`Cannot resolve ${specifier}`))
          : done(null, fileURLToPath(import.meta.resolve(specifier))),
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
    project.release();
  }
});

test("Turbopack invocations share a compiler until the CSS changes and compile only edited modules and their importers", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-shared-project-"));
  onTestFinished(() => rm(root, { recursive: true, force: true }));
  const css = path.join(root, "root.css");
  await writeFile(
    css,
    "@utility paint{color:red} @utility wide{width:3px} @stillwind-slot utilities;",
  );
  await writeFile(
    path.join(root, "entry.tsx"),
    'import {style} from "stillwind/react"; import {tone} from "./tone"; export const props = [tone, style("paint")];',
  );
  const tone = path.join(root, "tone.ts");
  await writeFile(tone, 'export const tone = "first";');
  const dependencies: string[] = [];
  const context = {
    rootContext: root,
    getOptions: () =>
      ({
        compiler: { configFile: false, css: "root.css" },
        root: ".",
        entries: ["entry.tsx"],
        mode: "release",
      }) satisfies TurbopackLoaderOptions,
    fs,
    addDependency: (id: string) => dependencies.push(id),
    addMissingDependency: () => {},
    getResolve:
      () =>
      (directory: string, specifier: string, done: (error: Error | null, id?: string) => void) =>
        done(null, `${path.resolve(directory, specifier)}.ts`),
    emitWarning: () => {},
  } as unknown as LoaderContext<TurbopackLoaderOptions>;
  let compilations = 0;
  const listener = (message: unknown) => {
    if ((message as { phase: string }).phase === "entry") {
      compilations += 1;
    }
  };
  subscribe("stillwind:host:phase", listener);
  onTestFinished(() => {
    unsubscribe("stillwind:host:phase", listener);
  });
  const plan = async () => {
    compilations = 0;
    const project = await createProject(context);
    try {
      return {
        compiler: project.compiler,
        css: (await compileGraph(["entry.tsx"], project)).parts.map((part) => part.css).join("\n"),
        compilations,
      };
    } finally {
      project.release();
    }
  };

  const first = await plan();
  expect(first.compilations).toBe(1);
  dependencies.length = 0;
  const second = await plan();
  expect(second).toMatchObject({ compiler: first.compiler, css: first.css, compilations: 0 });
  expect(dependencies).toContain(css);

  // A per-module compilation of an edit that is reverted before the next reconstruction.
  const entry = path.join(root, "entry.tsx");
  const original = await fs.promises.readFile(entry, "utf8");
  const edited = original.replace('style("paint")', 'style("wide")');
  const project = await createProject(context);
  try {
    await project.compiler.compile(edited, entry, project.host);
  } finally {
    project.release();
  }
  expect((await plan()).css).toBe(first.css);

  await writeFile(tone, 'export const tone = "second";');
  expect(await plan()).toMatchObject({ compiler: first.compiler, compilations: 1 });

  await writeFile(css, "@utility paint{color:blue} @stillwind-slot utilities;");
  const third = await plan();
  expect(third.compiler).not.toBe(first.compiler);
  expect(third.css).toMatch(/color:\s*blue/);
});

test("Turbopack reconstruction compiles only an edited producer that keeps its exports", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-kept-interface-"));
  onTestFinished(() => rm(root, { recursive: true, force: true }));
  await writeFile(
    path.join(root, "root.css"),
    "@utility paint{color:red} @utility wide{width:3px} @stillwind-slot utilities;",
  );
  await writeFile(
    path.join(root, "entry.tsx"),
    'import {style} from "stillwind/react"; import {base} from "./base"; export const props = style(base, "wide");',
  );
  const base = path.join(root, "base.ts");
  await writeFile(
    base,
    'import {style} from "stillwind/react"; export const base = style("paint");',
  );
  const context = {
    rootContext: root,
    getOptions: () =>
      ({
        compiler: { configFile: false, css: "root.css" },
        root: ".",
        entries: ["entry.tsx"],
        mode: "release",
      }) satisfies TurbopackLoaderOptions,
    fs,
    addDependency: () => {},
    addMissingDependency: () => {},
    getResolve:
      () =>
      (directory: string, specifier: string, done: (error: Error | null, id?: string) => void) =>
        done(
          null,
          specifier.startsWith(".")
            ? `${path.resolve(directory, specifier)}.ts`
            : fileURLToPath(import.meta.resolve(specifier)),
        ),
    emitWarning: () => {},
  } as unknown as LoaderContext<TurbopackLoaderOptions>;
  let compilations = 0;
  const listener = (message: unknown) => {
    if ((message as { phase: string }).phase === "entry") {
      compilations += 1;
    }
  };
  subscribe("stillwind:host:phase", listener);
  onTestFinished(() => {
    unsubscribe("stillwind:host:phase", listener);
  });
  const plan = async () => {
    compilations = 0;
    const project = await createProject(context);
    try {
      return (await compileGraph(["entry.tsx"], project)).parts.map((part) => part.css).join("\n");
    } finally {
      project.release();
    }
  };

  const first = await plan();
  await writeFile(
    base,
    'import {style} from "stillwind/react"; const unused = 1; export const base = style("paint");',
  );
  expect(await plan()).toBe(first);
  expect(compilations).toBe(1);
});

test("Turbopack development compilers read the current text of an edited dependency", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-development-edit-"));
  onTestFinished(() => rm(root, { recursive: true, force: true }));
  await writeFile(
    path.join(root, "root.css"),
    "@utility paint{color:red} @utility wide{width:3px} @stillwind-slot utilities;",
  );
  await writeFile(
    path.join(root, "entry.tsx"),
    'import {style} from "stillwind/react"; import {base} from "./base"; export const props = style(base, "wide");',
  );
  const base = path.join(root, "base.ts");
  await writeFile(
    base,
    'import {style} from "stillwind/react"; export const base = style("paint");',
  );
  const context = {
    rootContext: root,
    getOptions: () =>
      ({
        compiler: { configFile: false, css: "root.css" },
        root: ".",
        entries: ["entry.tsx"],
        mode: "development",
      }) satisfies TurbopackLoaderOptions,
    fs,
    addDependency: () => {},
    addMissingDependency: () => {},
    addContextDependency: () => {},
    getResolve:
      () =>
      (directory: string, specifier: string, done: (error: Error | null, id?: string) => void) =>
        done(
          null,
          specifier.startsWith(".")
            ? `${path.resolve(directory, specifier)}.ts`
            : fileURLToPath(import.meta.resolve(specifier)),
        ),
    emitWarning: () => {},
  } as unknown as LoaderContext<TurbopackLoaderOptions>;
  const plan = async () => {
    const project = await createProject(context);
    try {
      return (await compileGraph(["entry.tsx"], project)).parts.map((part) => part.css).join("\n");
    } finally {
      project.release();
    }
  };

  expect(await plan()).toMatch(/color:\s*red/);
  await writeFile(
    base,
    'import {style} from "stillwind/react"; export const other = style("paint");',
  );
  await expect(plan()).rejects.toThrow();
});
