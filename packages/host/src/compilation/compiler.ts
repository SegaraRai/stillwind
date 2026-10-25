import type {
  BindingDiagnostic,
  BindingRejectedIdentifier,
  BindingReport,
  NativeDiagnosticBatch,
  NativeModuleSummary,
  NativePreparedSource,
  NativeProjectSession,
  ProjectModuleOutput,
} from "@stillwind/native";
import type { PreparedSource, ProjectInput } from "@stillwind/projection";

import { compileWithPlugins } from "../css/plugins/session.ts";
import type { TypeFactsBackend, TypeFactsInputs, UnverifiedRange } from "../semantics/index.ts";
import { loadSource, type CompilerHost } from "./host.ts";
import { CompilationFailure, ModuleGraph } from "./module-graph.ts";
import type { CompilationProject } from "./project.ts";
import { createModuleAnalysis, type ModuleAnalysis, type ModuleCompileOptions } from "./request.ts";

/** Most reloads with new plugin definitions during one compilation of a graph. */
const PLUGIN_ROUNDS = 3;

/**
 * Analyzes an entry and its semantic dependencies without requiring stylesheet delivery IDs.
 * Source failures return diagnostic batches. Unavailable semantic information records unverified
 * ranges. Entry I/O and native transport failures reject.
 * The caller owns the initialized session and the lazy checker service.
 *
 * @param session - Native project session with an initialized design system.
 * @param factsService - Supplies a checker configured for the host's TypeScript environment,
 * or undefined when semantic facts are unavailable.
 * @param id - Full resolver identity of the entry, including query suffixes.
 * @param host - Source loading, resolution, metadata, and dependency tracking operations.
 * @param typeFactsInputs - Captured overlays and ambient declarations for this analysis.
 * @param project - Optional sources prepared by earlier analyses of this project.
 * The caller must invalidate changed sources before analysis starts.
 * @returns Reports and sources for the semantic dependencies demanded by the entry.
 * @throws If entry I/O fails, a host changes resolver identity, or native transport operations fail.
 */
export async function analyzeModule(
  session: NativeProjectSession,
  factsService: () => TypeFactsBackend | undefined,
  id: string,
  host: CompilerHost,
  typeFactsInputs?: TypeFactsInputs,
  project?: CompilationProject,
): Promise<ModuleAnalysis> {
  const revision = project?.revision;
  const captured = project?.source(id);
  const input = captured ?? (await loadSource(host, id));
  if (project?.revision !== revision) {
    project = undefined;
  }
  const analysis: ModuleAnalysis = createModuleAnalysis(input.moduleId);
  if (!captured && input.resolverId !== id) {
    throw new Error(`Host changed resolver identity for ${id}`);
  }
  await compileGraph(
    session,
    factsService,
    input.document.code,
    input.resolverId,
    host,
    {
      diagnostics: "collect",
      moduleCssImport: "stillwind/module-css",
      typeFactsInputs,
    },
    analysis,
    input,
    project,
  );
  for (const batch of analysis.batches) {
    for (const report of batch.reports) {
      const edits = report.actions[0]?.edits;
      if (edits?.length && report.code !== "prefer-sorted-utilities") {
        analysis.fixes.push(edits);
      }
    }
  }
  return analysis;
}

/**
 * Compiles an entry buffer and its demanded semantic dependencies into a project session.
 * The host descriptor supplies the module identity and parser metadata for each source.
 * Type facts are requested lazily for original-coordinate probes in JS/TS or SFC inputs.
 *
 * Collect mode retains compiler and checker reports, including known syntax and projection
 * failures. The default throw mode rejects compiler diagnostics. Lint findings only produce warnings.
 * Host I/O, checker process, and malformed native transport failures propagate in both modes.
 * Failed compilation can still update the session, register watches, and send warnings.
 *
 * @param session - Native project session with an initialized design system.
 * @param factsService - Supplies the type-facts service when the source graph needs semantic facts.
 * @param code - Entry source before Stillwind transformation.
 * @param id - Full resolver identity of the entry source, including its query suffix.
 * @param host - Source metadata, resolution, reading, dependency tracking, and warning operations.
 * @param options - Diagnostic behavior, captured checker inputs, and generated imports for the entry.
 * @returns A promise for the entry output and composition metadata, or undefined when normal mode
 * produces no entry code.
 * @throws If compilation rejects diagnostics, source preparation fails, or host or checker operations fail.
 */
export async function compileModule(
  session: NativeProjectSession,
  factsService: () => TypeFactsBackend | undefined,
  code: string,
  id: string,
  host: CompilerHost,
  options: ModuleCompileOptions = {},
) {
  return compileGraph(session, factsService, code, id, host, options);
}

/**
 * Prepares the demanded graph, compiles it into the session, and derives the entry output.
 *
 * @param session - Initialized native session that retains successful module artifacts.
 * @param factsService - Lazy checker backend for semantic probes.
 * @param code - Original entry text used when entryInput is absent.
 * @param id - Full host identity of the entry.
 * @param host - Stable source metadata, resolution, and notification operations.
 * @param options - Diagnostic policy, captured checker inputs, and entry transport imports.
 * @param analysis - Optional object populated with sources, dependencies, reports, and generation status.
 * @param entryInput - Optional loaded entry, which takes precedence over code and host.describe.
 * @param project - Optional prepared sources shared with other analyses of this project.
 * @returns Entry output, a diagnostic-only output after a collected failure, or undefined when no code is emitted.
 * @throws If strict compilation fails or a host or transport failure cannot be collected.
 */
async function compileGraph(
  session: NativeProjectSession,
  factsService: () => TypeFactsBackend | undefined,
  code: string,
  id: string,
  host: CompilerHost,
  options: ModuleCompileOptions,
  analysis?: ModuleAnalysis,
  entryInput?: ProjectInput | PreparedSource,
  project?: CompilationProject,
) {
  const graph = new ModuleGraph(session, factsService, host, options, analysis, project);
  let hasModuleCssImport = false;
  try {
    const descriptor = entryInput ?? {
      resolverId: id,
      ...host.describe(id),
      document: {
        ...host.describe(id).document,
        code,
      },
    };
    const entrySource = await graph.prepare(descriptor);
    const entrySummary = graph.scan(entrySource);
    hasModuleCssImport = entrySummary.moduleCssImports > 0;
    if (entrySummary.macroImports > 0) {
      entrySource.stylesheetImports = options.stylesheetImports;
    }
    if (hasModuleCssImport) {
      entrySource.moduleCssImport = options.moduleCssImport;
    }
    await graph.collectDependencies(entrySource, entrySummary);
    const typedSources = await graph.collectTypedSources(entrySource);
    const producer = analysis ? undefined : entrySource.moduleId;
    const projectOutput = await compileWithPlugins(
      session,
      () => session.compile(typedSources, producer),
      PLUGIN_ROUNDS,
    );
    if (analysis) {
      recordAnalysis(analysis, projectOutput.modules, session, typedSources, graph);
    }
    reportOutputDiagnostics(projectOutput.modules, graph, host, options);
    return entryOutput(projectOutput.modules, entrySource.moduleId, hasModuleCssImport, options);
  } catch (error) {
    const failure = graph.failure(error);
    if (failure instanceof CompilationFailure && options.diagnostics === "collect") {
      if (analysis) {
        analysis.batches = [...failure.batches];
        analysis.generation = "blocked";
      }
      return failedOutput(
        entryInput?.moduleId ?? host.describe(id).moduleId,
        failure.batches,
        hasModuleCssImport,
      );
    }
    throw failure;
  }
}

/**
 * Selects the entry and computes its composition dependency identities and topology fingerprint.
 *
 * @param modules - Native outputs in compiler order.
 * @param entryModuleId - Compiler identity of the requested entry.
 * @param hasModuleCssImport - Whether the entry requested an explicit CSS value.
 * @param options - Diagnostic policy controlling whether reports from all modules are attached.
 * @returns Enriched entry output. Throw mode omits entries without nonempty code, and collect mode retains them.
 */
function entryOutput(
  modules: ProjectModuleOutput[],
  entryModuleId: string,
  hasModuleCssImport: boolean,
  options: ModuleCompileOptions,
) {
  const output = modules.find((candidate) => candidate.moduleId === entryModuleId);
  const composition = output?.composition ?? [];
  const compositionModules = [...new Set(composition.map(({ base }) => base.moduleId))].filter(
    (module) => module !== entryModuleId,
  );
  const compositionTopology = JSON.stringify(
    composition
      .map(({ derived, base }) => [
        `${derived.moduleId}:${derived.start}`,
        `${base.moduleId}:${base.start}`,
      ])
      .sort((a, b) => a[0].localeCompare(b[0]) || a[1].localeCompare(b[1])),
  );
  if (options.diagnostics === "collect" && output) {
    return {
      ...output,
      hasModuleCssImport,
      compositionModules,
      compositionTopology,
      diagnostics: modules.flatMap((module) => module.diagnostics),
      reports: modules.flatMap((module) => module.reports),
    };
  }
  if (!output?.code) {
    return undefined;
  }
  return {
    ...output,
    hasModuleCssImport,
    compositionModules,
    compositionTopology,
  };
}

/**
 * Populates analysis batches and removes artifacts when missing semantic facts block lowering.
 *
 * @param analysis - Mutable result whose unverified ranges are deduplicated and generation status updated.
 * @param modules - Mutable native outputs whose reports are filtered and artifacts may be cleared.
 * @param session - Session used to diagnose sources independently when facts are unavailable.
 * @param typedSources - Prepared sources from the same compilation, with available type facts attached.
 * @param graph - Graph retaining scanner summaries for the compiled modules.
 */
function recordAnalysis(
  analysis: ModuleAnalysis,
  modules: ProjectModuleOutput[],
  session: NativeProjectSession,
  typedSources: NativePreparedSource[],
  graph: ModuleGraph,
): void {
  analysis.modules = modules;
  analysis.unverified = [
    ...new Map(analysis.unverified.map((range) => [JSON.stringify(range), range])).values(),
  ];
  const localReports = analysis.unverified.length ? session.diagnose(typedSources) : [];
  for (const module of modules) {
    module.reports =
      localReports.find((batch) => batch.module === module.moduleId)?.reports ?? module.reports;
    const ranges = analysis.unverified.filter((range) => range.moduleId === module.moduleId);
    const generationUnverified = generationNeedsFacts(graph.summary(module.moduleId), ranges);
    module.reports = verifiedReports(module.reports, ranges);
    if (generationUnverified || module.diagnostics.length) {
      analysis.generation = "blocked";
    }
    if (generationUnverified) {
      clearArtifacts(module);
    }
    analysis.batches.push({
      module: module.moduleId,
      reports: module.reports,
    });
  }
}

/**
 * Checks whether unavailable semantic information affects a probe required for lowering.
 *
 * @param summary - Scanner summary for the module, or undefined if unavailable.
 * @param ranges - Unverified ranges in original UTF-8 bytes. Missing bounds cover the whole module.
 * @returns Whether an unbounded range exists or a bounded range overlaps a probe beyond recognition.
 */
function generationNeedsFacts(
  summary: NativeModuleSummary | undefined,
  ranges: UnverifiedRange[],
): boolean {
  return ranges.some((range) => {
    if (range.start === undefined || range.end === undefined) {
      return true;
    }
    return summary?.typeProbes.some(
      (probe) => !probe.recognitionOnly && overlaps(range, probe.start, probe.end),
    );
  });
}

/**
 * Keeps source failures while omitting missing checker facts errors in unverified ranges.
 *
 * @param reports - Reports with original UTF-8 locations.
 * @param ranges - Unverified original UTF-8 ranges for the same module.
 * @returns A filtered array in report order. Other diagnostic codes remain unchanged.
 */
function verifiedReports(reports: BindingReport[], ranges: UnverifiedRange[]): BindingReport[] {
  return reports.filter((report) => {
    if (report.code !== checkerFactsRequiredCode) {
      return true;
    }
    const { start, end } = report.primary.location;
    return !ranges.some((range) => overlaps(range, start, end));
  });
}

/** `CompileDiagnostic::CheckerFactsRequired`, which unverified semantic ranges can cause. */
const checkerFactsRequiredCode = "SW30001";

/**
 * Tests whether a diagnostic or probe overlaps an unavailable semantic range.
 *
 * @param range - Half-open original UTF-8 range. Missing bounds cover the entire module.
 * @param start - Inclusive original UTF-8 byte offset.
 * @param end - Exclusive original UTF-8 byte offset.
 * @returns Whether the ranges intersect, treating an unbounded range as intersecting everything.
 */
function overlaps(range: UnverifiedRange, start: number, end: number): boolean {
  if (range.start === undefined || range.end === undefined) {
    return true;
  }
  return start < range.end && end > range.start;
}

/**
 * Removes generated artifacts while keeping reports and composition metadata for analysis.
 *
 * @param module - Mutable output whose missing facts make its generated artifacts unreliable.
 */
function clearArtifacts(module: ProjectModuleOutput): void {
  module.code = null;
  module.css = "";
  module.resources = [];
  module.classes = [];
  module.symbols = [];
  module.mappings = [];
}

/**
 * Checks output identities, reports lint and rejected-name warnings, and enforces diagnostic policy.
 *
 * @param modules - Native outputs with identities registered in graph.
 * @param graph - Source graph used to verify each output's host identity.
 * @param host - Warning receiver, which may receive messages before a later failure.
 * @param options - Collect mode retains compiler reports instead of throwing.
 * @throws If an output identity is unknown or throw mode encounters compiler errors.
 */
function reportOutputDiagnostics(
  modules: ProjectModuleOutput[],
  graph: ModuleGraph,
  host: CompilerHost,
  options: ModuleCompileOptions,
): void {
  for (const output of modules) {
    graph.resolverId(output.moduleId);
    if (output.diagnostics.length > 0 && options.diagnostics !== "collect") {
      throw new CompilationFailure(
        modules
          .filter((module) => module.diagnostics.length > 0)
          .map((module) => ({
            module: module.moduleId,
            reports: module.reports,
          })),
      );
    }
    reportLintWarnings(output, host);
    warnRejectedNames(output.rejectedNames ?? [], (message) => host.warn(message));
  }
}

/**
 * Reports naming fallbacks through the host warning channel without a source location.
 *
 * @param rejected - Generator rejections in request order.
 * @param warn - Receives the family, input hash, and rejection reason for each fallback.
 */
export function warnRejectedNames(
  rejected: readonly BindingRejectedIdentifier[],
  warn: (message: string) => void,
): void {
  for (const entry of rejected) {
    warn(`stillwind: the ${entry.kind} name for ${entry.hash} was rejected: ${entry.reason}`);
  }
}

/**
 * Sends enabled lint findings through the host without changing their report severity.
 *
 * @param output - Findings and per-rule lint levels. Missing settings default to off.
 * @param host - Receiver for warning text with original source positions.
 */
function reportLintWarnings(output: ProjectModuleOutput, host: CompilerHost): void {
  for (const finding of output.lintFindings ?? []) {
    const level = output.lintSettings?.[finding.rule] ?? "off";
    if (level === "off") {
      continue;
    }
    host.warn(`${finding.rule}: ${finding.message} (${finding.uri}:${finding.start})`);
  }
}

/**
 * Projects a report's primary label into the diagnostic shape returned by compilation.
 *
 * @param report - Structured compiler report.
 * @returns Primary location and scope, diagnostic code and class, message, and notes. Related
 * labels and severity remain in the report.
 */
function reportDiagnostic(report: BindingReport): BindingDiagnostic {
  return {
    ...report.primary.location,
    scope: report.primary.scope,
    code: report.code,
    ...(report.class && { class: report.class }),
    message: report.message,
    notes: report.notes,
  };
}

/**
 * Builds a collect-mode entry output when preparation, scanning, or checking failed.
 *
 * @param moduleId - Compiler identity of the entry module.
 * @param batches - Failure reports from the attempted compilation.
 * @param hasModuleCssImport - Whether scanning found an explicit CSS value import in the entry.
 * @returns An output with null code, empty generated artifacts, all reports, and diagnostics
 * derived only from error reports.
 */
function failedOutput(
  moduleId: string,
  batches: readonly NativeDiagnosticBatch[],
  hasModuleCssImport: boolean,
) {
  const reports = batches.flatMap((batch) => batch.reports);

  return {
    moduleId,
    code: null,
    css: "",
    resources: [],
    mappings: [],
    classes: [],
    symbols: [],
    diagnostics: reports.filter((report) => report.severity === "error").map(reportDiagnostic),
    reports,
    rejectedNames: [],
    dynamicCandidates: [],
    lintFindings: [],
    lintSettings: {} as Record<string, "off" | "warn" | "deny">,
    composition: [],
    hasModuleCssImport,
    compositionModules: [],
    compositionTopology: "[]",
  };
}
