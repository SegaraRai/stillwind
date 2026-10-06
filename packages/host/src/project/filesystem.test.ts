import { mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { prepareSource } from "@stillwind/projection";
import { expect, test } from "vite-plus/test";

import { fileModuleDescriptor } from "../compilation/index.ts";
import { AnalysisProject } from "./analysis.ts";
import { createFileSystemAnalysisHost } from "./filesystem.ts";
import { AnalysisWorkspace } from "./workspace.ts";

const css =
  "@utility red { color: red; } @utility blue { color: blue; } @stillwind-slot utilities;";

test("source services select configured or isolated TypeScript settings", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-analysis-"));
  const hosts: ReturnType<typeof createFileSystemAnalysisHost>[] = [];
  try {
    await writeFile(
      path.join(root, "tsconfig.json"),
      JSON.stringify({
        compilerOptions: {
          paths: {
            "@token": ["./token.ts"],
          },
        },
      }),
    );
    await writeFile(path.join(root, "token.ts"), "export const token = 'red';");
    const configured = createFileSystemAnalysisHost(
      {
        css: "root.css",
        files: ["**/*.tsx"],
      },
      root,
    );
    const isolated = createFileSystemAnalysisHost(
      {
        css: "root.css",
        files: ["**/*.tsx"],
        tsconfig: false,
      },
      root,
    );
    hosts.push(configured, isolated);
    expect(configured.typeFactsEnvironment.project).toEqual({
      kind: "configured",
      configPath: path.join(root, "tsconfig.json"),
    });
    expect(isolated.typeFactsEnvironment.project).toEqual({ kind: "isolated" });
  } finally {
    await Promise.all(hosts.map((host) => host.close()));
    await rm(root, { recursive: true, force: true });
  }
});

test("filesystem analysis follows selected barrel exports, uses explicit tsconfig aliases, and reads unsaved dependencies", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-analysis-"));
  const entry = path.join(root, "consumer.tsx");
  const tokens = path.join(root, "tokens.ts");
  await writeFile(
    path.join(root, "tsconfig.app.json"),
    JSON.stringify({
      compilerOptions: {
        paths: {
          "@tokens": ["./barrel.ts"],
        },
      },
    }),
  );
  const project = new AnalysisProject(
    createFileSystemAnalysisHost(
      {
        css: "root.css",
        files: ["**/*.tsx"],
        types: false,
        tsconfig: "tsconfig.app.json",
      },
      root,
    ),
  );
  try {
    await writeFile(path.join(root, "root.css"), css);
    await writeFile(
      entry,
      'import { style } from "stillwind/react"; import { token } from "@tokens"; export const props = style(token(""));',
    );
    await writeFile(
      path.join(root, "barrel.ts"),
      'export { token } from "./tokens.js"; export { other } from "./never-read";',
    );
    await writeFile(
      tokens,
      'import { style } from "stillwind/react"; export const token = style.keyframes({from:"red",to:"red"});',
    );
    const first = await project.analyze(entry);
    expect(first.status, first.error ?? JSON.stringify(first.graph)).toBe("complete");
    expect(first.reports).toEqual([]);
    expect(first.graph?.dependencies.map((dependency) => dependency.specifier)).toEqual([
      "@tokens",
      "./tokens.js",
    ]);
    expect(first.graph?.modules.find((module) => module.moduleId === "consumer.tsx")?.css).toMatch(
      /color:\s*red/,
    );
    project.update(
      tokens,
      'import { style } from "stillwind/react"; export const token = style.keyframes({from:"blue",to:"blue"});',
      1,
    );
    const second = await project.analyze(entry);
    expect(second.status, second.error).toBe("complete");
    expect(second.graph?.modules.find((module) => module.moduleId === "consumer.tsx")?.css).toMatch(
      /color:\s*blue/,
    );
    expect(project.checkerStats).toMatchObject({
      starts: 1,
      snapshots: expect.any(Number),
      incrementalSnapshots: expect.any(Number),
    });
    project.update(
      tokens,
      'import { style } from "stillwind/react"; export const token = style.keyframes({from:"blue",to:"blue"}); export const unused = style("missing");',
      2,
    );
    const findings = await project.analyze(entry);
    expect(findings.reports).toEqual([
      expect.objectContaining({
        code: "unknown-utility",
        primary: expect.objectContaining({
          location: expect.objectContaining({ uri: expect.stringContaining("tokens.ts") }),
        }),
      }),
    ]);
  } finally {
    await project.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("missing imports recover after file creation and failed syntax retains original diagnostics", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-analysis-"));
  const entry = path.join(root, "consumer.tsx");
  const project = new AnalysisProject(
    createFileSystemAnalysisHost({ css: "root.css", files: ["**/*.tsx"] }, root),
  );
  try {
    await writeFile(path.join(root, "root.css"), css);
    await writeFile(
      entry,
      'import { style } from "stillwind/react"; import { token } from "./tokens"; export const props = style(token(""));',
    );
    const missing = await project.analyze(entry);
    expect(missing.status).toBe("incomplete");
    expect(missing.graph?.dependencies).toContainEqual(
      expect.objectContaining({ specifier: "./tokens", status: "unresolved" }),
    );
    await writeFile(
      path.join(root, "tokens.ts"),
      'import { style } from "stillwind/react"; export const token = style.keyframes({from:"red",to:"red"});',
    );
    project.invalidate();
    const recovered = await project.analyze(entry);
    expect(recovered.status, JSON.stringify(recovered.graph)).toBe("complete");
    project.update(entry, "const broken = (", 1);
    const broken = await project.analyze(entry);
    expect(broken.reports[0]).toMatchObject({
      severity: "error",
      primary: { location: { uri: expect.stringContaining("consumer.tsx") } },
    });
    expect(broken.error).toBeUndefined();
    project.update(entry, "export const ordinary = 1", 2);
    expect((await project.analyze(entry)).reports).toEqual([]);
    expect((await project.analyze(entry, AbortSignal.abort())).status).toBe("cancelled");
  } finally {
    await project.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("unavailable declaration bodies and checker failures keep local diagnostics without source errors", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-analysis-"));
  const entry = path.join(root, "entry.tsx");
  const projects: AnalysisProject[] = [];
  try {
    await writeFile(path.join(root, "root.css"), css);
    await writeFile(
      path.join(root, "globals.d.ts"),
      'declare module "virtual:assets" { import {style} from "stillwind/react"; export const props: ReturnType<typeof style>; export const ref: ReturnType<typeof style.ref>; export const frames: ReturnType<typeof style.keyframes>; }',
    );
    await writeFile(
      path.join(root, "tsconfig.json"),
      JSON.stringify({
        compilerOptions: { moduleResolution: "bundler", module: "esnext" },
        files: ["globals.d.ts"],
      }),
    );
    await writeFile(
      entry,
      'import {style} from "stillwind/react";import {props,ref,frames} from "virtual:assets";export const local=style("missing");export const a=style(props);export const b=style(ref.asContainer());export const c=style(frames(""));',
    );
    const normal = new AnalysisProject(
      createFileSystemAnalysisHost({ css: "root.css", files: ["*.tsx"] }, root),
    );
    const unavailable = new AnalysisProject(
      createFileSystemAnalysisHost({ css: "root.css", files: ["*.tsx"] }, root),
      () => {
        throw new Error("checker transport unavailable");
      },
    );
    const disabled = new AnalysisProject(
      createFileSystemAnalysisHost({ css: "root.css", files: ["*.tsx"], typeFacts: false }, root),
    );
    projects.push(normal, unavailable, disabled);
    for (const project of projects) {
      const result = await project.analyze(entry);
      expect(result.status, JSON.stringify(result)).toBe("incomplete");
      expect(result.error).toBeUndefined();
      expect(result.reports.map((report) => report.code)).toEqual(["unknown-utility"]);
      expect(result.graph?.unverified.length).toBeGreaterThan(0);
      expect(result.graph?.generation).toBe("blocked");
      expect(result.graph?.modules[0].code).toBeNull();
      expect(result.graph?.modules[0].css).toBe("");
      if (project === normal) {
        expect(result.graph?.dependencies).toContainEqual(
          expect.objectContaining({ specifier: "virtual:assets", status: "types" }),
        );
      }
    }
    disabled.update(
      entry,
      'import {style} from "stillwind/react";import {ref} from "virtual:assets";ref.asContainer();export const local=style("red missing");',
      1,
    );
    const recognition = await disabled.analyze(entry);
    expect(recognition.status).toBe("incomplete");
    expect(recognition.graph?.unverified.length).toBeGreaterThan(0);
    expect(recognition.reports.map((report) => report.code)).toEqual(["unknown-utility"]);
    expect(recognition.graph?.generation).toBe("complete");
    expect(recognition.graph?.modules[0].code).not.toBeNull();
    expect(recognition.graph?.modules[0].css).toMatch(/color:\s*red/);
  } finally {
    await Promise.all(projects.map((project) => project.close()));
    await rm(root, { recursive: true, force: true });
  }
});

test("diagnostic scopes exclude dependencies and globals from target enumeration while checker queries retain them", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-workspace-"));
  let workspace: AnalysisWorkspace | undefined;
  try {
    await writeFile(path.join(root, "root.css"), css);
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.tsx"],exclude:["ignored.tsx"],tsconfig:"tsconfig.app.json"}]};',
    );
    await writeFile(
      path.join(root, "tsconfig.app.json"),
      JSON.stringify({
        compilerOptions: {
          module: "esnext",
          moduleResolution: "bundler",
          paths: { "@frames": ["./dependency.ts"] },
        },
        files: ["ambient.d.ts"],
      }),
    );
    await writeFile(path.join(root, "ambient.d.ts"), "declare const extra: string;");
    await writeFile(
      path.join(root, "entry.tsx"),
      'import {style} from "stillwind/react";import {frames} from "@frames";export const props=style(frames(""));',
    );
    await writeFile(path.join(root, "ignored.tsx"), "const broken=(");
    await writeFile(
      path.join(root, "dependency.ts"),
      'import {style} from "stillwind/react";export const frames=style.keyframes({from:"red",to:"red"});export const other=style("missing");',
    );
    workspace = await AnalysisWorkspace.open(path.join(root, "stillwind.config.ts"));
    expect(
      (await workspace.targets(workspace.projects[0])).map((file) => path.basename(file)),
    ).toEqual(["entry.tsx"]);
    const [result] = await workspace.lint();
    expect(result.result.status, JSON.stringify(result)).toBe("complete");
    expect(result.result.graph?.modules.map((module) => module.moduleId)).toContain(
      "dependency.ts",
    );
    expect(result.result.reports).toEqual([]);
    expect(await workspace.analyze(path.join(root, "ignored.tsx"))).toEqual([]);
  } finally {
    await workspace?.close();
    await rm(root, { recursive: true, force: true });
  }
});

test("source and projection providers supply virtual implementations and captured documents", async () => {
  const root = await mkdtemp(path.join(import.meta.dirname, ".test-provider-"));
  const entry = path.join(root, "entry.tsx");
  const dependency = path.join(root, "virtual.ts");
  const project = new AnalysisProject(
    createFileSystemAnalysisHost(
      {
        css: "root.css",
        files: ["*.tsx"],
        sources: {
          kind: "custom",
          supports: () => true,
          resolve: async (specifier) => (specifier === "virtual:frames" ? dependency : null),
          load: async (id, context) => {
            const descriptor = fileModuleDescriptor(root, id);
            return {
              resolverId: id,
              ...descriptor,
              document: {
                ...descriptor.document,
                code:
                  context.documents.get(id) ??
                  (id === entry
                    ? 'import {style} from "stillwind/react";import {frames} from "virtual:frames";export const props=style(frames(""));'
                    : 'import {style} from "stillwind/react";export const frames=style.keyframes({from:"red",to:"red"});'),
              },
            };
          },
        },
        projection: { kind: "custom", prepare: prepareSource },
      },
      root,
    ),
  );
  try {
    await writeFile(path.join(root, "root.css"), css);
    const first = await project.analyze(entry);
    expect(first.status, JSON.stringify(first)).toBe("complete");
    expect(first.graph?.modules.find((module) => module.moduleId === "entry.tsx")?.css).toMatch(
      /color:\s*red/,
    );
    project.update(
      dependency,
      'import {style} from "stillwind/react";export const frames=style.keyframes({from:"blue",to:"blue"});',
      1,
    );
    const second = await project.analyze(entry);
    expect(second.status, JSON.stringify(second)).toBe("complete");
    expect(second.graph?.modules.find((module) => module.moduleId === "entry.tsx")?.css).toMatch(
      /color:\s*blue/,
    );
  } finally {
    await project.close();
    await rm(root, { recursive: true, force: true });
  }
});
