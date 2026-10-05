import { createHash } from "node:crypto";

import { cleanModuleId } from "@stillwind/host";

import { sourceModuleId } from "./module-ids.ts";
import type { StylesheetResource } from "./resources.ts";

type ModuleEntry =
  | {
      kind: "value" | "projection";
      sourceId: string;
    }
  | {
      kind: "producer";
      sourceId: string;
      moduleId: string;
    }
  | {
      kind: "fragment";
      css: string;
      resources: ReadonlyMap<string, StylesheetResource>;
    };

/** References issued by one project environment. Unknown IDs never supply file paths. */
export class ModuleRegistry {
  readonly #entries = new Map<string, ModuleEntry>();
  readonly #projections = new Map<string, Map<string, string>>();

  constructor(
    readonly root: string,
    readonly environment: string,
  ) {}

  #register(kind: string, key: string, entry: ModuleEntry): string {
    const token = createHash("sha256")
      .update(JSON.stringify([this.environment, kind, key]))
      .digest("hex");
    const id = `virtual:stillwind/${kind}:${token}`;
    this.#entries.set(id, entry);
    return id;
  }

  source(kind: "value" | "projection" | "producer", sourceId: string): string {
    const moduleId = sourceModuleId(this.root, sourceId);
    return this.#register(kind, moduleId, {
      kind,
      sourceId,
      moduleId,
    });
  }

  fragment(css: string, resources: ReadonlyMap<string, StylesheetResource>): string {
    const selected = new Map([...resources].filter(([placeholder]) => css.includes(placeholder)));
    return this.#register("fragment", JSON.stringify([css, [...selected]]), {
      kind: "fragment",
      css,
      resources: selected,
    });
  }

  get(id: string): ModuleEntry | undefined {
    return this.#entries.get(id.replace(/^\0stillwind:/, "virtual:stillwind/"));
  }

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

  projection(specifier: string, importer?: string): string | undefined {
    return importer === undefined ? undefined : this.#projections.get(importer)?.get(specifier);
  }

  remove(sourceId: string, file = false): void {
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

  clear(): void {
    this.#entries.clear();
    this.#projections.clear();
  }
}
