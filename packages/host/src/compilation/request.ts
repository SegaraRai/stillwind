import type { BindingEdit, NativeDiagnosticBatch, ProjectModuleOutput } from "@stillwind/native";
import type { PreparedSource } from "@stillwind/projection";

import type { TypeFactsInputs, UnverifiedRange } from "../semantics/index.ts";

/**
 * Current reports and sources for the semantic graph demanded by one entry.
 */
export interface ModuleAnalysis {
  /** Compiler identity of the entry after loading its descriptor. */
  entry: string;
  /** Traversal covers dependencies demanded by semantic probes, rather than all application imports. */
  coverage: "observed";
  /** Outputs produced by native project compilation, including diagnostic-only outputs. */
  modules: ProjectModuleOutput[];
  /** Structured diagnostic batches keyed by compiler module identity. */
  batches: NativeDiagnosticBatch[];
  /** Nonempty first fix actions in report order, excluding editor sorting actions. */
  fixes: BindingEdit[][];
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
 * One semantic import observed while answering an entry's type probes.
 */
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

/**
 * Diagnostics, captured checker inputs, and generated imports for one module compilation.
 */
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

/**
 * Creates an empty observed graph before loading its semantic dependencies.
 *
 * @param entry - Compiler module ID of the loaded entry source.
 * @returns A mutable graph with no reports or dependencies and generation initially allowed.
 */
export function createModuleAnalysis(entry: string): ModuleAnalysis {
  return {
    entry: entry,
    coverage: "observed",
    modules: [],
    batches: [],
    fixes: [],
    sources: [],
    dependencies: [],
    unverified: [],
    generation: "complete",
  };
}
