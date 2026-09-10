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

function semanticDependencies(summary: ModuleSummary, requested: string | null) {
  if (requested === null) return summary.probe_imports;
  if (requested !== "*" && Object.hasOwn(summary.export_imports, requested)) {
    return summary.export_imports[requested];
  }
  const reExports = summary.re_exports.filter(
    (entry) => requested === "*" || entry.exported === requested || entry.exported === "*",
  );
  if (requested !== "*" && reExports.length > 0) {
    return reExports.map((entry) => ({
      source: entry.source,
      imported: entry.exported === "*" ? requested : entry.imported,
    }));
  }
  // Namespace imports and exports without a declaration slice remain conservative.
  return [...summary.imports, ...summary.re_exports];
}

export async function compileModule(
  session: NativeProjectSession,
  factsService: () => TypeScriptFactsService,
  code: string,
  id: string,
  projectRoot: string,
  host: CompilerHost,
  options: { diagnostics?: "throw" | "collect" } = {},
) {
  const entryId = cleanModuleId(id);
  const entrySummary = JSON.parse(
    summarizeReact(code, compilerModulePath(projectRoot, entryId)),
  ) as ModuleSummary;
  const needsTypeFacts = entrySummary.type_probes.length > 0;
  const summaries = new Map<string, ModuleSummary>();
  const sourcesById = new Map<string, ProjectSource>();
  const resolutions = new Map<string, Map<string, string>>();
  const visited = new Map<string, Set<string | null>>();
  const pending: Array<{ source: ProjectSource; requested: string | null }> = [
    {
      source: { id: entryId, path: compilerModulePath(projectRoot, entryId), source: code },
      requested: null,
    },
  ];
  while (pending.length > 0) {
    const { source, requested } = pending.pop()!;
    const demands = visited.get(source.id) ?? new Set<string | null>();
    if (demands.has(requested)) continue;
    demands.add(requested);
    visited.set(source.id, demands);
    sourcesById.set(source.id, source);
    const summary =
      summaries.get(source.path) ??
      (source.id === entryId
        ? entrySummary
        : (JSON.parse(summarizeReact(source.source, source.path)) as ModuleSummary));
    summaries.set(source.path, summary);
    if (!needsTypeFacts) continue;
    for (const dependency of semanticDependencies(summary, requested)) {
      const resolved = await host.resolve(dependency.source, source.id, { skipSelf: true });
      if (!resolved || resolved.external) continue;
      const dependencyId = cleanModuleId(resolved.id);
      // Package runtime exports do not identify their declaration files. Leave
      // external package type resolution (for example React) to TypeScript.
      if (/(?:^|[\\/])node_modules[\\/]/.test(dependencyId)) continue;
      if (!/\.[cm]?[jt]sx?$/.test(dependencyId)) continue;
      let sourceResolutions = resolutions.get(source.id);
      if (!sourceResolutions) resolutions.set(source.id, (sourceResolutions = new Map()));
      sourceResolutions.set(dependency.source, dependencyId);
      host.watch(dependencyId);
      let dependencySource = sourcesById.get(dependencyId);
      if (!dependencySource) {
        dependencySource = {
          id: dependencyId,
          path: compilerModulePath(projectRoot, dependencyId),
          source: await host.read(dependencyId),
        };
        sourcesById.set(dependencyId, dependencySource);
      }
      pending.push({
        source: dependencySource,
        requested: dependency.imported,
      });
    }
  }

  const sources = [...sourcesById.values()];
  const typeFacts = needsTypeFacts
    ? factsService().collect(
        sources.map((source) => {
          const summary = summaries.get(source.path)!;
          return {
            ...source,
            probes: summary.type_probes,
            resolutions: summary.module_specifiers.flatMap((specifier) => {
              const id = resolutions.get(source.id)?.get(specifier.source);
              return id ? [{ start: specifier.start, end: specifier.end, id }] : [];
            }),
          };
        }),
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
    if (output.diagnostics.length > 0 && options.diagnostics !== "collect") {
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
    if (deniedFindings.length > 0 && options.diagnostics !== "collect") {
      fail(deniedFindings.join("\n"));
    }
  }
  const output = projectOutput.modules.find(
    (candidate) => candidate.path === compilerModulePath(projectRoot, entryId),
  );
  if (options.diagnostics === "collect" && output) {
    return {
      ...output,
      diagnostics: projectOutput.modules.flatMap((module) => module.diagnostics),
    };
  }
  if (!output?.code) return undefined;
  return output;
}
