import { createHash } from "node:crypto";

import { cleanModuleId } from "@stillwind/host";

import { sourceModuleId } from "./module-ids.ts";
import type { StylesheetResource } from "./resources.ts";

/** Payload for an issued virtual reference in one compiler environment. */
type ModuleEntry =
  | {
      /** Selects explicit CSS values or source projections that compile CSS without application execution. */
      kind: "value" | "projection";
      /** Full source resolver identity, including any query selecting distinct source. */
      sourceId: string;
    }
  | {
      /** Selects the runtime registration module for an automatic CSS producer. */
      kind: "producer";
      /** Full source resolver identity. */
      sourceId: string;
      /** Compiler identity used for stylesheet activation and composition dependencies. */
      moduleId: string;
    }
  | {
      /** Selects a generated module exporting one stylesheet fragment. */
      kind: "fragment";
      /** Fragment CSS containing the recorded resource placeholders. */
      css: string;
      /** Placeholder records selected because they occur in this fragment. */
      resources: ReadonlyMap<string, StylesheetResource>;
    };

/** References issued by one project environment. Unknown IDs never supply file paths. */
export class ModuleRegistry {
  /** Issued virtual identities mapped to their payloads. References are not interpreted as paths. */
  readonly #entries = new Map<string, ModuleEntry>();
  /** Projection imports keyed by full importer identity and authored specifier. */
  readonly #projections = new Map<string, Map<string, string>>();

  /**
   * Creates an empty registry for one project environment.
   * @param root - Project root used to derive compiler identities from source resolver identities.
   * @param environment - Environment name included in every issued reference's fingerprint.
   */
  constructor(
    readonly root: string,
    readonly environment: string,
  ) {}

  /**
   * Issues a deterministic reference and replaces its retained payload.
   * @param kind - Reference family included in the virtual prefix and fingerprint.
   * @param key - Source identity or serialized fragment content used within that family.
   * @param entry - Payload retained by reference under the resulting identity.
   * @returns A virtual:stillwind reference containing a SHA-256 token scoped to the environment.
   */
  #register(kind: string, key: string, entry: ModuleEntry): string {
    const token = createHash("sha256")
      .update(JSON.stringify([this.environment, kind, key]))
      .digest("hex");
    const id = `virtual:stillwind/${kind}:${token}`;
    this.#entries.set(id, entry);
    return id;
  }

  /**
   * Registers a source-backed generated module.
   * @param kind - CSS value, projection, or producer registration module family.
   * @param sourceId - Full resolver identity, including queries that select distinct source.
   * @returns A deterministic reference for that environment, family, and compiler module identity.
   */
  source(kind: "value" | "projection" | "producer", sourceId: string): string {
    const moduleId = sourceModuleId(this.root, sourceId);
    return this.#register(kind, moduleId, {
      kind,
      sourceId,
      moduleId,
    });
  }

  /**
   * Registers CSS with only the resource records whose placeholders occur in it.
   * @param css - Generated fragment text before resource replacement.
   * @param resources - Available placeholder records. The supplied map is unchanged.
   * @returns A reference fingerprinted from CSS and selected records in map order.
   */
  fragment(css: string, resources: ReadonlyMap<string, StylesheetResource>): string {
    const selected = new Map([...resources].filter(([placeholder]) => css.includes(placeholder)));
    return this.#register("fragment", JSON.stringify([css, [...selected]]), {
      kind: "fragment",
      css,
      resources: selected,
    });
  }

  /**
   * Looks up an issued public or resolved virtual reference.
   * @param id - virtual:stillwind identity or its NUL-prefixed resolved spelling.
   * @returns The retained payload, or undefined for an unregistered identity.
   */
  get(id: string): ModuleEntry | undefined {
    return this.#entries.get(id.replace(/^\0stillwind:/, "virtual:stillwind/"));
  }

  /**
   * Replaces an importer's projection edges and removes previous references with no remaining owner.
   * @param importer - Full resolver identity of the original importing document.
   * @param sources - Authored projection specifiers mapped to resolved source identities. The map is copied.
   */
  projections(importer: string, sources: ReadonlyMap<string, string>): void {
    const previous = this.#projections.get(importer);
    this.#projections.set(
      importer,
      new Map(
        [...sources].map(([specifier, source]) => [specifier, this.source("projection", source)]),
      ),
    );
    for (const reference of previous?.values() ?? []) {
      if (
        ![...this.#projections.values()].some((edges) => [...edges.values()].includes(reference))
      ) {
        this.#entries.delete(reference);
      }
    }
  }

  /**
   * Resolves a projection only through an importer's previously registered edge.
   * @param specifier - Authored projection request, including ?stillwind.css.
   * @param importer - Full importer identity. Omission cannot select an edge.
   * @returns The issued reference, or undefined when no corresponding edge exists.
   */
  projection(specifier: string, importer?: string): string | undefined {
    return importer === undefined ? undefined : this.#projections.get(importer)?.get(specifier);
  }

  /**
   * Removes source-backed references and projection edges after source removal.
   * @param sourceId - Full resolver identity to remove.
   * @param file - Whether to match all queries and fragments of the cleaned file identity.
   */
  remove(sourceId: string, file = false): void {
    /**
     * Applies the selected source-removal policy to a registered identity.
     * @param id - Full resolver identity from a source entry or projection importer.
     * @returns Whether the exact identity, or its cleaned file identity when requested, matches.
     */
    const matches = (id: string) =>
      file ? cleanModuleId(id) === cleanModuleId(sourceId) : id === sourceId;
    for (const [id, entry] of this.#entries) {
      if ("sourceId" in entry && matches(entry.sourceId)) {
        this.#entries.delete(id);
      }
    }
    for (const [importer, edges] of this.#projections) {
      if (matches(importer)) {
        this.#projections.delete(importer);
      } else {
        for (const [specifier, target] of edges) {
          if (!this.#entries.has(target)) {
            edges.delete(specifier);
          }
        }
      }
    }
  }

  /** Removes every issued reference and projection edge when the environment is reset. */
  clear(): void {
    this.#entries.clear();
    this.#projections.clear();
  }
}
