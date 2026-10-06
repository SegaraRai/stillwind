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
import type {
  CompilerHost,
  ModuleAnalysis,
  ModuleCompileOptions,
  SemanticDependency,
} from "./compiler.ts";
import { byteToUtf16, cleanModuleId } from "./module-source.ts";

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
  readonly #attempted = new Map<string, PreparedSource>();
  readonly #summaries = new Map<string, NativeModuleSummary>();
  readonly #sources = new Map<string, PreparedSource>();
  readonly #resolverIds = new Map<string, string>();
  readonly #resolutions = new Map<string, Map<string, string>>();
  #needsTypeFacts = false;

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
   */
  constructor(
    readonly session: NativeProjectSession,
    readonly factsService: () => TypeFactsBackend | undefined,
    readonly host: CompilerHost,
    readonly options: ModuleCompileOptions,
    readonly analysis?: ModuleAnalysis,
  ) {}

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
      const source = "kind" in input ? input : await prepareSource(input);
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
    const result = this.session.scanSource(source);
    if (result.kind === "failed") {
      throw new CompilationFailure([result.batch]);
    }
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
      source: PreparedSource;
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

  #assets() {
    return new Map([...this.#summaries].map(([id, summary]) => [id, summary.assets]));
  }

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

  async #semanticResolutions(
    source: PreparedSource,
  ): Promise<ReadonlyMap<string, string> | undefined> {
    if (!this.host.typeFactsResolution) {
      return undefined;
    }
    try {
      return (
        await this.factsService()?.resolveModules(
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
    const id = resolved.id;
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

  async #load(id: string): Promise<ProjectInput | PreparedSource> {
    if (this.host.load) {
      return this.host.load(id);
    }
    const descriptor = this.host.describe(id);
    return {
      resolverId: id,
      ...descriptor,
      document: {
        ...descriptor.document,
        code: await this.host.read(id),
      },
    };
  }

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
        const available = await backend.collectAvailable(
          this.#checkerSources(),
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
