import { existsSync } from "node:fs";
import path from "node:path";

import type { NativeAssetRecord as AssetRecord } from "@stillwind/native";
import type { Node, SourceFile } from "typescript-7/unstable/ast";
import { isExpression } from "typescript-7/unstable/ast/is";
import {
  API,
  Snapshot,
  type Checker,
  type Diagnostic,
  type Type,
} from "typescript-7/unstable/sync";

import type { TypeFactSource } from "../backend.ts";
import type { TypePropertyCache } from "./decoder.ts";
import { visit } from "./facts.ts";
import { withHiddenSpawnWindows } from "./hidden-spawn.ts";
import {
  normalize,
  prepareProject,
  type PreparedProject,
  type TypeFactsEnvironment,
  type TypeFactsInputs,
  type TypeScriptProject,
} from "./project.ts";

/**
 * Cumulative checker lifecycle and filesystem counters for one service. They include recovery
 * attempts and are not reset between collections.
 */
export interface TypeScriptFactsStats {
  /** Number of API construction attempts, including starts after invalidation or query failure. */
  starts: number;
  /**
   * Number of snapshot updates that returned a snapshot, including updates followed by a query
   * failure.
   */
  snapshots: number;
  /** Number of snapshot updates made while a previous snapshot was retained. */
  incrementalSnapshots: number;
  /** Number of recovery restarts attempted after a failure other than `TypeFactsError`. */
  restarts: number;
  /** Number of checker read callbacks answered with text from the virtual overlay. */
  overlayReads: number;
  /**
   * Number of checker read callbacks delegated to the filesystem because the overlay had no text. A
   * request does not imply a successful disk read.
   */
  diskReadRequests: number;
}

/**
 * Owns the Node checker process, snapshots, and answers cached within each snapshot.
 * The analysis service serializes calls and closes the checker after pending work finishes.
 */
export class NodeChecker {
  /** Property answers for the current snapshot. Cleared before replacing that snapshot. */
  readonly properties: TypePropertyCache = {
    named: new Map(),
    all: new Map(),
  };

  /** Types keyed by filename and AST range within the current snapshot. */
  readonly #queryTypes = new Map<string, Type | undefined>();

  /** Expression nodes in AST traversal order, keyed by checker filename. */
  readonly #queryNodes = new Map<string, Node[]>();

  /** Syntactic and semantic diagnostics for each checker file in the current snapshot. */
  readonly diagnostics = new Map<string, readonly Diagnostic[]>();

  /** Paths observed by disk reads and existence checks, including missing paths and overlay checks. */
  readonly dependencies = new Set<string>();

  /**
   * Live cumulative counters updated by this service. Consumers needing a stable report must copy
   * their values.
   */
  readonly stats: TypeScriptFactsStats = {
    starts: 0,
    snapshots: 0,
    incrementalSnapshots: 0,
    restarts: 0,
    overlayReads: 0,
    diskReadRequests: 0,
  };

  /** Copy of the selected TypeScript settings, fixed for this service's lifetime. */
  readonly #project: TypeScriptProject;

  /** Filesystem root supplied as the checker API working directory. */
  readonly #projectRoot: string;

  /**
   * Absolute filename of the in-memory `.stillwind-tsconfig.json`. The service does not write it to
   * disk.
   */
  readonly #shadowTsconfigPath: string;

  /** Active checker API instance, replaced after a recoverable collection failure. */
  #api: API;

  /**
   * Latest retained checker snapshot, also retained after query failures until the next update,
   * restart, or close.
   */
  #snapshot: Snapshot | undefined;

  /**
   * Current normalized filename overlay read dynamically by the checker API filesystem callbacks.
   */
  #virtualFiles = new Map<string, string>();

  /**
   * Starts a checker API for a project root without creating its first snapshot.
   *
   * @param environment - Absolute filesystem root and selected TypeScript settings. The checker
   * copies these settings and uses them for its lifetime. Create a new checker to change them.
   * @throws If the checker API cannot start.
   */
  constructor(environment: TypeFactsEnvironment) {
    this.#project = { ...environment.project };
    this.#projectRoot = environment.root;
    this.#shadowTsconfigPath = path.join(environment.root, ".stillwind-tsconfig.json");
    this.#api = this.#startApi();
  }

  /**
   * Builds virtual checker files using the project's TypeScript settings.
   *
   * @param sources - Prepared sources and probes for this request.
   * @param assets - Declarations grouped by compiler module ID.
   * @param inputs - Overlays and ambient declarations used for config reads and checker files.
   * @returns Virtual files, checker mappings, and declaration identities for the supplied sources.
   * @throws If configuration parsing, checker mapping, or declaration preparation fails.
   */
  prepareProject(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly AssetRecord[]>,
    inputs: TypeFactsInputs,
  ): PreparedProject {
    return prepareProject(
      sources,
      assets,
      this.#shadowTsconfigPath,
      this.#project,
      inputs,
      this.#configuredDeclarations(inputs),
    );
  }

  /**
   * Queries types once per expression in the current snapshot.
   *
   * @param checker - Checker that owns the current snapshot.
   * @param nodes - Expression nodes from that snapshot, in the requested order.
   * @returns Types in the same order. Undefined records a missing type. Answers are cached until the snapshot changes.
   * @throws If the checker query fails.
   */
  types(checker: Checker, nodes: readonly Node[]): (Type | undefined)[] {
    const keys = nodes.map((node) => `${node.getSourceFile().fileName}:${node.pos}:${node.end}`);
    const missing = new Map<string, Node>();
    for (const [index, key] of keys.entries()) {
      if (!this.#queryTypes.has(key)) {
        missing.set(key, nodes[index]);
      }
    }
    if (missing.size) {
      const values = checker.getTypeAtLocation([...missing.values()]);
      [...missing.keys()].forEach((key, index) => this.#queryTypes.set(key, values[index]));
    }
    return keys.map((key) => this.#queryTypes.get(key));
  }

  /**
   * Finds expression nodes once per file in the current snapshot.
   *
   * @param file - Source file from the current checker snapshot.
   * @returns Cached expression nodes in AST traversal order. Do not mutate the array.
   */
  expressions(file: SourceFile): readonly Node[] {
    let nodes = this.#queryNodes.get(file.fileName);
    if (!nodes) {
      const expressions: Node[] = [];
      visit(file, (node) => {
        if (isExpression(node)) {
          expressions.push(node);
        }
      });
      nodes = expressions;
      this.#queryNodes.set(file.fileName, nodes);
    }
    return nodes;
  }

  /**
   * Reads the consumer's declaration roots using the captured configuration overlays. Ordinary
   * source files selected by the consumer remain outside the generated project's root files.
   *
   * @param inputs - File replacements used while parsing the selected configuration.
   * @returns Declaration filenames selected by TypeScript, or an empty list for isolated settings.
   * Restores the overlay used by the retained snapshot before returning or throwing.
   * @throws If TypeScript cannot parse the selected configuration or its inherited files.
   */
  #configuredDeclarations(inputs: TypeFactsInputs): readonly string[] {
    if (this.#project.kind === "isolated") {
      return [];
    }
    const previous = this.#virtualFiles;
    this.#virtualFiles = new Map(
      [...(inputs.overlayFiles ?? []), ...(inputs.ambientDeclarations ?? [])].map(
        ([name, text]) => [normalize(name), text],
      ),
    );
    try {
      return this.#api
        .parseConfigFile(this.#project.configPath)
        .fileNames.filter((file) => /\.d\.[cm]?ts$/i.test(file));
    } finally {
      this.#virtualFiles = previous;
    }
  }

  /**
   * Starts a checker API whose filesystem callbacks consult the current virtual overlay.
   *
   * @returns A new API using the project root, with timing collection enabled. Increments starts
   * before construction. Read callbacks update the filesystem counters.
   * @throws If API construction fails.
   */
  #startApi(): API {
    this.stats.starts += 1;
    return withHiddenSpawnWindows(
      () =>
        new API({
          cwd: this.#projectRoot,
          collectTiming: true,
          fs: {
            directoryExists: (directoryName) => {
              const prefix = normalize(directoryName).replace(/\/$/, "") + "/";
              return [...this.#virtualFiles.keys()].some((name) => name.startsWith(prefix))
                ? true
                : undefined;
            },
            fileExists: (fileName) => {
              this.dependencies.add(fileName);
              return this.#virtualFiles.has(normalize(fileName)) ? true : undefined;
            },
            readFile: (fileName) => {
              const overlay = this.#virtualFiles.get(normalize(fileName));
              if (overlay === undefined) {
                this.dependencies.add(fileName);
                this.stats.diskReadRequests += 1;
              } else {
                this.stats.overlayReads += 1;
              }
              return overlay;
            },
          },
        }),
    );
  }

  /**
   * Disposes the previous snapshot and API, then replaces the API without clearing virtual files or
   * counters.
   *
   * @throws If cleanup or creation of the replacement API fails.
   */
  restart(): void {
    this.#disposeApi();
    this.#api = this.#startApi();
  }

  /**
   * Releases the API and all answers whose objects belong to its snapshot.
   */
  #disposeApi(): void {
    this.#snapshot?.dispose();
    this.#snapshot = undefined;
    this.#clearQueries();
    this.#api.close();
  }

  /**
   * Discards query results before replacing a checker snapshot.
   */
  #clearQueries(): void {
    this.properties.named.clear();
    this.properties.all.clear();
    this.#queryTypes.clear();
    this.#queryNodes.clear();
    this.diagnostics.clear();
  }

  /**
   * Replaces virtual files and retains the new snapshot after releasing the previous one.
   *
   * @param prepared - Complete normalized virtual project, retained as the new filesystem overlay.
   * @returns The checker project from the new snapshot. Snapshot counters are incremented after update.
   * @throws If snapshot update or disposal fails, or the generated project cannot be opened.
   */
  updateProject(prepared: PreparedProject) {
    const changed: string[] = [];
    const created: string[] = [];
    const deleted: string[] = [];
    for (const [fileName, contents] of prepared.virtualFiles) {
      if (!this.#virtualFiles.has(fileName)) {
        (existsSync(fileName) ? changed : created).push(fileName);
      } else if (this.#virtualFiles.get(fileName) !== contents) {
        changed.push(fileName);
      }
    }
    for (const fileName of this.#virtualFiles.keys()) {
      if (!prepared.virtualFiles.has(fileName)) {
        (existsSync(fileName) ? changed : deleted).push(fileName);
      }
    }
    this.#virtualFiles = prepared.virtualFiles;

    if (this.#snapshot && changed.length === 0 && created.length === 0 && deleted.length === 0) {
      const project = this.#snapshot.getProject(this.#shadowTsconfigPath);
      if (project) {
        return project;
      }
    }

    this.#clearQueries();
    const next = this.#snapshot
      ? this.#api.updateSnapshot({
          fileChanges: {
            changed,
            created,
            deleted,
          },
        })
      : this.#api.updateSnapshot({
          openProjects: [this.#shadowTsconfigPath],
        });
    this.stats.snapshots += 1;
    if (this.#snapshot) {
      this.stats.incrementalSnapshots += 1;
    }
    this.#snapshot?.dispose();
    this.#snapshot = next;
    const project = next.getProject(this.#shadowTsconfigPath);
    if (!project) {
      throw new Error("TypeScript 7 could not open the Stillwind shadow project");
    }
    return project;
  }

  /**
   * Releases the checker process and all objects owned by its snapshot.
   */
  close(): void {
    this.#disposeApi();
  }
}
