import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync, writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { compileReactProject, renderReactStylesheet, summarizeReact } from "@stillwind/native";
import { finalizeReactModuleWithMap, type FinalizedModuleEdit } from "@stillwind/vite";
import { format } from "prettier";
import { TypeScriptFactsService, type AssetRecord } from "../packages/vite/src/type-facts.ts";

type Manifest = {
  schemaVersion: 1;
  id: string;
  title: string;
  description: string;
  tags: string[];
  theme: string;
  sources: string[];
};
type ModuleOutput = {
  path: string;
  code: string | null;
  css: string;
  mappings: Array<{
    original_start: number;
    original_end: number;
    generated_start: number;
    generated_end: number;
  }>;
  diagnostics: Array<{ code: string; message: string }>;
};

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const projectsRoot = path.join(repositoryRoot, "fixtures/projects");
const catalogPath = path.join(repositoryRoot, "apps/fixtures-viewer/public/fixtures.json");
const check = process.argv.includes("--check");

function normalize(value: string) {
  return value.replaceAll("\r\n", "\n");
}

function loadProjects() {
  return readdirSync(projectsRoot, { withFileTypes: true })
    .filter(
      (entry) =>
        entry.isDirectory() && existsSync(path.join(projectsRoot, entry.name, "project.json")),
    )
    .map((entry) => {
      const root = path.join(projectsRoot, entry.name);
      const manifest = JSON.parse(
        readFileSync(path.join(root, "project.json"), "utf8"),
      ) as Manifest;
      assert.equal(manifest.schemaVersion, 1, `${entry.name}: unsupported schema`);
      assert.equal(manifest.id, entry.name, `${entry.name}: id must match directory`);
      assert.equal(
        new Set(manifest.sources).size,
        manifest.sources.length,
        `${entry.name}: duplicate source`,
      );
      return { root, manifest };
    })
    .sort((left, right) => left.manifest.id.localeCompare(right.manifest.id));
}

function compileProject(root: string, manifest: Manifest) {
  const sources = manifest.sources.map((sourcePath) => {
    // Place virtual TS files under packages/vite so module resolution sees the
    // same workspace dependencies as the production checker service. The TS7
    // host virtualizes files but consults the real filesystem for directories.
    const id = path.join(repositoryRoot, "packages/vite/.review-fixtures", manifest.id, sourcePath);
    mkdirSync(path.dirname(id), { recursive: true });
    return {
      id,
      path: sourcePath,
      source: normalize(readFileSync(path.join(root, "input", sourcePath), "utf8")),
    };
  });
  const summaries = new Map(
    sources.map((source) => [
      source.path,
      JSON.parse(summarizeReact(source.source, source.path)) as { assets: AssetRecord[] },
    ]),
  );
  const service = new TypeScriptFactsService(repositoryRoot);
  const assets = new Map(
    [...summaries].map(([sourcePath, summary]) => [sourcePath, summary.assets] as const),
  );
  // The first collection boots the TS7 project. A second snapshot mirrors the
  // Vite integration's steady-state compilation, where imported identities are
  // available after the initial graph has opened.
  service.collect(sources, assets);
  const facts = service.collect(sources, assets);
  service.close();
  const theme = normalize(readFileSync(path.join(root, manifest.theme), "utf8"));
  const output = JSON.parse(
    compileReactProject(
      JSON.stringify(
        sources.map(({ path: sourcePath, source }) => ({
          path: sourcePath,
          source,
          type_facts: facts.get(sourcePath) ?? [],
        })),
      ),
      theme,
    ),
  ) as { modules: ModuleOutput[] };
  const diagnostics = output.modules.flatMap((module) =>
    module.diagnostics.map(
      (diagnostic) => `${module.path}: ${diagnostic.code} ${diagnostic.message}`,
    ),
  );
  assert.deepEqual(
    diagnostics,
    [],
    `${manifest.id}: compilation diagnostics\n${diagnostics.join("\n")}`,
  );
  return {
    modules: output.modules,
    projectCss: renderReactStylesheet(output.modules.map((module) => module.css).join("\n"), theme),
  };
}

function expectedPath(root: string, sourcePath: string) {
  return path.join(root, "expected", sourcePath);
}

function cssPath(root: string, sourcePath: string) {
  return `${expectedPath(root, sourcePath)}.css`;
}

function projectCssPath(root: string) {
  return path.join(root, "expected", "project.css");
}

async function prettyCss(css: string) {
  return css.trim() === "" ? "" : format(css, { parser: "css", printWidth: 100 });
}

function writeOrCheck(file: string, content: string, stale: string[]) {
  const next = `${normalize(content).trimEnd()}\n`;
  if (check) {
    if (!existsSync(file) || normalize(readFileSync(file, "utf8")) !== next)
      stale.push(path.relative(repositoryRoot, file));
    return;
  }
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, next);
}

function finalizedPosition(position: number, prefix: string, removed: FinalizedModuleEdit[]) {
  return (
    position +
    prefix.length -
    removed
      .filter((edit) => edit.to <= position)
      .reduce((length, edit) => length + edit.to - edit.from, 0)
  );
}

function sourceMappedDiff(module: ModuleOutput, prefix: string, removed: FinalizedModuleEdit[]) {
  const changes: Array<{ fromA: number; toA: number; fromB: number; toB: number }> = [];
  const remainingRemoved = [...removed];
  if (remainingRemoved[0]?.from === 0) {
    const header = remainingRemoved.shift()!;
    changes.push({ fromA: 0, toA: header.to, fromB: 0, toB: prefix.length });
  } else if (prefix.length > 0) {
    changes.push({ fromA: 0, toA: 0, fromB: 0, toB: prefix.length });
  }
  for (const edit of remainingRemoved) {
    const generated = finalizedPosition(edit.from, prefix, removed);
    changes.push({ fromA: edit.from, toA: edit.to, fromB: generated, toB: generated });
  }
  for (const mapping of module.mappings) {
    changes.push({
      fromA: mapping.original_start,
      toA: mapping.original_end,
      fromB: finalizedPosition(mapping.generated_start, prefix, removed),
      toB: finalizedPosition(mapping.generated_end, prefix, removed),
    });
  }
  return changes.sort((left, right) => left.fromA - right.fromA || left.fromB - right.fromB);
}

function runnableOutput(module: ModuleOutput, fixture: string) {
  const finalized = finalizeReactModuleWithMap(module.code!, false);
  const output = finalized.code;
  const runtimeImport = output.match(
    /import\s*\{([^}]+)\}\s*from\s*["']stillwind\/react\/runtime["']/,
  );
  const imported = new Set(runtimeImport?.[1].split(",").map((name) => name.trim()) ?? []);
  const calledHelpers = [...output.matchAll(/\b(stillwind[A-Z]\w*)\s*\(/g)].map(
    (match) => match[1],
  );
  assert.deepEqual(
    calledHelpers.filter((name) => !imported.has(name)),
    [],
    `${fixture}: transformed runtime helper is not imported`,
  );
  assert(!/\bdeclare\s+(?:const|let|var)\b/.test(output), `${fixture}: output is not executable`);
  return { output, diff: sourceMappedDiff(module, finalized.prefix, finalized.removed) };
}

const projects = loadProjects();
const stale: string[] = [];
const compiledProjects = new Map<string, ReturnType<typeof compileProject>>();
for (const { root, manifest } of projects) {
  const output = compileProject(root, manifest);
  compiledProjects.set(manifest.id, output);
  for (const module of output.modules) {
    assert(module.code, `${manifest.id}/${module.path}: compiler produced no transformed source`);
    const finalized = runnableOutput(module, `${manifest.id}/${module.path}`);
    writeOrCheck(expectedPath(root, module.path), finalized.output, stale);
    writeOrCheck(cssPath(root, module.path), await prettyCss(module.css), stale);
  }
  writeOrCheck(projectCssPath(root), await prettyCss(output.projectCss), stale);
}

const catalog = {
  schemaVersion: 1,
  projects: projects.map(({ root, manifest }) => ({
    ...manifest,
    css: normalize(readFileSync(projectCssPath(root), "utf8")),
    files: manifest.sources.map((sourcePath) => ({
      path: sourcePath,
      language: sourcePath.endsWith("x") ? "tsx" : "typescript",
      before: normalize(readFileSync(path.join(root, "input", sourcePath), "utf8")),
      after: normalize(readFileSync(expectedPath(root, sourcePath), "utf8")),
      css: normalize(readFileSync(cssPath(root, sourcePath), "utf8")),
      diff: (() => {
        const output = compiledProjects
          .get(manifest.id)!
          .modules.find((module) => module.path === sourcePath);
        assert(output?.code, `${manifest.id}/${sourcePath}: missing compiler output`);
        return runnableOutput(output, `${manifest.id}/${sourcePath}`).diff;
      })(),
    })),
  })),
};
writeOrCheck(catalogPath, JSON.stringify(catalog, null, 2), stale);
if (stale.length > 0) {
  throw new Error(
    `Review fixtures are stale:\n${stale.map((file) => `- ${file}`).join("\n")}\nRun: vp run fixtures:refresh`,
  );
}
console.log(`${check ? "Verified" : "Generated"} ${projects.length} review fixture project(s).`);
