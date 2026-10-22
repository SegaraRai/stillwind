import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { AnalysisProject, createFileSystemAnalysisHost } from "@stillwind/host";
import type { AnalysisProjectConfig } from "stillwind/config";

import { defaultStylesheet } from "../../src/lib/docs/reference/compiler";

export async function projectReports(
  code: string,
  lint: AnalysisProjectConfig["lint"],
  files: Record<string, string> = {},
) {
  const parent = resolve(import.meta.dirname, "../../.astro");
  await mkdir(parent, { recursive: true });
  const root = await mkdtemp(resolve(parent, "reference-test-"));
  if (!root.startsWith(`${parent}/`) && !root.startsWith(`${parent}\\`)) {
    throw new Error("Test workspace escaped Astro cache");
  }
  let project: AnalysisProject | undefined;
  try {
    for (const [name, source] of Object.entries({
      "Reference.tsx": code,
      "Reference.css": defaultStylesheet,
      "tsconfig.json": JSON.stringify({
        compilerOptions: { module: "esnext", moduleResolution: "bundler", jsx: "react-jsx" },
        include: ["*.ts", "*.tsx"],
      }),
      ...files,
    })) {
      await writeFile(resolve(root, name), source);
    }
    project = new AnalysisProject(
      createFileSystemAnalysisHost({ css: "Reference.css", files: ["*.tsx"], lint }, root),
    );
    const result = await project.analyze(resolve(root, "Reference.tsx"));
    if (result.error) {
      throw new Error(result.error);
    }
    return result.reports;
  } finally {
    await project?.close();
    await rm(root, { recursive: true, force: true });
  }
}
