import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { expect, test } from "vite-plus/test";

import { TypeScriptFactsService } from "../semantics/index.ts";
import { AnalysisProject } from "./analysis.ts";
import { createFileSystemAnalysisHost } from "./filesystem.ts";

test("failed CSS loading retains structured diagnostics in the analysis result", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-analysis-"));
  const entry = path.join(root, "entry.tsx");
  const css = path.join(root, "root.css");
  const project = new AnalysisProject(
    createFileSystemAnalysisHost(
      {
        css: "root.css",
        files: ["*.tsx"],
      },
      root,
    ),
  );
  try {
    await writeFile(css, "@theme {");
    await writeFile(entry, "export const value = 1;");
    expect(await project.analyze(entry)).toMatchObject({
      status: "incomplete",
      reports: [
        {
          code: "SW08026",
          primary: {
            location: {
              uri: pathToFileURL(css).href,
            },
          },
        },
      ],
    });
  } finally {
    await project.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("queued analysis discards an obsolete snapshot and reloads CSS from the next overlay", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-analysis-"));
  const entry = path.join(root, "entry.tsx");
  const css = path.join(root, "root.css");
  const host = createFileSystemAnalysisHost(
    {
      css: "root.css",
      files: ["**/*.tsx"],
    },
    root,
  );
  const entered = Promise.withResolvers<void>();
  const released = Promise.withResolvers<void>();
  let first = true;
  const project = new AnalysisProject({
    ...host,
    load: async (id) => {
      if (first) {
        first = false;
        entered.resolve();
        await released.promise;
      }
      const descriptor = host.describe(id);
      return {
        resolverId: id,
        ...descriptor,
        document: {
          ...descriptor.document,
          code: await host.read(id),
        },
      };
    },
  });
  try {
    await writeFile(css, "@utility red{color:red} @stillwind-slot utilities;");
    await writeFile(
      entry,
      'import {style} from "stillwind/react";export const props=style("red");',
    );
    const previous = project.analyze(entry);
    await entered.promise;
    project.update(
      entry,
      'import {style} from "stillwind/react";export const props=style("blue");',
      1,
    );
    project.update(css, "@utility blue{color:blue} @stillwind-slot utilities;", 1);
    const current = project.analyze(entry);
    released.resolve();
    expect(await previous).toMatchObject({ status: "stale", reports: [] });
    const result = await current;
    expect(result.status, result.error).toBe("complete");
    expect(result.reports).toEqual([]);
    expect(result.graph?.modules[0].css).toMatch(/color:\s*blue/);
  } finally {
    released.resolve();
    await project.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("analysis awaits asynchronous semantic queries and closes its host when semantic cleanup fails", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-analysis-"));
  const entry = path.join(root, "entry.tsx");
  const host = createFileSystemAnalysisHost(
    {
      css: "root.css",
      files: ["*.tsx"],
    },
    root,
  );
  let hostClosed = false;
  let service: TypeScriptFactsService | undefined;
  const project = new AnalysisProject(
    {
      ...host,
      close: async () => {
        hostClosed = true;
        await host.close();
      },
    },
    () => {
      const backend = (service = new TypeScriptFactsService(host.typeFactsEnvironment));
      return {
        resolveModules: async (...args) => backend.resolveModules(...args),
        collect: async (...args) => backend.collect(...args),
        collectAvailable: async (...args) => backend.collectAvailable(...args),
        invalidate: async () => backend.invalidate(),
        close: async () => {
          backend.close();
          throw new Error("checker cleanup failed");
        },
      };
    },
  );
  try {
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red} @stillwind-slot utilities;",
    );
    await writeFile(
      entry,
      'import {style} from "stillwind/react";const frames=style.keyframes({from:"red",to:"red"});const alias=frames;export const props=style(alias(""));',
    );
    const result = await project.analyze(entry);
    expect(service).toBeDefined();
    expect(result.status, JSON.stringify(result)).toBe("complete");
    expect(result.graph?.modules[0].css, JSON.stringify(result)).toMatch(/color:\s*red/);
    await expect(project.close()).rejects.toThrow("Failed to close Stillwind analysis services");
    expect(hostClosed).toBe(true);
  } finally {
    await Promise.allSettled([project.close()]);
    await rm(root, { recursive: true, force: true });
  }
});
