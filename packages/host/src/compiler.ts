import { relative } from "node:path";

import { scanSource } from "@stillwind/native";
import type { NativeProjectSession } from "@stillwind/native";
import { checkerEnvironment, projectFrameworkSource } from "@stillwind/projection";

import type { ModuleSummary, ProjectSource } from "./protocol.ts";
import { TypeFactsError } from "./type-facts.ts";
import type { TypeScriptFactsService, TypeFact } from "./type-facts.ts";
/**
 * Host services for resolving and reading semantic dependencies, registering watches,
 * and reporting lint warnings. Resolution may exclude external modules or return null.
 */
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
/**
 * Returns the root-relative module identity with forward slashes. Paths outside
 * root retain `..` segments; this does not validate filesystem containment.
 */
export function compilerModulePath(root: string, id: string): string {
  return relative(root, id).replaceAll("\\", "/");
}

/**
 * Removes the query or fragment suffix from a host module identifier.
 */
export function cleanModuleId(id: string): string {
  return id.replace(/[?#].*$/, "");
}

function semanticDependencies(summary: ModuleSummary, requested: string | null) {
  if (requested === null) return summary.probe_imports;
  const selected = summary.export_imports.find((entry) => entry.name === requested);
  if (requested !== "*" && selected) {
    return selected.imports;
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

/**
 * Compiles an entry buffer and its demanded semantic dependencies into a project session.
 * The entry id is cleaned before projection; native module paths are relative to projectRoot.
 * Type facts are requested lazily for original-coordinate probes in JS/TS or SFC inputs.
 *
 * Returns the entry output with composition metadata, or undefined when normal mode
 * has no generated entry code. Collect mode retains compiler diagnostics across outputs;
 * normal mode throws on compiler diagnostics and deny-level lint findings.
 * Checker contract errors use original coordinates and participate in collect mode.
 * Host I/O, projection, checker process, and native failures propagate in either mode.
 * Session updates, watches, and warnings are not rolled back on failure.
 */
export async function compileModule(
  session: NativeProjectSession,
  factsService: () => TypeScriptFactsService,
  code: string,
  id: string,
  projectRoot: string,
  host: CompilerHost,
  options: {
    diagnostics?: "throw" | "collect";
    stylesheetImports?: string[];
    moduleCssImport?: string;
  } = {},
) {
  const entryId = cleanModuleId(id);
  const entrySource = await projectFrameworkSource({
    id: entryId,
    path: compilerModulePath(projectRoot, entryId),
    source: code,
  });
  const entrySummary = scanSource(entrySource);
  const hasModuleCssImport = entrySummary.module_css_imports > 0;
  if (entrySummary.macro_imports > 0) entrySource.stylesheet_imports = options.stylesheetImports;
  if (hasModuleCssImport) entrySource.module_css_import = options.moduleCssImport;
  const needsTypeFacts = entrySummary.type_probes.length > 0;
  const summaries = new Map<string, ModuleSummary>();
  const sourcesById = new Map<string, ProjectSource>();
  const resolutions = new Map<string, Map<string, string>>();
  const visited = new Map<string, Set<string | null>>();
  const pending: Array<{ source: ProjectSource; requested: string | null }> = [
    {
      source: entrySource,
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
      summaries.get(source.path) ?? (source.id === entryId ? entrySummary : scanSource(source));
    summaries.set(source.path, summary);
    if (!needsTypeFacts) continue;
    for (const dependency of semanticDependencies(summary, requested)) {
      const resolved = await host.resolve(dependency.source, source.id, { skipSelf: true });
      if (!resolved || resolved.external) continue;
      const dependencyId = cleanModuleId(resolved.id);
      // Package runtime exports do not identify their declaration files. Leave
      // external package type resolution (for example React) to TypeScript.
      if (/(?:^|[\\/])node_modules[\\/]/.test(dependencyId)) continue;
      if (!/\.(?:[cm]?[jt]sx?|vue|svelte|astro)$/.test(dependencyId)) continue;
      let sourceResolutions = resolutions.get(source.id);
      if (!sourceResolutions) resolutions.set(source.id, (sourceResolutions = new Map()));
      sourceResolutions.set(dependency.source, dependencyId);
      host.watch(dependencyId);
      let dependencySource = sourcesById.get(dependencyId);
      if (!dependencySource) {
        dependencySource = await projectFrameworkSource({
          id: dependencyId,
          path: compilerModulePath(projectRoot, dependencyId),
          source: await host.read(dependencyId),
        });
        sourcesById.set(dependencyId, dependencySource);
      }
      pending.push({
        source: dependencySource,
        requested: dependency.imported,
      });
    }
  }

  const sources = [...sourcesById.values()];
  let typeFacts: Map<string, TypeFact[]>;
  try {
    typeFacts = needsTypeFacts
      ? factsService().collect(
          sources.map((source) => {
            const summary = summaries.get(source.path)!;
            return {
              ...source,
              ...checkerEnvironment(source),
              checker_spans: summary.checkerSpans,
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
  } catch (error) {
    if (!(error instanceof TypeFactsError) || options.diagnostics !== "collect") throw error;
    const source = sources.find((source) => source.path === error.path)!;
    return {
      path: entrySource.path,
      code: null,
      css: "",
      resources: [],
      mappings: [],
      classes: [],
      symbols: [],
      diagnostics: [error.diagnostic(source.fw_code ?? source.source)],
      rejected_names: [],
      lint_findings: [],
      lint_policy: {},
      composition: [],
      hasModuleCssImport,
      compositionModules: [],
      compositionTopology: "[]",
    };
  }
  const typedSources = sources.map((source) => ({
    ...source,
    module_css_import: source.id === entryId ? source.module_css_import : "stillwind/module-css",
    type_facts: typeFacts.get(source.path) ?? [],
  }));
  const projectOutput = session.compile(typedSources, compilerModulePath(projectRoot, entryId));
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
      if (level === "off") continue;
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
    // A rejected name comes from the host's own generator rather than from the source, so it
    // carries no position: it is reported over the warning channel, and the compilation has
    // already substituted the built-in name.
    for (const rejected of output.rejected_names ?? []) {
      host.warn(
        `stillwind: the ${rejected.kind} name for ${rejected.hash} was rejected: ${rejected.reason}`,
      );
    }
  }
  const output = projectOutput.modules.find(
    (candidate) => candidate.path === compilerModulePath(projectRoot, entryId),
  );
  const composition = output?.composition ?? [];
  const compositionModules = [...new Set(composition.map(({ base }) => base.module_path))].filter(
    (module) => module !== entrySource.path,
  );
  const compositionTopology = JSON.stringify(
    composition
      .map(({ derived, base }) => [
        `${derived.module_path}:${derived.start}`,
        `${base.module_path}:${base.start}`,
      ])
      .sort((a, b) => a[0].localeCompare(b[0]) || a[1].localeCompare(b[1])),
  );
  if (options.diagnostics === "collect" && output) {
    return {
      ...output,
      hasModuleCssImport,
      compositionModules,
      compositionTopology,
      diagnostics: projectOutput.modules.flatMap((module) => module.diagnostics),
    };
  }
  if (!output?.code) return undefined;
  return { ...output, hasModuleCssImport, compositionModules, compositionTopology };
}
