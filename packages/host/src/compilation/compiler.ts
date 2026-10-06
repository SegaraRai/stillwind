import type {
  BindingDiagnostic,
  BindingReport,
  NativeDiagnosticBatch,
  NativeModuleSummary,
  NativePreparedSource,
  NativeProjectSession,
  ProjectModuleOutput,
} from "@stillwind/native";
import type { PreparedSource, ProjectInput } from "@stillwind/projection";

import type { TypeFactsBackend, TypeFactsInputs, UnverifiedRange } from "../semantics/index.ts";
import { CompilationFailure, ModuleGraph } from "./module-graph.ts";
import type { ModuleDescriptor } from "./module-source.ts";

/**
 * Source metadata, dependency resolution, reads, watches, and warnings supplied to module
 * compilation. Hosts must preserve a stable correspondence between resolver and compiler module
 * identities.
 */
export interface CompilerHost {
  /** Requests checker import resolutions. A non-null host resolve result still takes precedence. */
  typeFactsResolution?: boolean;
  /**
   * Loads source before or after preparation, including its original document and projection.
   *
   * @param id - Full resolver identity. The returned resolverId must equal this value.
   * @returns Original input for preparation or a prepared source. Omission uses describe and read.
   * @throws If loading or preparation fails.
   */
  load?(id: string): Promise<ProjectInput | PreparedSource>;
  /**
   * Determines whether a resolved dependency supplies source that Stillwind can analyze.
   *
   * @param id - Full resolved identity, including query suffixes.
   * @returns Whether source traversal can continue. Omission checks for supported JS/TS and SFC extensions.
   */
  supports?(id: string): boolean;
  /**
   * Describes one resolved source before it is scanned or compiled.
   *
   * @param id - Full resolver identity of the source, including any query suffix.
   * @returns Compiler and checker identities, parser metadata, and original document metadata.
   */
  describe(id: string): ModuleDescriptor;
  /**
   * Resolves a semantic dependency relative to its importing source.
   *
   * @param specifier - Authored module request text.
   * @param importer - Full resolver identity of the importing source.
   * @param options - Optional resolver policy. Compilation passes `skipSelf: true`.
   * @returns A promise for a resolved identity and optional external status, or null when
   * unresolved. Unresolved and external modules are skipped by semantic dependency traversal.
   */
  resolve(
    specifier: string,
    importer: string,
    options?: {
      skipSelf: boolean;
    },
  ): Promise<{
    id: string;
    external?: boolean | string;
  } | null>;
  /**
   * Reads one resolved dependency before source preparation.
   *
   * @param id - Full resolver identity, including any query selecting transformed source.
   * @returns A promise for source text. Read failures reject the compilation.
   */
  read(id: string): Promise<string>;
  /**
   * Records the identity correspondence before a source is prepared.
   *
   * This callback can run before preparation fails. Callers must not treat it as proof of a
   * successful compilation.
   *
   * @param moduleId - Stable identity used by the compiler and checker results.
   * @param resolverId - Full host identity used to resolve and read this source.
   */
  onModule?(moduleId: string, resolverId: string): void;
  /**
   * Registers a resolved semantic dependency for change tracking.
   *
   * @param id - Full dependency identity selected by the host resolver.
   */
  watch(id: string): void;
  /**
   * Reports a lint warning or a rejected generated name through the host.
   *
   * @param message - Formatted warning text. No structured diagnostic location is implied.
   */
  warn(message: string): void;
}

/** Diagnostics, captured checker inputs, and generated imports for one module compilation. */
export interface ModuleCompileOptions {
  /** Files captured for this compilation's semantic queries. Omission uses disk dependencies. */
  typeFactsInputs?: TypeFactsInputs;
  /**
   * Whether compiler and checker diagnostics reject compilation or are returned in reports.
   * Defaults to `throw`. Host I/O and malformed transport failures still reject in `collect` mode.
   */
  diagnostics?: "throw" | "collect";
  /** Module specifiers injected into the entry source to activate its styles, in import order. */
  stylesheetImports?: string[];
  /**
   * JavaScript module exporting CSS for an explicit `stillwind/module-css` import in the entry.
   * Omission produces a compiler diagnostic when the entry contains that import.
   */
  moduleCssImport?: string;
}

/** One semantic import observed while answering an entry's type probes. */
export interface SemanticDependency {
  /** Full resolver identity of the importing source. */
  importer: string;
  /** Authored import request text. */
  specifier: string;
  /** Resolved identity, when resolution succeeded. */
  id?: string;
  /**
   * Resolved means analyzable source. External modules and dependencies handled through checker
   * declarations (`types`) are not traversed. Unresolved or unsupported dependencies leave analysis incomplete.
   */
  status: "resolved" | "external" | "unresolved" | "unsupported" | "types";
}

/** Current reports and sources for the semantic graph demanded by one entry. */
export interface ModuleAnalysis {
  /** Compiler identity of the entry after loading its descriptor. */
  entry: string;
  /** Traversal covers dependencies demanded by semantic probes, rather than all application imports. */
  coverage: "observed";
  /** Outputs produced by native project compilation, including diagnostic-only outputs. */
  modules: ProjectModuleOutput[];
  /** Structured diagnostic batches keyed by compiler module identity. */
  batches: NativeDiagnosticBatch[];
  /** Prepared original sources retained for mapping reports, including sources that later fail scanning. */
  sources: PreparedSource[];
  /** Import requests and their resolution or support status observed during traversal. */
  dependencies: SemanticDependency[];
  /** Original UTF-8 ranges that lack information needed for semantic checks or code generation. */
  unverified: UnverifiedRange[];
  /** Blocked when compiler errors or missing facts needed for lowering prevent reliable code generation. */
  generation: "complete" | "blocked";
}

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
 * @returns Reports and sources for the semantic dependencies demanded by the entry.
 * @throws If entry I/O fails, a host changes resolver identity, or native transport operations fail.
 */
export async function analyzeModule(
  session: NativeProjectSession,
  factsService: () => TypeFactsBackend | undefined,
  id: string,
  host: CompilerHost,
  typeFactsInputs?: TypeFactsInputs,
): Promise<ModuleAnalysis> {
  const input = host.load
    ? await host.load(id)
    : {
        resolverId: id,
        ...host.describe(id),
        document: {
          ...host.describe(id).document,
          code: await host.read(id),
        },
      };
  const analysis: ModuleAnalysis = {
    entry: input.moduleId,
    coverage: "observed",
    modules: [],
    batches: [],
    sources: [],
    dependencies: [],
    unverified: [],
    generation: "complete",
  };
  if (input.resolverId !== id) {
    throw new Error(`Host changed resolver identity for ${id}`);
  }
  await compileGraph(
    session,
    factsService,
    input.document.code,
    id,
    host,
    {
      diagnostics: "collect",
      moduleCssImport: "stillwind/module-css",
      typeFactsInputs,
    },
    analysis,
    input,
  );
  return analysis;
}

/**
 * Compiles an entry buffer and its demanded semantic dependencies into a project session.
 * The host descriptor supplies the module identity and parser metadata for each source.
 * Type facts are requested lazily for original-coordinate probes in JS/TS or SFC inputs.
 *
 * Collect mode retains compiler and checker reports, including known syntax and projection
 * failures. The default throw mode rejects compiler diagnostics and deny-level lint findings.
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

async function compileGraph(
  session: NativeProjectSession,
  factsService: () => TypeFactsBackend | undefined,
  code: string,
  id: string,
  host: CompilerHost,
  options: ModuleCompileOptions,
  analysis?: ModuleAnalysis,
  entryInput?: ProjectInput | PreparedSource,
) {
  const graph = new ModuleGraph(session, factsService, host, options, analysis);
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
    const projectOutput = session.compile(
      typedSources,
      analysis ? undefined : entrySource.moduleId,
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

/** Keeps source failures while omitting identity lowering errors in unverified ranges. */
function verifiedReports(reports: BindingReport[], ranges: UnverifiedRange[]): BindingReport[] {
  return reports.filter((report) => {
    if (!identityDiagnosticCodes.has(report.code)) {
      return true;
    }
    const { start, end } = report.primary.location;
    return !ranges.some((range) => overlaps(range, start, end));
  });
}

const identityDiagnosticCodes = new Set([
  "SW01004",
  "SW03001",
  "SW03007",
  "SW03010",
  "SW03016",
  "SW03019",
]);

function overlaps(range: UnverifiedRange, start: number, end: number): boolean {
  if (range.start === undefined || range.end === undefined) {
    return true;
  }
  return start < range.end && end > range.start;
}

function clearArtifacts(module: ProjectModuleOutput): void {
  module.code = null;
  module.css = "";
  module.resources = [];
  module.classes = [];
  module.symbols = [];
  module.mappings = [];
}

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
    const denied = reportLintWarnings(output, host);
    if (denied && options.diagnostics !== "collect") {
      throw new CompilationFailure([
        {
          module: output.moduleId,
          reports: output.reports.filter((report) => report.severity === "error"),
        },
      ]);
    }
    // A rejected name comes from the host's own generator rather than from the source, so it
    // carries no position: it is reported over the warning channel, and the compilation has
    // already substituted the built-in name.
    for (const rejected of output.rejectedNames ?? []) {
      host.warn(
        `stillwind: the ${rejected.kind} name for ${rejected.hash} was rejected: ${rejected.reason}`,
      );
    }
  }
}

function reportLintWarnings(output: ProjectModuleOutput, host: CompilerHost): boolean {
  let denied = false;
  for (const finding of output.lintFindings ?? []) {
    const level = output.lintSettings?.[finding.rule] ?? "warn";
    if (level === "off") {
      continue;
    }
    if (level === "deny") {
      denied = true;
    } else {
      host.warn(`${finding.rule}: ${finding.message} (${finding.uri}:${finding.start})`);
    }
  }
  return denied;
}

/**
 * Projects a report's primary label into the diagnostic shape returned by compilation.
 *
 * @param report - Structured compiler report.
 * @returns Primary location and scope, diagnostic code, message, and notes. Related labels and
 * severity remain in the report.
 */
function reportDiagnostic(report: BindingReport): BindingDiagnostic {
  return {
    ...report.primary.location,
    scope: report.primary.scope,
    code: report.code,
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
    lintFindings: [],
    lintSettings: {} as Record<string, "off" | "warn" | "deny">,
    composition: [],
    hasModuleCssImport,
    compositionModules: [],
    compositionTopology: "[]",
  };
}
