import type { NativeAssetRecord, NativeIdentity, NativeTypeFact } from "@stillwind/native";
import ts from "typescript-6";

import {
  TypeFactsError,
  type AvailableFacts,
  type SemanticInputs,
  type TypeFactsBackend,
  type TypeFactSource,
  type UnverifiedRange,
} from "../backend.ts";
import { checkerId, CheckerSource, withRequestedProbes } from "../checker-source.ts";
import { decode, producerArgumentDiagnostic, visit } from "./facts.ts";
import { BrowserProgram, type BrowserCheckerOptions } from "./program.ts";

/**
 * Answers declaration probes with the browser TypeScript checker.
 */
export class BrowserTypeFactsService implements TypeFactsBackend {
  /** Program and language service owned by this backend. */
  readonly #program: BrowserProgram;

  /** Prevents preparing further requests after cleanup. */
  #closed = false;

  /** Project inputs retained until {@link setProjectSources} replaces them. Queries select a subset. */
  #installed?: {
    sources: readonly TypeFactSource[];
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>;
    inputs: SemanticInputs;
  };

  /** Program and source mappings prepared from the installed project inputs. */
  #prepared?: {
    program: ts.Program;
    mappings: CheckerSource[];
  };

  /**
   * Creates a service without loading its sources.
   *
   * @param options - Virtual files and fixed TypeScript settings. Call invalidate after changing dependency files.
   */
  constructor(options: BrowserCheckerOptions) {
    this.#program = new BrowserProgram(options);
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
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs: SemanticInputs = {},
  ): void {
    this.#installed = {
      sources,
      assets,
      inputs,
    };
    this.#prepared = undefined;
  }

  /**
   * Collects finite declaration candidates and rejects unavailable required queries.
   *
   * @param sources - Prepared sources with probes in original UTF-8 coordinates.
   * @param assets - Declaration records grouped by compiler module identity.
   * @param inputs - Overlays and ambient declarations for an independent query. Omission supplies none.
   * An installed project uses the inputs supplied by {@link setProjectSources}.
   * @returns Facts keyed by compiler identity, deduplicated by original range across checker buffers.
   * Known non-asset types produce null facts. Unresolved recognition-only probes produce no fact.
   * @throws If the service is closed, preparation fails, or checker contracts cannot be verified.
   */
  collect(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ): Map<string, NativeTypeFact[]> {
    return this.#collect(sources, assets, inputs, false).facts;
  }

  /**
   * Collects usable facts while retaining unavailable original ranges for analysis.
   *
   * @param sources - Prepared sources with original UTF-8 probes and checker correspondences.
   * @param assets - Declaration records grouped by compiler module identity.
   * @param inputs - Overlays and ambient declarations for an independent query. Omission supplies none.
   * An installed project uses the inputs supplied by {@link setProjectSources}.
   * @returns Verified facts and unverified ranges without discarding other expressions' answers.
   * @throws If the service is closed, preparation fails, or checker results conflict for one original range.
   */
  collectAvailable(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ): AvailableFacts {
    return this.#collect(sources, assets, inputs, true);
  }

  /**
   * Resolves authored imports and exports from module symbols in the virtual checker.
   * When project sources are installed, resolves imports for the whole installed project.
   *
   * @param sources - Prepared sources whose checker buffers contain module requests.
   * @param assets - Declaration records used to prepare checker identities and helpers.
   * @param inputs - Overlays and ambient declarations for independent resolution. Omission supplies none.
   * An installed project uses the inputs supplied by {@link setProjectSources}.
   * @returns Maps from importing resolver identity to specifier and resolved identity. Unmatched requests are omitted.
   * @throws If the service is closed or checker preparation fails.
   */
  resolveModules(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ): Map<string, Map<string, string>> {
    const { program, mappings } = this.#prepare(sources, assets, inputs);

    const result = new Map<string, Map<string, string>>();
    const checker = program.getTypeChecker();
    for (const mapping of mappings) {
      const file = program.getSourceFile(mapping.id);
      if (!file) {
        continue;
      }

      const resolutions = new Map<string, string>();
      result.set(mapping.source.resolverId, resolutions);
      visit(file, (node) => {
        if (
          !(ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) ||
          !node.moduleSpecifier ||
          !ts.isStringLiteral(node.moduleSpecifier)
        ) {
          return;
        }
        const symbol = checker.getSymbolAtLocation(node.moduleSpecifier);
        const declaration = symbol?.declarations?.find(
          (item) => ts.isSourceFile(item) || item.getSourceFile().isDeclarationFile,
        );
        if (declaration) {
          const filename = declaration.getSourceFile().fileName;
          resolutions.set(
            mapping.moduleSpecifiers.get(
              `${node.moduleSpecifier.getStart(file)}:${node.moduleSpecifier.end}`,
            ) ?? node.moduleSpecifier.text,
            (this.#installed?.sources ?? sources).find((source) => checkerId(source) === filename)
              ?.resolverId ?? filename,
          );
        }
      });
    }

    return result;
  }

  /**
   * Discards the language service after file or configuration changes.
   */
  invalidate(): void {
    this.#program.invalidate();
    this.#prepared = undefined;
  }

  /**
   * Releases the checker. Subsequent queries reject.
   */
  close(): void {
    this.#closed = true;
    this.invalidate();
  }

  /**
   * Reuses prepared project inputs or prepares only the requested sources if project preparation fails.
   *
   * @param sources - Requested sources to use when no project is installed or preparation fails.
   * @param assets - Declarations grouped by module ID for those sources.
   * @param inputs - Overlays and ambient declarations for the requested sources.
   * @returns The current checker program and source mappings. Successful project preparation is cached.
   * @throws If the service is closed or the requested sources cannot be prepared.
   */
  #prepare(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs: SemanticInputs = {},
  ): { program: ts.Program; mappings: CheckerSource[] } {
    if (this.#closed) {
      throw new Error("Browser checker is closed");
    }

    if (this.#prepared) {
      return this.#prepared;
    }
    if (this.#installed) {
      try {
        this.#prepared = this.#program.prepare(
          this.#installed.sources,
          this.#installed.assets,
          this.#installed.inputs,
        );
        return this.#prepared;
      } catch {
        // Unrelated invalid checker mappings must not reject a scoped query.
      }
    }
    return this.#program.prepare(sources, assets, inputs);
  }

  /**
   * Decodes current checker expressions and enforces finite declaration identities for probes.
   *
   * @param sources - Prepared sources with validated checker correspondences.
   * @param assets - Native declarations keyed by compiler identity.
   * @param inputs - Overlays and ambient declarations for this snapshot.
   * @param available - Whether unavailable queries are recorded instead of thrown.
   * @returns Facts grouped by compiler identity and original UTF-8 ranges that could not be verified.
   * @throws If preparation fails, checker results conflict, or strict mode encounters unavailable facts.
   */
  #collect(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs: SemanticInputs | undefined,
    available: boolean,
  ): AvailableFacts {
    if (this.#installed) {
      const updated = withRequestedProbes(this.#installed.sources, sources);
      if (updated !== this.#installed.sources) {
        this.#installed = { ...this.#installed, sources: updated };
        this.#prepared = undefined;
      }
    }
    const prepared = this.#prepare(sources, assets, inputs);
    const { program } = prepared;
    const selected = new Set(sources.map((source) => source.resolverId));
    const mappings = prepared.mappings.filter((mapping) => selected.has(mapping.source.resolverId));
    const identities = new Map(mappings.flatMap((mapping) => [...mapping.identities]));
    const checker = program.getTypeChecker();
    const facts = new Map<string, NativeTypeFact[]>();
    const unverified: UnverifiedRange[] = [];
    const brands = new Map<string, NativeIdentity["kind"]>();
    const producers: ts.Type[] = [];
    for (const mapping of mappings) {
      const file = program.getSourceFile(mapping.id);
      if (!file) {
        continue;
      }
      visit(file, (node) => {
        const kind = mapping.producers.get(`${node.getStart(file)}:${node.end}`);
        if (!kind) {
          return;
        }
        producers.push(checker.getTypeAtLocation(node));
        if (kind === "style") {
          return;
        }
        const prefix = {
          ref: "refIdentity",
          keyframes: "keyframesIdentity",
          viewTransition: "viewTransitionIdentity",
        }[kind];
        const property = checker
          .getPropertiesOfType(checker.getTypeAtLocation(node))
          .find((item) => item.name.startsWith(`__@${prefix}@`));
        if (property) {
          brands.set(property.name, kind);
        }
      });
    }
    /**
     * Records an unverifiable use when partial facts are allowed.
     *
     * @param error - Failure with the original module identity and UTF-8 range, when available.
     * @throws The supplied error when this collection requires complete facts.
     */
    const unavailable = (error: TypeFactsError) => {
      if (!available) {
        throw error;
      }
      unverified.push({
        moduleId: error.moduleId,
        start: error.start,
        end: error.end,
        reason: error.message,
      });
    };
    for (const mapping of mappings) {
      const file = program.getSourceFile(mapping.id);
      if (!file) {
        throw new Error(`Browser checker omitted ${mapping.id}`);
      }
      const diagnostic = [
        ...program.getSyntacticDiagnostics(file),
        ...program.getSemanticDiagnostics(file),
      ].find(
        (item) =>
          !producerArgumentDiagnostic(item, file, mapping) &&
          /stillwind\/v1:|StyleResult|StyleRef|StyleKeyframes|StyleViewTransition|__stillwindDeclarationIdentity|data-__stillwindStyleIdentity/.test(
            ts.flattenDiagnosticMessageText(item.messageText, "\n"),
          ),
      );
      if (diagnostic) {
        const start = mapping.originalOffset(diagnostic.start ?? 0) ?? undefined;
        const end =
          mapping.originalOffset((diagnostic.start ?? 0) + (diagnostic.length ?? 0), true) ??
          undefined;
        unavailable(
          new TypeFactsError(
            mapping.source.moduleId,
            start,
            end,
            "assignment",
            `TS${diagnostic.code}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n")} Preserve declaration candidates with an explicit union or a generic parameter.`,
          ),
        );
      }
      const matched = new Set<string>();
      const values = facts.get(mapping.source.moduleId) ?? [];
      visit(file, (node) => {
        if (!ts.isExpression(node)) {
          return;
        }
        const key = `${node.getStart(file)}:${node.end}`;
        const probe = mapping.probes.get(key);
        if (!probe) {
          return;
        }
        matched.add(key);
        const type = checker.getTypeAtLocation(node);
        const decoded = decode(
          checker,
          type,
          node,
          identities,
          brands,
          producers,
          probe.recognitionOnly ?? false,
        );
        if (
          decoded.kind === "asset" ||
          decoded.kind === "nonAsset" ||
          (decoded.kind === "unbranded" && probe.recognitionOnly)
        ) {
          const value = decoded.kind === "asset" ? decoded.value : null;
          const existing = values.find(
            (item) => item.start === probe.start && item.end === probe.end,
          );
          if (existing && JSON.stringify(existing.value) !== JSON.stringify(value)) {
            throw new TypeFactsError(
              mapping.source.moduleId,
              probe.start,
              probe.end,
              "checker",
              "Ambiguous checker results for this original expression",
            );
          }
          if (!existing) {
            values.push({
              ...probe,
              value,
            });
          }
        } else if (!probe.recognitionOnly) {
          unavailable(
            new TypeFactsError(
              mapping.source.moduleId,
              probe.start,
              probe.end,
              "identity",
              "Cannot determine a finite set of declaration identities. Preserve the value's inferred type, use an explicit union, or use a generic parameter.",
            ),
          );
        }
      });
      for (const [key, probe] of mapping.probes) {
        if (!matched.has(key)) {
          unavailable(
            new TypeFactsError(
              mapping.source.moduleId,
              probe.start,
              probe.end,
              "checker",
              "Projected expression has no matching checker AST node",
            ),
          );
        }
      }
      facts.set(mapping.source.moduleId, values);
    }
    return {
      facts,
      unverified,
    };
  }
}
