import path from "node:path";

import { scanSource, type NativeProjectSession } from "@stillwind/native";
import { expect, test } from "vite-plus/test";

import { fileModuleDescriptor } from "./filesystem.ts";
import type { CompilerHost } from "./host.ts";
import { CompilationProject, ResolutionMismatchError } from "./project.ts";

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

test("a resolver domain shares prepared sources only while it resolves source imports alike", async () => {
  const root = path.resolve("/project");
  const entry = path.join(root, "entry.ts");
  const files = new Map([
    [entry, 'import { base } from "./base";\nimport "react";\nexport const value = base;\n'],
    [path.join(root, "base.ts"), "export const base = 1;\n"],
    [path.join(root, "base.server.ts"), "export const base = 2;\n"],
  ]);
  const host = (domain: string, suffix: string, externalPackages: boolean): CompilerHost => ({
    resolverDomain: domain,
    describe: (id) => fileModuleDescriptor(root, id),
    read: async (id) => files.get(id)!,
    resolve: async (specifier, importer) =>
      specifier === "react"
        ? externalPackages
          ? { id: "react", external: true }
          : { id: path.join(root, "node_modules/react/index.js") }
        : { id: path.resolve(path.dirname(importer), `${specifier}${suffix}.ts`) },
    watch: () => {},
    warn: () => {},
  });
  const session = { scanSource } as unknown as NativeProjectSession;
  const project = new CompilationProject(true);
  await project.prepare([entry], host("client", "", false), session);

  // The checker resolves package types itself, so only the source target must agree.
  await project.prepare([entry], host("ssr", "", true), session);
  await expect(
    project.prepare([entry], host("edge", ".server", true), session),
  ).rejects.toBeInstanceOf(ResolutionMismatchError);
});

test("a resolver domain does not resolve the imports of sources that its targets do not reach", async () => {
  const root = path.resolve("/project");
  const island = path.join(root, "island.ts");
  const middleware = path.join(root, "middleware.ts");
  const files = new Map([
    [island, "export const value = 1;\n"],
    [middleware, 'import { styles } from "virtual:server-only";\nexport const value = styles;\n'],
  ]);
  const host = (domain: string): CompilerHost => ({
    resolverDomain: domain,
    describe: (id) => fileModuleDescriptor(root, id),
    read: async (id) => files.get(id)!,
    resolve: async (specifier) => {
      if (domain === "client") {
        expect.fail(`client resolved ${specifier}`);
      }
      return {
        id: specifier,
        external: true,
      };
    },
    watch: () => {},
    warn: () => {},
  });
  const session = { scanSource } as unknown as NativeProjectSession;
  const project = new CompilationProject(true);
  await project.prepare([middleware], host("ssr"), session);

  await project.prepare([island], host("client"), session);
  expect(project.source(island)).toBeDefined();
});
