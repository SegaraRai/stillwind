import type {
  NativeAssetRecord,
  NativeModuleSummary,
  NativeProjectSession,
  NativeTypeFact,
} from "@stillwind/native";
import { checkerEnvironment, prepareSource, type PreparedSource } from "@stillwind/projection";

import type { SemanticInputs, TypeFactSource, TypeFactsBackend } from "../semantics/backend.ts";
import { loadSource, type CompilerHost } from "./host.ts";
import { sourceSupport } from "./module-graph.ts";

/** A prepared source, its scan results, and the inputs needed by the checker. */
interface CapturedSource {
  /** Owned copy of the prepared source. Callers must not mutate it. */
  source: PreparedSource;
  /** Scan results for the same source text. */
  summary: NativeModuleSummary;
  /** Resolved imports whose ranges use original UTF-8 byte offsets. */
  resolutions: NonNullable<TypeFactSource["resolutions"]>;
  /** Whether the host has tried to resolve every import in this source. */
  linked: boolean;
  /** Resolver domains whose answers for every import of this source match the stored links. */
  verified: Set<string | undefined>;
  /** Created on the first semantic query. Framework dependencies may fail to load. */
  checker?: TypeFactSource;
}

/**
 * Two resolver domains resolved the same import to different source targets, so they cannot share
 * one project. Answers that analysis does not read, such as an external package and its runtime
 * file, may differ. The project keeps the answer that it stored first.
 */
export class ResolutionMismatchError extends Error {
  /**
   * @param importer - Full resolver ID of the importing source.
   * @param specifier - Authored module request.
   * @param domain - Resolver domain whose answer differs from the stored answer.
   */
  constructor(
    readonly importer: string,
    readonly specifier: string,
    readonly domain: string | undefined,
  ) {
    super(
      `Resolver domain ${String(domain)} resolves ${JSON.stringify(specifier)} from ${importer} differently`,
    );
    this.name = "ResolutionMismatchError";
  }
}

/**
 * Keeps prepared sources and checker inputs between analyses of one project.
 *
 * Hosts whose resolvers can answer differently, such as the client and server environments of one
 * bundler, can share a project when each host sets its own `resolverDomain`. The project stores the
 * first answer for an import, and every other domain checks that answer once before using it.
 */
export class CompilationProject {
  /** Sources keyed by normalized resolver ID. Windows paths ignore case. */
  readonly #sources = new Map<string, CapturedSource>();
  /** Collected facts keyed by normalized resolver ID. Present only when sources stay unchanged. */
  readonly #facts?: Map<string, NativeTypeFact[]>;
  /**
   * Host import resolutions keyed by importer resolver ID and specifier. Present only when sources
   * stay unchanged.
   */
  readonly #imports?: Map<
    string,
    {
      resolved: ReturnType<CompilerHost["resolve"]>;
      /** Resolver domains that produced the stored answer or whose own answer matched it. */
      domains: Set<string | undefined>;
    }
  >;
  /** Import answers for the currently installed checker inputs. */
  #resolutions?: ReadonlyMap<string, ReadonlyMap<string, string>>;
  /** Resolver IDs of sources that {@link invalidateFile} removed and {@link prepare} loads again. */
  readonly #reload = new Set<string>();
  /** Advances on invalidation so pending loads cannot publish old sources. */
  #revision = 0;
  /** Advances when sources or import links change so checker inputs are installed again. */
  #generation = 0;
  /** Last backend and inputs installed by this project. */
  #installed?: {
    backend: TypeFactsBackend;
    generation: number;
    inputs?: SemanticInputs;
    sources: readonly TypeFactSource[];
  };

  /**
   * Creates an empty project.
   *
   * @param fixedSources - True when no source text and no import resolution changes until
   * invalidation. The project then keeps collected facts and host import resolutions. With every
   * file that a source imports installed, the facts of a source depend only on its own text, so
   * other analyses can reuse them.
   */
  constructor(fixedSources = false) {
    this.#facts = fixedSources ? new Map() : undefined;
    this.#imports = fixedSources ? new Map() : undefined;
  }

  /** Revision that a pending load must still match before it stores a source. */
  get revision(): number {
    return this.#revision;
  }

  /**
   * Finds a source prepared since its last invalidation.
   *
   * @param id - Full resolver ID. Windows path spelling may differ in case or slashes.
   * @returns The shared source, or undefined when it has not been prepared. Do not mutate it.
   */
  source(id: string): PreparedSource | undefined {
    return this.#sources.get(sourceKey(id))?.source;
  }

  /** Returns the normalized identities of the prepared sources, as {@link sourceKey} defines them. */
  sourceKeys(): string[] {
    return [...this.#sources.keys()];
  }

  /** Returns the prepared documents' original text so plugins can find candidates together. */
  texts(): string[] {
    return [...this.#sources.values()].map((record) => record.source.document.code);
  }

  /**
   * Finds the scan results for a prepared source.
   *
   * @param source - Source whose text has not changed since preparation.
   * @returns Shared scan results, or undefined when the source is not retained. Do not mutate them.
   */
  summary(source: PreparedSource): NativeModuleSummary | undefined {
    const record = this.#sources.get(sourceKey(source.resolverId));
    return record?.source.document.code === source.document.code ? record.summary : undefined;
  }

  /**
   * Finds facts that an earlier analysis collected for a checker source.
   *
   * @param source - Checker source that {@link install} returned.
   * @returns Shared facts, or undefined when they are not kept. Do not mutate them.
   */
  facts(source: TypeFactSource): readonly NativeTypeFact[] | undefined {
    return this.#facts?.get(sourceKey(source.resolverId));
  }

  /**
   * Keeps collected facts for reuse when the project was created with fixed sources.
   *
   * @param source - Checker source that {@link install} returned for the current revision.
   * @param facts - Complete facts that the checker collected for that source.
   */
  retainFacts(source: TypeFactSource, facts: NativeTypeFact[]): void {
    this.#facts?.set(sourceKey(source.resolverId), facts);
  }

  /**
   * Stores a source and its successful scan results, copying the source before retaining it.
   *
   * @param source - Source passed to the scanner.
   * @param summary - Successful scan results for that source. Do not mutate them after this call.
   * @param revision - Revision captured before loading. Defaults to the current revision.
   * Old revisions and sources already stored under the same ID have no effect.
   */
  retain(source: PreparedSource, summary: NativeModuleSummary, revision = this.#revision): void {
    if (revision !== this.#revision || this.#sources.has(sourceKey(source.resolverId))) {
      return;
    }
    source = structuredClone(source);
    this.#sources.set(sourceKey(source.resolverId), {
      source,
      summary,
      linked: false,
      verified: new Set(),
      resolutions: [],
    });
    this.#changed();
  }

  /**
   * Resolves an import through the host. A project with fixed sources asks the host once for each
   * importer, specifier, and resolver domain.
   *
   * @param host - Host that resolves the import. Its `resolverDomain` selects the domain.
   * @param specifier - Authored module request.
   * @param importer - Full resolver ID of the importing source.
   * @returns The stored answer. A failed resolution is not kept.
   * @throws If the host resolution fails, or a ResolutionMismatchError if the host's domain
   * resolves the import to a source target that differs from the stored answer.
   */
  resolveImport(
    host: CompilerHost,
    specifier: string,
    importer: string,
  ): ReturnType<CompilerHost["resolve"]> {
    if (!this.#imports) {
      return host.resolve(specifier, importer, { skipSelf: true });
    }
    const key = `${importer}\0${specifier}`;
    const domain = host.resolverDomain;
    const entry = this.#imports.get(key);
    if (!entry) {
      const resolved = host.resolve(specifier, importer, { skipSelf: true });
      this.#imports.set(key, { resolved, domains: new Set([domain]) });
      resolved.catch(() => this.#imports?.delete(key));
      return resolved;
    }
    if (entry.domains.has(domain)) {
      return entry.resolved;
    }
    return Promise.all([
      entry.resolved,
      host.resolve(specifier, importer, { skipSelf: true }),
    ]).then(([stored, own]) => {
      // Analysis reads only source targets. A package that SSR externalizes and the client
      // resolves to a runtime file gives the same analysis, because the checker supplies its types.
      if (analyzedTarget(host, stored) !== analyzedTarget(host, own)) {
        throw new ResolutionMismatchError(importer, specifier, domain);
      }
      entry.domains.add(domain);
      return stored;
    });
  }

  /**
   * Loads and scans targets together, then resolves their imports through the host.
   * Analysis retries failed loads and scans so it can report each target's own error.
   *
   * @param ids - Full resolver IDs of the targets to prepare.
   * @param host - Host used by later analysis of these targets.
   * @param session - Initialized native session that will analyze the prepared sources.
   * @returns Completion of preparation. Invalidation stops old work from storing results.
   * @throws If host import resolution fails. Source loading and scan failures are left for analysis.
   */
  async prepare(
    ids: readonly string[],
    host: CompilerHost,
    session: NativeProjectSession,
  ): Promise<void> {
    const revision = this.#revision;
    const reload = [...this.#reload];
    this.#reload.clear();
    await Promise.allSettled(
      [...reload, ...ids].map((id) => this.#load(id, host, session, revision)),
    );
    if (revision !== this.#revision) {
      for (const id of reload) {
        this.#reload.add(id);
      }
      return;
    }
    const domain = host.resolverDomain;
    for (const record of this.#sources.values()) {
      const { source, summary } = record;
      if (record.linked && record.verified.has(domain)) {
        continue;
      }
      if (record.linked) {
        // Another domain stored these links. Checking each import throws when this domain
        // resolves it differently.
        await Promise.all(
          summary.moduleSpecifiers.map((specifier) =>
            this.resolveImport(host, specifier.source, source.resolverId),
          ),
        );
        if (revision !== this.#revision) {
          return;
        }
        record.verified.add(domain);
        continue;
      }
      const resolutions = await Promise.all(
        summary.moduleSpecifiers.map(async (specifier) => {
          const resolved = await this.resolveImport(host, specifier.source, source.resolverId);
          const target = analyzedTarget(host, resolved);
          if (target === undefined) {
            return undefined;
          }
          await this.#load(target, host, session, revision).catch(() => {});
          return {
            id: this.source(target)?.resolverId ?? target,
            start: specifier.start,
            end: specifier.end,
          };
        }),
      );
      if (revision !== this.#revision) {
        return;
      }
      record.linked = true;
      record.verified.add(domain);
      this.#setResolutions(
        record,
        resolutions.filter((value) => value !== undefined),
      );
    }
  }

  /**
   * Removes changed sources and clears all import links and cached resolution answers.
   *
   * @param ids - One changed resolver ID or several IDs. Omission removes every source.
   * Advances the revision so pending loads cannot restore the removed sources.
   */
  invalidate(ids?: string | Iterable<string>): void {
    this.#revision++;
    this.#facts?.clear();
    this.#imports?.clear();
    if (ids === undefined) {
      this.#sources.clear();
      this.#reload.clear();
    } else {
      for (const id of typeof ids === "string" ? [ids] : ids) {
        this.#sources.delete(sourceKey(id));
      }
    }
    for (const record of this.#sources.values()) {
      record.resolutions = [];
      if (record.checker) {
        record.checker.resolutions = record.resolutions;
      }
      record.linked = false;
      record.verified.clear();
    }
    this.#changed();
  }

  /**
   * Removes every source read from a file, including query-qualified sources, and clears every
   * collected fact. The next {@link prepare} call loads the removed sources again.
   *
   * A content change keeps the import links of other sources, because their resolved targets
   * keep the same IDs. Adding or removing a file can change any resolution, so it also clears
   * all import links and cached resolution answers.
   *
   * @param file - Absolute path of a file that changed, was added, or was removed. A file that
   * no source reads still clears the facts, because the checker reads unprepared files from disk.
   * @param entry - Whether the file was added or removed rather than changed.
   */
  invalidateFile(file: string, entry: boolean): void {
    const key = sourceKey(file);
    const removed = this.sourceKeys().filter((id) => id === key || id.startsWith(`${key}?`));
    for (const id of removed) {
      this.#reload.add(this.#sources.get(id)!.source.resolverId);
    }
    if (entry) {
      this.invalidate(removed);
      return;
    }
    this.#revision++;
    this.#facts?.clear();
    for (const importKey of this.#imports?.keys() ?? []) {
      const importer = sourceKey(importKey.slice(0, importKey.indexOf("\0")));
      if (importer === key || importer.startsWith(`${key}?`)) {
        this.#imports?.delete(importKey);
      }
    }
    for (const id of removed) {
      this.#sources.delete(id);
    }
    if (removed.length > 0) {
      this.#changed();
    }
  }

  /**
   * Installs prepared checker inputs when their sources, import links, or supplied files change.
   *
   * @param backend - Backend used by later queries. Receives inputs if it supports setProjectSources.
   * @param sources - Sources requested by the current analysis, with any known import resolutions.
   * @param inputs - File overlays and ambient declarations for these sources.
   * @returns Shared checker sources in the requested order. Do not mutate them.
   * @throws If a requested source's checker environment or backend installation fails.
   * Other sources with unavailable framework dependencies are left out of the installed inputs.
   */
  install(
    backend: TypeFactsBackend,
    sources: readonly TypeFactSource[],
    inputs?: SemanticInputs,
  ): TypeFactSource[] {
    const requested = sources.map((source) => {
      const record = this.#sources.get(sourceKey(source.resolverId));
      if (!record) {
        return source;
      }
      const checker = this.#checkerSource(record);
      if (!record.linked) {
        const resolutions = new Map(
          record.resolutions.map((item) => [`${item.start}:${item.end}`, item]),
        );
        for (const item of source.resolutions ?? []) {
          const target = this.source(item.id);
          if (target) {
            resolutions.set(`${item.start}:${item.end}`, { ...item, id: target.resolverId });
          }
        }
        this.#setResolutions(record, [...resolutions.values()]);
      }
      return checker;
    });
    const all = new Map<string, TypeFactSource>();
    for (const [id, record] of this.#sources) {
      try {
        all.set(id, this.#checkerSource(record));
      } catch {
        // Optional inventory entries must not require a framework checker environment for other targets.
      }
    }
    for (const source of requested) {
      all.set(sourceKey(source.resolverId), source);
    }
    if (
      this.#installed?.backend !== backend ||
      this.#installed.generation !== this.#generation ||
      !sameFiles(this.#installed.inputs?.overlayFiles, inputs?.overlayFiles) ||
      !sameFiles(this.#installed.inputs?.ambientDeclarations, inputs?.ambientDeclarations)
    ) {
      const inventory = [...all.values()].sort((a, b) => {
        const left = sourceKey(a.resolverId);
        const right = sourceKey(b.resolverId);
        return left < right ? -1 : left > right ? 1 : 0;
      });
      backend.setProjectSources?.(inventory, this.assets(), inputs);
      this.#installed = { backend, generation: this.#generation, inputs, sources: inventory };
      this.#resolutions = undefined;
    }
    return requested;
  }

  /**
   * @returns Declarations grouped by module ID. The arrays belong to the retained scans.
   */
  assets(): ReadonlyMap<string, readonly NativeAssetRecord[]> {
    return new Map(
      [...this.#sources.values()].map(({ source, summary }) => [source.moduleId, summary.assets]),
    );
  }

  /**
   * Resolves imports and caches the answers until checker inputs change.
   *
   * @param backend - Backend that resolves imports. Backends without setProjectSources query only sources.
   * @param sources - Sources requested by the current analysis, with any known import resolutions.
   * @param inputs - File overlays and ambient declarations for this analysis.
   * @returns Shared maps from resolver ID to authored specifier and resolved ID. Do not mutate them.
   * @throws If checker input preparation, installation, or import resolution fails.
   */
  async resolve(
    backend: TypeFactsBackend,
    sources: readonly TypeFactSource[],
    inputs?: SemanticInputs,
  ): Promise<ReadonlyMap<string, ReadonlyMap<string, string>>> {
    if (!backend.setProjectSources) {
      return backend.resolveModules(sources, this.assets(), inputs);
    }
    this.install(backend, sources, inputs);
    if (!this.#resolutions) {
      const generation = this.#generation;
      const resolutions = await backend.resolveModules(
        this.#installed!.sources,
        this.assets(),
        inputs,
      );
      if (generation === this.#generation) {
        this.#resolutions = resolutions;
      }
      return resolutions;
    }
    return this.#resolutions;
  }

  /**
   * Loads and scans one supported source unless it is already prepared.
   *
   * @param id - Full resolver ID.
   * @param host - Source loader and format metadata for this project.
   * @param session - Initialized session used to scan the source.
   * @param revision - Revision captured before loading. A newer revision discards this work.
   * @returns Completion of loading and scanning. Failed scans are not stored.
   * @throws If loading, projection, or native transport fails.
   */
  async #load(
    id: string,
    host: CompilerHost,
    session: NativeProjectSession,
    revision: number,
  ): Promise<void> {
    if (revision !== this.#revision || this.source(id) || sourceSupport(host, id) !== "source") {
      return;
    }
    const input = await loadSource(host, id);
    const source = "kind" in input ? input : await prepareSource(input);
    if (revision !== this.#revision) {
      return;
    }
    const scan = session.scanSource(source);
    if (scan.kind !== "failed") {
      this.retain(source, scan.summary, revision);
    }
  }

  /**
   * Creates a source's checker inputs once, loading any required framework declarations.
   *
   * @param record - Retained source and scan results to update.
   * @returns Shared checker inputs. Later import updates also update this object.
   * @throws If the framework checker environment cannot be prepared.
   */
  #checkerSource(record: CapturedSource): TypeFactSource {
    return (record.checker ??= {
      ...record.source,
      ...checkerEnvironment(record.source),
      probes: record.summary.typeProbes,
      checkerSpans: record.summary.checkerSpans,
      checkerBindings: record.summary.checkerBindings,
      resolutions: record.resolutions,
    });
  }

  /**
   * Replaces import links and clears cached answers when a target or range changes.
   *
   * @param record - Retained source whose links are being replaced.
   * @param resolutions - Resolved targets with original UTF-8 byte ranges.
   */
  #setResolutions(record: CapturedSource, resolutions: CapturedSource["resolutions"]): void {
    if (
      record.resolutions.length === resolutions.length &&
      record.resolutions.every(
        (item, index) =>
          item.id === resolutions[index].id &&
          item.start === resolutions[index].start &&
          item.end === resolutions[index].end,
      )
    ) {
      return;
    }
    record.resolutions = resolutions;
    if (record.checker) {
      record.checker.resolutions = resolutions;
    }
    this.#changed();
  }

  /**
   * Clears import answers and marks checker inputs for installation on the next query.
   */
  #changed(): void {
    this.#resolutions = undefined;
    this.#generation++;
  }
}

/**
 * Normalizes a resolver ID for map lookups.
 *
 * @param id - Resolver ID, with either slash style for Windows paths.
 * @returns Forward slashes and, for absolute Windows paths, lowercase text.
 */
export function sourceKey(id: string): string {
  const normalized = id.replaceAll("\\", "/");
  return /^[a-z]:\//i.test(normalized) ? normalized.toLowerCase() : normalized;
}

/**
 * Compares supplied file contents without depending on map order.
 *
 * @param a - First map of filenames to text. Omission means no files.
 * @param b - Second map, interpreted in the same way.
 * @returns Whether both maps contain the same filenames and text.
 */
function sameFiles(a?: ReadonlyMap<string, string>, b?: ReadonlyMap<string, string>): boolean {
  return (
    (a?.size ?? 0) === (b?.size ?? 0) && [...(a ?? [])].every(([id, text]) => b?.get(id) === text)
  );
}

/**
 * Selects the part of a host resolution that analysis reads.
 *
 * @param host - Host whose `supports` and `typeFactsResolution` settings apply.
 * @param resolved - Host resolution, or null when unresolved.
 * @returns The resolved identity when it is loaded as source. Undefined for unresolved, external,
 * declaration, package, and unsupported targets, whose types the checker resolves itself.
 */
function analyzedTarget(
  host: CompilerHost,
  resolved: Awaited<ReturnType<CompilerHost["resolve"]>>,
): string | undefined {
  return resolved && !resolved.external && sourceSupport(host, resolved.id) === "source"
    ? resolved.id
    : undefined;
}
