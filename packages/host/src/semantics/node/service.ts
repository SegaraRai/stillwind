import type { NativeAssetRecord, NativeTypeFact } from "@stillwind/native";
import type { Node } from "typescript-7/unstable/ast";
import {
  isCallExpression,
  isExportDeclaration,
  isImportDeclaration,
  isSourceFile,
  isStringLiteral,
} from "typescript-7/unstable/ast/is";

import {
  TypeFactsError,
  type AvailableFacts,
  type TypeFactSource,
  type TypeFactsBackend,
  type UnverifiedRange,
} from "../backend.ts";
import { checkerId, withRequestedProbes } from "../checker-source.ts";
import { NodeChecker, type TypeScriptFactsStats } from "./checker.ts";
import { AssetTypeDecoder } from "./decoder.ts";
import { visit, diagnosticText, producerArgumentDiagnostic, factsForSource } from "./facts.ts";
import {
  normalize,
  type PreparedProject,
  type TypeFactsEnvironment,
  type TypeFactsInputs,
} from "./project.ts";

/**
 * Native declaration record carrying an identity, metadata, and the original-byte range of its
 * producer call.
 */
export type AssetRecord = NativeAssetRecord;

/** Semantic value attached to an original UTF-8 expression range for native lowering. */
export type TypeFact = NativeTypeFact;

/**
 * Collects declaration facts with a reusable Node checker. Close the service after pending queries finish.
 */
export class TypeScriptFactsService implements TypeFactsBackend {
  /** Project inputs retained until setProjectSources replaces them. Queries select a subset. */
  #installed?: {
    sources: readonly TypeFactSource[];
    assets: ReadonlyMap<string, readonly AssetRecord[]>;
    inputs: TypeFactsInputs;
  };

  /** Checker buffers prepared from the installed project inputs. */
  #prepared?: PreparedProject;

  /**
   * Whether close has been requested. Subsequent collections reject and repeated close calls do
   * nothing.
   */
  #closed = false;

  /** Checker process and snapshot caches owned by this service. */
  readonly #checker: NodeChecker;

  /** Paths observed by disk reads and existence checks, including missing paths and overlay checks. */
  readonly dependencies: Set<string>;

  /**
   * Live cumulative counters updated by this service. Consumers needing a stable report must copy
   * their values.
   */
  readonly stats: TypeScriptFactsStats;

  /**
   * Creates a service and starts its checker process without opening a snapshot.
   *
   * @param environment - Fixed project root and TypeScript settings.
   * @throws If the checker process cannot start.
   */
  constructor(environment: TypeFactsEnvironment) {
    this.#checker = new NodeChecker(environment);
    this.dependencies = this.#checker.dependencies;
    this.stats = this.#checker.stats;
  }

  /**
   * Keeps project inputs for later queries and clears prepared checker buffers.
   *
   * @param sources - Prepared project sources, retained by reference until replaced.
   * @param assets - Declarations grouped by compiler module ID, retained with the sources.
   * @param inputs - File overlays and ambient declarations. Omission supplies no extra files.
   * Do not mutate these inputs while they are installed. Queries select their own probes and sources.
   */
  setProjectSources(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs = {},
  ): void {
    this.#installed = { sources, assets, inputs };
    this.#prepared = undefined;
  }

  /**
   * Collects finite declaration candidates for original-coordinate probes.
   *
   * Queries reuse the current snapshot when files have not changed. Mapping, assignment, and
   * identity errors throw TypeFactsError. Other checker failures restart the API and retry once.
   * Only diagnostics about Stillwind declaration contracts affect the result.
   *
   * @param sources - Prepared sources and original-byte probes, with validated projection
   * correspondences where needed.
   * @param assetsByModuleId - Declaration records keyed by compiler module identity.
   * @param inputs - Overlays and ambient declarations for an independent query. Omission supplies none.
   * When project sources are installed, preparation uses their installed inputs instead.
   * @returns Facts keyed by compiler module identity. Facts for the same original range are
   * deduplicated across buffers. Conflicting values throw. Known non-asset types produce null facts.
   * Unresolved recognition-only probes produce no fact.
   * @throws If the service is closed, preparation or checker contracts fail, or the API fails. A
   * failed query retry throws an AggregateError containing both query failures.
   */
  collect(
    sources: readonly TypeFactSource[],
    assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs = {},
  ): Map<string, TypeFact[]> {
    return this.#collect(sources, assetsByModuleId, inputs, false).facts;
  }

  /**
   * Collects verified facts, reusing the snapshot when files have not changed.
   *
   * @param sources - Prepared sources and original UTF-8 probes with validated checker correspondences.
   * @param assetsByModuleId - Declaration records grouped by compiler module identity.
   * @param inputs - Overlays and ambient declarations for an independent query. Omission supplies none.
   * When project sources are installed, preparation uses their installed inputs instead.
   * @returns Verified facts and unavailable original ranges. Unavailable queries do not discard
   * facts collected for other expressions.
   * @throws If the service is closed, project preparation fails, facts conflict, or checker restart and retry fail.
   */
  collectAvailable(
    sources: readonly TypeFactSource[],
    assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs = {},
  ): AvailableFacts {
    return this.#collect(sources, assetsByModuleId, inputs, true);
  }

  /**
   * Resolves authored imports and exports using module symbols in the selected TypeScript settings.
   *
   * @param sources - Prepared sources whose checker buffers contain the authored module requests.
   * Independent resolution ignores probes. An installed project also prepares its probes for later queries.
   * @param assetsByModuleId - Declaration records used to expose generated helper types.
   * @param inputs - Overlays and ambient declarations for independent resolution. Omission supplies none.
   * An installed project uses the inputs supplied by setProjectSources.
   * @returns Request-to-identity maps grouped by source resolver identity. Prepared checker paths
   * map back to resolver identities. Other resolutions retain checker filenames, including declarations.
   * Requests without a matching module symbol are omitted.
   * @throws If the service is closed, project preparation fails, or checker resolution fails.
   */
  resolveModules(
    sources: readonly TypeFactSource[],
    assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs = {},
  ): Map<string, Map<string, string>> {
    if (this.#closed) {
      throw new Error("TypeScript facts service is closed");
    }
    const inventory = this.#installed?.sources ?? sources;
    const prepared = this.#prepare(sources, assetsByModuleId, inputs, true);
    const project = this.#checker.updateProject(prepared);
    const result = new Map<string, Map<string, string>>();
    for (const mapping of prepared.mappings) {
      const file = project.program.getSourceFile(mapping.id);
      if (!file) {
        continue;
      }
      const resolutions = result.get(mapping.source.resolverId) ?? new Map<string, string>();
      result.set(mapping.source.resolverId, resolutions);
      visit(file, (node) => {
        if (
          !(isImportDeclaration(node) || isExportDeclaration(node)) ||
          !node.moduleSpecifier ||
          !isStringLiteral(node.moduleSpecifier)
        ) {
          return;
        }
        const symbol = project.checker.getSymbolAtLocation(node.moduleSpecifier);
        const declaration = symbol?.declarations.find((declaration) => {
          const node = declaration.resolve(project);
          return (
            node !== undefined && (isSourceFile(node) || node.getSourceFile().isDeclarationFile)
          );
        });
        if (declaration) {
          const fileName = declaration.resolve(project)!.getSourceFile().fileName;
          const original = inventory.find(
            (source) => normalize(checkerId(source)) === normalize(fileName),
          );
          const key = `${node.moduleSpecifier.getStart(file)}:${node.moduleSpecifier.end}`;
          resolutions.set(
            mapping.moduleSpecifiers.get(key) ?? node.moduleSpecifier.text,
            original?.resolverId ?? fileName,
          );
        }
      });
    }
    return result;
  }

  /**
   * Prepares one request and retries an unexpected checker failure once with a clean API.
   *
   * @param sources - Prepared sources and original UTF-8 probes.
   * @param assetsByModuleId - Declaration records grouped by compiler identity.
   * @param inputs - Complete overlays and ambient declarations for the request.
   * @param available - Whether unavailable facts are recorded instead of rejecting the request.
   * @returns Collected facts and unavailable ranges from the successful attempt.
   * @throws If preparation fails, a TypeFactsError occurs, restart fails, or both query attempts fail.
   */
  #collect(
    sources: readonly TypeFactSource[],
    assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs,
    available: boolean,
  ): AvailableFacts {
    if (this.#closed) {
      throw new Error("TypeScript facts service is closed");
    }
    if (this.#installed) {
      const updated = withRequestedProbes(this.#installed.sources, sources);
      if (updated !== this.#installed.sources) {
        this.#installed = { ...this.#installed, sources: updated };
        this.#prepared = undefined;
      }
    }
    const full = this.#prepare(sources, assetsByModuleId, inputs);
    const selected = new Set(sources.map((source) => source.resolverId));
    const mappings = full.mappings.filter((mapping) => selected.has(mapping.source.resolverId));
    const prepared = {
      ...full,
      mappings,
      identities: new Map(mappings.flatMap((mapping) => [...mapping.identities])),
    };
    try {
      return this.#collectPrepared(prepared, available);
    } catch (firstError) {
      if (firstError instanceof TypeFactsError) {
        throw firstError;
      }
      this.stats.restarts += 1;
      this.#restartApi();
      try {
        return this.#collectPrepared(prepared, available);
      } catch (retryError) {
        throw new AggregateError(
          [firstError, retryError],
          `TypeScript 7 checker failed before and after a clean restart: ${String(retryError)}`,
        );
      }
    }
  }

  /**
   * Reuses prepared project inputs or prepares only the requested sources if project preparation fails.
   *
   * @param sources - Requested sources to use when no project is installed or preparation fails.
   * @param assets - Declarations grouped by module ID for those sources.
   * @param inputs - Overlays and ambient declarations for the requested sources.
   * @param resolutionOnly - True omits probes when preparing only the requested sources for import resolution.
   * @returns Prepared checker files and mappings. Successful project preparation is cached.
   * @throws If the requested sources or their TypeScript configuration cannot be prepared.
   */
  #prepare(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs,
    resolutionOnly = false,
  ): PreparedProject {
    if (this.#installed && this.#prepared) {
      return this.#prepared;
    }
    if (this.#installed) {
      try {
        this.#prepared = this.#checker.prepareProject(
          this.#installed.sources,
          this.#installed.assets,
          this.#installed.inputs,
        );
        return this.#prepared;
      } catch {
        // Unrelated invalid checker mappings must not reject a scoped query.
      }
    }
    return this.#checker.prepareProject(
      resolutionOnly ? sources.map((source) => ({ ...source, probes: [] })) : sources,
      assets,
      inputs,
    );
  }

  /**
   * Marks the service closed, disposes its retained snapshot, and closes the checker API. Repeated
   * calls return without further work.
   *
   * @throws If snapshot disposal or API cleanup fails.
   */
  close(): void {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    this.#prepared = undefined;
    this.#checker.close();
  }

  /**
   * Discards cached disk contents and the retained snapshot while keeping the selected environment.
   * The next collection opens a new project. Calls after close have no effect.
   *
   * @throws If checker cleanup or restart fails.
   */
  invalidate(): void {
    if (!this.#closed) {
      this.#restartApi();
    }
  }

  /**
   * Updates the snapshot and collects facts from the prepared checker buffers.
   *
   * @param prepared - Virtual files, original-coordinate mappings, and serialized declaration identities.
   * @param available - Whether contract failures become unverified ranges where recovery is possible.
   * @returns Deduplicated facts by compiler identity and unverified original UTF-8 ranges.
   * @throws If snapshot update fails, a checker file is missing, facts conflict, or strict collection fails.
   */
  #collectPrepared(prepared: PreparedProject, available: boolean): AvailableFacts {
    const project = this.#checker.updateProject(prepared);
    const unverified: UnverifiedRange[] = [];
    const result = new Map<string, TypeFact[]>();
    const decoder = new AssetTypeDecoder(
      project.checker,
      prepared.identities,
      this.#checker.properties,
    );
    const producers: Node[] = [];
    for (const mapping of prepared.mappings) {
      const file = project.program.getSourceFile(mapping.id);
      if (!file) {
        continue;
      }
      producers.push(
        ...this.#checker
          .expressions(file)
          .filter(
            (node) =>
              isCallExpression(node) && mapping.producers.has(`${node.getStart(file)}:${node.end}`),
          ),
      );
    }
    for (const type of this.#checker.types(project.checker, producers)) {
      if (type) {
        decoder.register(type);
      }
    }
    for (const mapping of prepared.mappings) {
      const sourceFile = project.program.getSourceFile(mapping.id);
      if (!sourceFile) {
        throw new Error(`TypeScript 7 omitted ${mapping.id}`);
      }
      let diagnostics = this.#checker.diagnostics.get(mapping.id);
      if (!diagnostics) {
        diagnostics = [
          ...project.program.getSyntacticDiagnostics(mapping.id),
          ...project.program.getSemanticDiagnostics(mapping.id),
        ];
        this.#checker.diagnostics.set(mapping.id, diagnostics);
      }
      const diagnostic = diagnostics.find(
        (d) =>
          !producerArgumentDiagnostic(d, sourceFile, mapping) &&
          /stillwind\/v1:|StyleResult|StyleRef|StyleKeyframes|StyleViewTransition|__stillwindDeclarationIdentity|data-__stillwindStyleIdentity/.test(
            diagnosticText(d),
          ),
      );
      if (diagnostic) {
        const start = mapping.originalOffset(diagnostic.pos) ?? undefined;
        const end = mapping.originalOffset(diagnostic.end, true) ?? undefined;
        const message = diagnosticText(diagnostic).replace(
          /stillwind\/v1:[A-Za-z0-9_-]+/g,
          "declaration identity",
        );
        const error = new TypeFactsError(
          mapping.source.moduleId,
          start,
          end,
          "assignment",
          `TS${diagnostic.code}: ${message} Preserve declaration candidates with an explicit union or a generic parameter.`,
        );
        if (!available) {
          throw error;
        }
        unverified.push({ moduleId: error.moduleId, start, end, reason: error.message });
      }
      const facts = factsForSource(
        sourceFile,
        mapping,
        decoder,
        this.#checker.expressions(sourceFile),
        (nodes) => this.#checker.types(project.checker, nodes),
        available ? unverified : undefined,
      );
      const previous = result.get(mapping.source.moduleId) ?? [];
      for (const fact of facts) {
        const existing = previous.find(
          (value) => value.start === fact.start && value.end === fact.end,
        );
        if (existing && JSON.stringify(existing.value) !== JSON.stringify(fact.value)) {
          throw new TypeFactsError(
            mapping.source.moduleId,
            fact.start,
            fact.end,
            "checker",
            "Ambiguous checker results for this original expression",
          );
        }
        if (!existing) {
          previous.push(fact);
        }
      }
      result.set(mapping.source.moduleId, previous);
    }
    return {
      facts: result,
      unverified,
    };
  }

  /**
   * Clears prepared inputs and replaces the checker process after invalidation or query failure.
   */
  #restartApi(): void {
    this.#prepared = undefined;
    this.#checker.restart();
  }
}

/**
 * Collects facts with a temporary checker service and closes it afterward.
 *
 * @param sources - Prepared sources and their original-byte semantic probes.
 * @param assetsByModuleId - Native declarations keyed by compiler module identity.
 * @param environment - Fixed filesystem root and TypeScript settings for the temporary checker.
 * @param inputs - File replacements and declarations required by this collection.
 * @returns Facts keyed by compiler module identity. The temporary checker is closed before returning.
 * @throws If service startup, fact collection, or cleanup fails.
 */
export function collectTypeScriptFacts(
  sources: readonly TypeFactSource[],
  assetsByModuleId: ReadonlyMap<string, readonly AssetRecord[]>,
  environment: TypeFactsEnvironment,
  inputs: TypeFactsInputs = {},
): Map<string, TypeFact[]> {
  const service = new TypeScriptFactsService(environment);
  try {
    return service.collect(sources, assetsByModuleId, inputs);
  } finally {
    service.close();
  }
}
