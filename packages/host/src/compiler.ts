import { relative } from "node:path";

import { summarizeReact } from "@stillwind/native";
import type { NativeProjectSession } from "@stillwind/native";

import type { ModuleSummary, ProjectSource, ProjectCompilerOutput } from "./protocol.ts";
import type { TypeScriptFactsService, TypeFact } from "./type-facts.ts";
export interface CompilerHost {
  resolve(
    specifier: string,
    importer: string,
    options?: { skipSelf: boolean },
  ): Promise<{ id: string; external?: boolean | string } | null>;
  read(id: string): Promise<string>;
  watch(id: string): void;
  warn(message: string): void;
}
function fail(message: string): never {
  throw new Error(message);
}
export function compilerModulePath(root: string, id: string): string {
  return relative(root, id).replaceAll("\\", "/");
}

export function cleanModuleId(id: string): string {
  return id.replace(/[?#].*$/, "");
}

function localDependencies(summary: ModuleSummary): string[] {
  return [
    ...summary.imports.map((entry) => entry.source),
    ...summary.re_exports.map((entry) => entry.source),
  ].filter(
    (specifier, index, all) => specifier.startsWith(".") && all.indexOf(specifier) === index,
  );
}

export async function compileModule(
  session: NativeProjectSession,
  factsService: () => TypeScriptFactsService,
  code: string,
  id: string,
  projectRoot: string,
  host: CompilerHost,
) {
  const entryId = cleanModuleId(id);
  const entrySummary = JSON.parse(
    summarizeReact(code, compilerModulePath(projectRoot, entryId)),
  ) as ModuleSummary;
  const needsTypeFacts = entrySummary.type_probes.length > 0;
  const summaries = new Map<string, ModuleSummary>();
  const sourcesById = new Map<string, ProjectSource>();
  const pending: ProjectSource[] = [
    { id: entryId, path: compilerModulePath(projectRoot, entryId), source: code },
  ];
  while (pending.length > 0) {
    const source = pending.pop()!;
    if (sourcesById.has(source.id)) continue;
    sourcesById.set(source.id, source);
    const summary =
      source.id === entryId
        ? entrySummary
        : (JSON.parse(summarizeReact(source.source, source.path)) as ModuleSummary);
    summaries.set(source.path, summary);
    if (!needsTypeFacts) continue;
    for (const specifier of localDependencies(summary)) {
      const resolved = await host.resolve(specifier, source.id, { skipSelf: true });
      if (!resolved || resolved.external) continue;
      const dependencyId = cleanModuleId(resolved.id);
      if (!/\.[cm]?[jt]sx?$/.test(dependencyId) || sourcesById.has(dependencyId)) continue;
      host.watch(dependencyId);
      pending.push({
        id: dependencyId,
        path: compilerModulePath(projectRoot, dependencyId),
        source: await host.read(dependencyId),
      });
    }
  }

  const sources = [...sourcesById.values()];
  const typeFacts = needsTypeFacts
    ? factsService().collect(
        sources,
        new Map([...summaries].map(([path, summary]) => [path, summary.assets])),
      )
    : new Map<string, TypeFact[]>();
  const typedSources = sources.map((source) => ({
    ...source,
    type_facts: typeFacts.get(source.path) ?? [],
  }));
  const projectOutput = JSON.parse(
    session.compile(JSON.stringify(typedSources), compilerModulePath(projectRoot, entryId)),
  ) as ProjectCompilerOutput;
  const idsByPath = new Map(sources.map((source) => [source.path, source.id]));
  for (const output of projectOutput.modules) {
    const moduleId = idsByPath.get(output.path);
    if (!moduleId) fail(`Stillwind returned an unknown module: ${output.path}`);
    if (output.diagnostics.length > 0) {
      const diagnostic = output.diagnostics[0];
      fail(`${diagnostic.code}: ${diagnostic.message} (${diagnostic.file}:${diagnostic.start})`);
    }
    const deniedFindings: string[] = [];
    for (const finding of output.lint_findings ?? []) {
      const level = output.lint_policy?.[finding.rule] ?? "warn";
      if (level !== "deny") {
        host.warn(`${finding.rule}: ${finding.message} (${finding.file}:${finding.start})`);
      } else {
        deniedFindings.push(
          `${finding.rule}: ${finding.message} (${finding.file}:${finding.start})`,
        );
      }
    }
    if (deniedFindings.length > 0) fail(deniedFindings.join("\n"));
  }
  const output = projectOutput.modules.find(
    (candidate) => candidate.path === compilerModulePath(projectRoot, entryId),
  );
  if (!output?.code) return undefined;
  return output;
}
