import type {
  BindingReport,
  NativeDiagnosticBatch,
  NativeModuleSummary,
  NativeProjectSession,
  NativeTypeFact,
} from "@stillwind/native";
import {
  checkerEnvironment,
  PreparationError,
  prepareSource,
  type PreparedSource,
  type ProjectInput,
} from "@stillwind/projection";

import {
  TypeFactsError,
  type TypeFactsBackend,
  type TypeFactSource,
} from "../semantics/backend.ts";
import { loadSource, type CompilerHost } from "./host.ts";
import { byteToUtf16, cleanModuleId } from "./module-source.ts";
import type { CompilationProject } from "./project.ts";
import type { ModuleAnalysis, ModuleCompileOptions, SemanticDependency } from "./request.ts";

/**
 * Compilation failure carrying diagnostic batches for the same report format used by collect mode.
 * Its message renders those reports with source locations.
 */
export class CompilationFailure extends Error {
  /**
   * Report batches grouped by compiler module identity. The supplied array is retained without
   * copying.
   */
  readonly batches: readonly NativeDiagnosticBatch[];

  /**
   * Builds an error message from the supplied reports and preserves an optional cause.
   *
   * @param batches - Reports to retain, in message order.
   * @param cause - Optional preparation or checker failure that produced these reports.
   */
  constructor(batches: readonly NativeDiagnosticBatch[], cause?: unknown) {
    super(
      batches
        .flatMap((batch) => batch.reports)
        .map(formatReport)
        .join("\n"),
      { cause },
    );
    this.batches = batches;
  }
}

/**
 * Formats a compiler report for an Error message or terminal output.
 *
 * @param report - Structured report with a primary label, related labels, and notes.
 * @returns Text with one-based line and column ranges for source labels, followed by related labels
 * and notes. Document-scoped primary labels use only the URI.
 */
function formatReport(report: BindingReport) {
  const { location, scope } = report.primary;
  const at =
    scope === "document"
      ? location.uri
      : `${location.uri}:${location.startLine + 1}:${location.startColumn + 1}-${location.endLine + 1}:${location.endColumn + 1}`;
  const related = report.related.map((label) => {
    const loc = label.location;
    const at =
      label.scope === "document"
        ? loc.uri
        : `${loc.uri}:${loc.startLine + 1}:${loc.startColumn + 1}`;
    return `  ${at}${label.message ? `: ${label.message}` : ""}`;
  });
  return [
    `${at}: ${report.code}: ${report.message}`,
    ...related,
    ...report.notes.map((note) => `  ${note}`),
  ].join("\n");
}

/** Collects prepared sources and semantic facts for one entry's demanded dependency graph. */
export class ModuleGraph {
  /** Prepared sources keyed by compiler identity, including sources that later fail scanning. */
  readonly #attempted = new Map<string, PreparedSource>();
  /** Successful scanner summaries keyed by compiler identity. */
  readonly #summaries = new Map<string, NativeModuleSummary>();
  /** Sources retained for compilation, keyed by full host identity. */
  readonly #sources = new Map<string, PreparedSource>();
  /** Correspondence from compiler identities to full host resolver identities. */
  readonly #resolverIds = new Map<string, string>();
  /** Observed import resolutions keyed by importing resolver identity and authored specifier. */
  readonly #resolutions = new Map<string, Map<string, string>>();
  /** Whether entry probes require semantic traversal and checker collection. */
  #needsTypeFacts = false;
  /** Only this project revision may store scan results or install shared checker inputs. */
  readonly #projectRevision?: number;

  /**
   * Uses a caller-owned session, host, and lazy semantic backend for one entry.
   * Supplying `analysis` retains unavailable dependencies and partial facts in that result.
   * Omitting it keeps checker and dependency failures strict.
   *
   * @param session - Initialized native session. The caller retains ownership.
   * @param factsService - Lazy backend for checker queries, or undefined when unavailable.
   * @param host - Stable source descriptors, loading, and resolver operations for this graph.
   * @param options - Diagnostic policy and captured checker inputs for one compilation.
   * @param analysis - Optional result object to populate with observed sources, dependencies,
   * reports, and unavailable ranges.
   * @param project - Optional prepared inputs shared across scoped graphs of the same project.
   */
  constructor(
    readonly session: NativeProjectSession,
    readonly factsService: () => TypeFactsBackend | undefined,
    readonly host: CompilerHost,
    readonly options: ModuleCompileOptions,
    readonly analysis?: ModuleAnalysis,
    readonly project?: CompilationProject,
  ) {
    this.#projectRevision = project?.revision;
  }

  /**
   * Prepares source and retains it for report mapping, even if a later scan fails.
   *
   * @param input - Original or already prepared source. Preparation must preserve resolverId.
   * @returns Prepared source. onModule runs before preparation and does not imply success.
   * @throws If preparation fails or changes the resolver identity. PreparationError becomes CompilationFailure.
   */
  async prepare(input: ProjectInput | PreparedSource): Promise<PreparedSource> {
    this.host.onModule?.(input.moduleId, input.resolverId);
    try {
      const source =
        this.project?.source(input.resolverId) ??
        ("kind" in input ? input : await prepareSource(input));
      if (source.resolverId !== input.resolverId) {
        throw new Error(`Host changed resolver identity for ${input.resolverId}`);
      }
      this.#attempted.set(source.moduleId, source);
      this.analysis?.sources.push(source);
      return source;
    } catch (error) {
      if (error instanceof PreparationError) {
        throw this.#preparationFailure(input, error);
      }
      throw error;
    }
  }

  /**
   * Scans a prepared source without registering it in dependency traversal.
   *
   * @param source - Source with validated original document and projection correspondences.
   * @returns Native summary of imports, declarations, and semantic probes.
   * @throws CompilationFailure when scanning returns source diagnostics.
   */
  scan(source: PreparedSource): NativeModuleSummary {
    const cached = this.project?.summary(source);
    if (cached) {
      return cached;
    }
    const result = this.session.scanSource(source);
    if (result.kind === "failed") {
      throw new CompilationFailure([result.batch]);
    }
    this.project?.retain(source, result.summary, this.#projectRevision);
    return result.summary;
  }

  /**
   * Looks up a summary retained during dependency traversal.
   *
   * @param moduleId - Compiler identity of the source.
   * @returns Its cached summary, or undefined when traversal did not retain one.
   */
  summary(moduleId: string): NativeModuleSummary | undefined {
    return this.#summaries.get(moduleId);
  }

  /**
   * Finds the host identity registered for a compiler module.
   *
   * @param moduleId - Compiler identity registered by dependency collection.
   * @returns Full resolver identity, including any query suffix.
   * @throws If the compiler refers to an unregistered module.
   */
  resolverId(moduleId: string): string {
    const id = this.#resolverIds.get(moduleId);
    if (id === undefined) {
      throw new Error(`Stillwind returned an unknown module: ${moduleId}`);
    }
    return id;
  }

  /**
   * Registers the entry and loads dependencies demanded by its semantic probes.
   *
   * @param entry - Prepared entry source.
   * @param entrySummary - Successful scan of that entry, with the same module identity.
   * @returns A promise that resolves after traversal. Analysis mode retains unresolved and
   * unsupported dependencies rather than failing the whole graph.
   * @throws If strict traversal fails or source identities conflict.
   */
  async collectDependencies(
    entry: PreparedSource,
    entrySummary: NativeModuleSummary,
  ): Promise<void> {
    this.#needsTypeFacts = entrySummary.typeProbes.length > 0;
    this.#register(entry);
    const visited = new Map<string, Set<string | null>>();
    const pending: {
      /** Registered source whose exported value or entry probes remain to be traversed. */
      source: PreparedSource;
      /** Null for entry probes, * for namespace demand, or a demanded export name. */
      requested: string | null;
    }[] = [
      {
        source: entry,
        requested: null,
      },
    ];
    while (pending.length > 0) {
      const { source, requested } = pending.pop()!;
      const demands = visited.get(source.resolverId) ?? new Set<string | null>();
      if (demands.has(requested)) {
        continue;
      }
      demands.add(requested);
      visited.set(source.resolverId, demands);
      const summary = this.#dependencySummary(source, entry, entrySummary);
      if (!summary || !this.#needsTypeFacts) {
        continue;
      }
      const semanticResolutions = await this.#semanticResolutions(source);
      for (const dependency of semanticDependencies(summary, requested)) {
        const next = await this.#followDependency(
          source,
          dependency.source,
          semanticResolutions?.get(dependency.source),
        );
        if (next) {
          pending.push({
            source: next,
            requested: dependency.imported,
          });
        }
      }
    }
  }

  /**
   * Attaches semantic facts and CSS value imports to the retained graph sources.
   *
   * @param entry - Registered entry whose transport imports remain unchanged.
   * @returns Copies of retained sources with native type facts. Analysis mode records unavailable
   * queries, while strict compilation requires a backend when facts are needed.
   * @throws If strict checker collection or a checker source contract fails.
   */
  async collectTypedSources(entry: PreparedSource) {
    const facts = await this.#collectFacts();
    return [...this.#sources.values()].map((source) => ({
      ...source,
      moduleCssImport:
        source.resolverId === entry.resolverId ? source.moduleCssImport : "stillwind/module-css",
      typeFacts: facts.get(source.moduleId) ?? [],
    }));
  }

  /**
   * Converts a known checker failure to a report in original source coordinates.
   *
   * @param error - Failure caught during compilation.
   * @returns CompilationFailure for TypeFactsError whose source was prepared, otherwise the input.
   */
  failure(error: unknown): unknown {
    if (!(error instanceof TypeFactsError)) {
      return error;
    }
    const source = this.#attempted.get(error.moduleId);
    if (!source) {
      return error;
    }
    const range =
      error.start !== undefined && error.end !== undefined
        ? {
            start: byteToUtf16(source.document.code, error.start),
            end: byteToUtf16(source.document.code, error.end),
          }
        : undefined;
    const batch = this.session.reportSourceFailure({
      moduleId: source.moduleId,
      document: source.document,
      issue: error.issue,
      range,
      converter: source.kind === "projected" ? source.projection.converter : undefined,
    });
    return new CompilationFailure([batch], error);
  }

  /**
   * Registers the source and enforces one-to-one host and compiler identities within the graph.
   *
   * @param source - Prepared source to retain for traversal and compilation.
   * @throws If a compiler identity is shared or a host identity changes its compiler identity.
   */
  #register(source: PreparedSource): void {
    const previous = this.#resolverIds.get(source.moduleId);
    if (previous !== undefined && previous !== source.resolverId) {
      throw new Error(
        `Module identity ${source.moduleId} is shared by ${previous} and ${source.resolverId}`,
      );
    }
    const existing = this.#sources.get(source.resolverId);
    if (existing !== undefined && existing.moduleId !== source.moduleId) {
      throw new Error(`Resolver identity ${source.resolverId} changed module identity`);
    }
    this.#resolverIds.set(source.moduleId, source.resolverId);
    this.#sources.set(source.resolverId, source);
  }

  /**
   * Converts a preparation error into an original-document diagnostic batch.
   *
   * @param input - Source whose document metadata identifies the failing preparation.
   * @param error - Preparation issue with optional original UTF-16 range and converter identity.
   * @returns A compilation failure retaining the preparation error as its cause.
   */
  #preparationFailure(
    input: ProjectInput | PreparedSource,
    error: PreparationError,
  ): CompilationFailure {
    return new CompilationFailure(
      [
        this.session.reportSourceFailure({
          moduleId: input.moduleId,
          document: input.document,
          issue: error.issue,
          converter: error.converter,
          range: error.range,
        }),
      ],
      error,
    );
  }

  /**
   * Reuses or scans a source summary and records dependency syntax failures in analysis mode.
   *
   * @param source - Registered dependency or entry source.
   * @param entry - Entry source whose initial scan is already available.
   * @param entrySummary - Summary with the entry's compiler identity.
   * @returns A retained summary, or undefined after an analysis-mode scan failure removes the source.
   * @throws If scanning fails in strict mode or a summary changes the source's compiler identity.
   */
  #dependencySummary(
    source: PreparedSource,
    entry: PreparedSource,
    entrySummary: NativeModuleSummary,
  ): NativeModuleSummary | undefined {
    let summary: NativeModuleSummary;
    try {
      summary =
        this.#summaries.get(source.moduleId) ??
        (source.resolverId === entry.resolverId ? entrySummary : this.scan(source));
    } catch (error) {
      if (!this.analysis || !(error instanceof CompilationFailure)) {
        throw error;
      }
      this.analysis.batches.push(...error.batches);
      this.#sources.delete(source.resolverId);
      for (const dependency of this.analysis.dependencies) {
        if (dependency.id === source.resolverId) {
          dependency.status = "unsupported";
        }
      }
      return undefined;
    }
    if (summary.moduleId !== source.moduleId) {
      throw new Error(`Scanner changed module identity for ${source.resolverId}`);
    }
    this.#summaries.set(source.moduleId, summary);
    return summary;
  }

  /**
   * Attaches checker environments, scanner probes, and observed resolutions to retained sources.
   *
   * @returns Checker inputs in source registration order. Missing summaries are scanned and cached.
   * @throws If scanning or checker preparation fails.
   */
  #checkerSources(): TypeFactSource[] {
    return [...this.#sources.values()].map((source) => {
      const summary = this.#summaries.get(source.moduleId) ?? this.scan(source);
      this.#summaries.set(source.moduleId, summary);
      let environment;
      try {
        environment = checkerEnvironment(source);
      } catch (error) {
        if (error instanceof PreparationError) {
          throw this.#preparationFailure(source, error);
        }
        throw error;
      }
      return {
        ...source,
        ...environment,
        checkerSpans: summary.checkerSpans,
        checkerBindings: summary.checkerBindings,
        probes: summary.typeProbes,
        resolutions: summary.moduleSpecifiers.flatMap((specifier) => {
          const id = this.#resolutions.get(source.resolverId)?.get(specifier.source);
          return id
            ? [
                {
                  id,
                  start: specifier.start,
                  end: specifier.end,
                },
              ]
            : [];
        }),
      };
    });
  }

  /**
   * Collects declared assets for checker interpretation.
   *
   * @returns A new map from compiler identity to the corresponding scanner asset array.
   */
  #assets() {
    return new Map([...this.#summaries].map(([id, summary]) => [id, summary.assets]));
  }

  /**
   * Adds an unverified range for each retained probe when analysis mode has no usable facts.
   *
   * @param source - Source whose scanner probes use original UTF-8 byte coordinates.
   * @param reason - Explanation included in every added range. Strict mode has no analysis to update.
   */
  #unavailable(source: PreparedSource, reason: string): void {
    this.analysis?.unverified.push(
      ...(this.#summaries.get(source.moduleId)?.typeProbes ?? []).map((probe) => ({
        moduleId: source.moduleId,
        start: probe.start,
        end: probe.end,
        reason,
      })),
    );
  }

  /**
   * Requests checker resolutions when the host enables them, recording failures as unverified probes.
   *
   * @param source - Importing source whose resolver identity selects the returned map.
   * @returns Authored specifiers mapped to host identities, or undefined when disabled or unavailable.
   * @throws If resolution fails outside analysis mode.
   */
  async #semanticResolutions(
    source: PreparedSource,
  ): Promise<ReadonlyMap<string, string> | undefined> {
    if (!this.host.typeFactsResolution) {
      return undefined;
    }
    try {
      const backend = this.factsService();
      if (backend && this.project && this.project.revision === this.#projectRevision) {
        return (
          await this.project.resolve(backend, this.#checkerSources(), this.options.typeFactsInputs)
        ).get(source.resolverId);
      }
      return (
        await backend?.resolveModules(
          this.#checkerSources(),
          this.#assets(),
          this.options.typeFactsInputs,
        )
      )?.get(source.resolverId);
    } catch (error) {
      if (!this.analysis) {
        throw error;
      }
      this.#unavailable(source, String(error));
    }
    return undefined;
  }

  /**
   * Resolves a demanded dependency, records its status, and loads supported source for traversal.
   *
   * @param source - Registered importing source.
   * @param specifier - Authored module request.
   * @param semanticId - Optional checker fallback used when the host returns no resolution.
   * @returns Registered dependency source, or undefined for external, unresolved, or unsupported requests.
   * @throws If strict loading fails. Analysis mode records loading failures and available diagnostics.
   */
  async #followDependency(
    source: PreparedSource,
    specifier: string,
    semanticId?: string,
  ): Promise<PreparedSource | undefined> {
    const resolved = await this.#resolveDependency(source, specifier, semanticId);
    const observed: SemanticDependency = {
      importer: source.resolverId,
      specifier,
      id: resolved?.id,
      status: !resolved ? "unresolved" : resolved.external ? "external" : "resolved",
    };
    this.analysis?.dependencies.push(observed);
    if (!resolved || resolved.external) {
      return undefined;
    }
    const id = this.project?.source(resolved.id)?.resolverId ?? resolved.id;
    observed.id = id;
    if (!this.#canLoadDependency(id, observed)) {
      return undefined;
    }
    let resolutions = this.#resolutions.get(source.resolverId);
    if (!resolutions) {
      this.#resolutions.set(source.resolverId, (resolutions = new Map()));
    }
    resolutions.set(specifier, id);
    this.host.watch(id);
    try {
      let dependency = this.#sources.get(id);
      if (!dependency) {
        dependency = await this.prepare(await this.#load(id));
        this.#register(dependency);
      }
      return dependency;
    } catch (error) {
      if (!this.analysis) {
        throw error;
      }
      observed.status = "unsupported";
      this.analysis.unverified.push({ moduleId: source.moduleId, reason: String(error) });
      if (error instanceof CompilationFailure) {
        this.analysis.batches.push(...error.batches);
      }
    }
    return undefined;
  }

  /**
   * Uses host resolution first and a checker identity only when the host returns null.
   *
   * @param source - Importer whose full resolver identity is passed to the host.
   * @param specifier - Authored module request.
   * @param semanticId - Optional identity supplied by checker resolution.
   * @returns A resolution, or null when unavailable. Analysis mode converts host exceptions to null.
   * @throws If host resolution fails in strict mode.
   */
  async #resolveDependency(source: PreparedSource, specifier: string, semanticId?: string) {
    try {
      return (
        (await this.host.resolve(specifier, source.resolverId, { skipSelf: true })) ??
        (semanticId ? { id: semanticId } : null)
      );
    } catch (error) {
      if (!this.analysis) {
        throw error;
      }
      return null;
    }
  }

  /**
   * Distinguishes traversable sources from checker declarations and unsupported dependencies.
   *
   * @param id - Full resolved host identity.
   * @param observed - Mutable dependency record whose status becomes types or unsupported when skipped.
   * @returns Whether the dependency should be loaded as source.
   */
  #canLoadDependency(id: string, observed: SemanticDependency): boolean {
    const name = cleanModuleId(id);
    // Build hosts can resolve runtime exports without identifying declaration files.
    // The checker supplies those package types.
    if (
      /\.d\.[cm]?ts$/.test(name) ||
      (!this.host.typeFactsResolution && /(?:^|[\\/])node_modules[\\/]/.test(name))
    ) {
      observed.status = "types";
      return false;
    }
    if (!(this.host.supports?.(id) ?? /\.(?:[cm]?[jt]sx?|astro|svelte|vue)$/.test(name))) {
      observed.status = "unsupported";
      return false;
    }
    return true;
  }

  /**
   * Reuses a prepared project source or loads new input through the host.
   *
   * @param id - Full resolved dependency identity.
   * @returns A shared prepared source or new input from the host. Do not mutate cached sources.
   * @throws If host loading, description, or reading fails.
   */
  async #load(id: string): Promise<ProjectInput | PreparedSource> {
    const cached = this.project?.source(id);
    if (cached) {
      return cached;
    }
    return loadSource(this.host, id);
  }

  /**
   * Collects available facts for retained sources when entry probes require semantic information.
   *
   * @returns Facts keyed by compiler identity. Analysis mode records unavailable queries and may return partial facts.
   * @throws If strict checker collection fails or a compilation failure occurs while preparing checker sources.
   */
  async #collectFacts(): Promise<Map<string, NativeTypeFact[]>> {
    if (!this.#needsTypeFacts) {
      return new Map();
    }
    try {
      const backend = this.factsService();
      if (!backend) {
        if (!this.analysis) {
          throw new Error("Type facts required for this compilation are unavailable");
        }
        for (const source of this.#sources.values()) {
          this.#unavailable(source, "Semantic queries are disabled");
        }
        return new Map();
      }
      if (this.analysis) {
        const sources =
          (this.project?.revision === this.#projectRevision
            ? this.project?.install(backend, this.#checkerSources(), this.options.typeFactsInputs)
            : undefined) ?? this.#checkerSources();
        const available = await backend.collectAvailable(
          sources,
          this.#assets(),
          this.options.typeFactsInputs,
        );
        this.analysis.unverified.push(...available.unverified);
        return available.facts;
      }
      return await backend.collect(
        this.#checkerSources(),
        this.#assets(),
        this.options.typeFactsInputs,
      );
    } catch (error) {
      if (!this.analysis || error instanceof CompilationFailure) {
        throw error;
      }
      for (const source of this.#sources.values()) {
        this.#unavailable(source, String(error));
      }
      return new Map();
    }
  }
}

/**
 * Selects imports needed to answer the current semantic demand.
 *
 * @param summary - Scanner summary for the source being traversed.
 * @param requested - Null for entry probes, `*` for a namespace demand, or a specific exported
 * name.
 * @returns Probe imports, imports for a matched export, or matching re-export demands. Unmatched
 * and namespace demands conservatively retain all imports and re-exports.
 */
function semanticDependencies(summary: NativeModuleSummary, requested: string | null) {
  if (requested === null) {
    return summary.probeImports;
  }
  const selected = summary.exportImports.find((entry) => entry.name === requested);
  if (requested !== "*" && selected) {
    return selected.imports;
  }
  const reExports = summary.reExports.filter(
    (entry) => requested === "*" || entry.exported === requested || entry.exported === "*",
  );
  if (requested !== "*" && reExports.length > 0) {
    return reExports.map((entry) => ({
      source: entry.source,
      imported: entry.exported === "*" ? requested : entry.imported,
    }));
  }
  // Namespace imports and exports without a declaration slice remain conservative.
  return [...summary.imports, ...summary.reExports];
}
