import path from "node:path";

import { scanSource, type NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { fileModuleDescriptor } from "./filesystem.ts";
import type { CompilerHost } from "./host.ts";
import { CompilationProject } from "./project.ts";

test("a changed file is loaded again while other sources and links stay prepared", async () => {
  const root = path.resolve("/project");
  const entry = path.join(root, "entry.ts");
  const base = path.join(root, "base.ts");
  const files = new Map([
    [entry, 'import { base } from "./base";\nexport const value = base;\n'],
    [base, "export const base = 1;\n"],
  ]);
  const resolved: string[] = [];
  const host: CompilerHost = {
    describe: (id) => fileModuleDescriptor(root, id),
    read: async (id) => files.get(id)!,
    resolve: async (specifier, importer) => {
      resolved.push(`${importer} ${specifier}`);
      return { id: path.resolve(path.dirname(importer), `${specifier}.ts`) };
    },
    watch: () => {},
    warn: () => {},
  };
  const session = { scanSource } as unknown as NativeProjectSession;
  const project = new CompilationProject(true);
  await project.prepare([entry], host, session);
  expect(project.source(base)?.document.code).toBe("export const base = 1;\n");
  const checker = project.install(
    { resolveModules: async () => new Map() } as never,
    [project.source(entry)!].map((source) => ({ ...source, probes: [] })),
  )[0];
  project.retainFacts(checker, []);

  files.set(base, "export const base = 2;\n");
  project.invalidateFile(base, false);
  expect(project.source(base)).toBe(undefined);
  expect(project.source(entry)).toBeDefined();
  expect(project.facts(checker)).toBe(undefined);

  resolved.length = 0;
  await project.prepare([], host, session);
  expect(project.source(base)?.document.code).toBe("export const base = 2;\n");
  // The entry keeps its import link, so preparation does not resolve its imports again.
  expect(resolved).toEqual([]);
});
