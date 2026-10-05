import { relative } from "node:path";
import { pathToFileURL } from "node:url";

import type {
  NativeProjectSession,
  NativeDiagnosticBatch,
  BindingReport,
  BindingDiagnostic,
} from "@stillwind/native";
import {
  checkerEnvironment,
  prepareSource,
  PreparationError,
  type CheckerEnvironment,
} from "@stillwind/projection";

import { byteToUtf16 } from "./checker-source.ts";
import type {
  ModuleSummary,
  PreparedSource,
  ProjectInput,
  SourceDocument,
  ScriptSyntax,
} from "./protocol.ts";
import { TypeFactsError, type TypeScriptFactsService, type TypeFact } from "./type-facts.ts";

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

/** Source identities and parser metadata supplied by a host before preparation or compilation. */
export type ModuleDescriptor = Pick<
  ProjectInput,
  "moduleId" | "checkerFileName" | "projectionFileName" | "syntax" | "framework"
> & {
  /** Original document identity and format. The compiler obtains its text through the host. */
  document: Omit<SourceDocument, "code">;
};

/**
 * Source metadata, dependency resolution, reads, watches, and warnings supplied to module
 * compilation. Hosts must preserve a stable correspondence between resolver and compiler module
 * identities.
 */
export interface CompilerHost {
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
    options?: { skipSelf: boolean },
  ): Promise<{ id: string; external?: boolean | string } | null>;
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

/**
 * Rejects compilation when host and compiler module identities violate an internal contract.
 *
 * @param message - Description of the violated identity contract.
 * @returns No value. This function always throws.
 * @throws An Error containing the supplied message.
 */
function fail(message: string): never {
  throw new Error(message);
}

/**
 * Allows a constant lookup table to be indexed by an arbitrary string at the type level.
 *
 * @param record - Lookup table retained without copying or validation.
 * @returns The same object typed so an unknown key can produce undefined.
 */
function asRecord<const T extends Readonly<Record<string, any>>>(
  record: T,
): T extends Record<string, infer U> ? Partial<Readonly<Record<string, U>>> : never {
  return record as any;
}

/**
 * Derives a compiler module identity relative to a filesystem root.
 *
 * @param root - Filesystem directory from which the identity is made relative.
 * @param id - Filesystem path to identify. Callers remove resolver queries before using this
 * helper.
 * @returns A relative path with forward slashes. Paths outside the root retain `..` segments. This
 * function does not enforce containment.
 */
export function compilerModuleId(root: string, id: string): string {
  return relative(root, id).replaceAll("\\", "/");
}

/**
 * Infers the source format and parser language from a module path.
 *
 * @param id - Source identity whose query or fragment is removed before case-insensitive extension
 * matching.
 * @returns Parser settings with unambiguous module kind and the inferred source format. TypeScript
 * and JSX extensions choose their parser languages. Declaration files use `dts`, and SFC
 * extensions select projected formats. Unrecognized extensions default to JavaScript script input.
 */
export function fileSourceKind(id: string): {
  syntax: ScriptSyntax;
  format: SourceDocument["format"];
} {
  const name = cleanModuleId(id).toLowerCase();
  const extension = /\.[^.]+$/.exec(name)?.[0];
  const format =
    asRecord({
      ".jsx": "jsx",
      ".tsx": "jsx",
      ".astro": "astro",
      ".svelte": "svelte",
      ".vue": "vue",
    })[extension ?? ""] ?? "script";
  const language = /\.d\.[cm]?ts$/.test(name)
    ? "dts"
    : (asRecord({
        ".ts": "ts",
        ".cts": "ts",
        ".mts": "ts",
        ".tsx": "tsx",
        ".jsx": "jsx",
      })[extension ?? ""] ?? "js");
  return {
    syntax: {
      language,
      moduleKind: "unambiguous",
    },
    format,
  };
}

/**
 * Infers parser metadata from a filesystem path and assigns compiler and checker identities.
 *
 * @param root - Project directory used for relative compiler and projection identities.
 * @param id - Absolute source path, optionally followed by a query or fragment suffix.
 * @returns Metadata for the file after removing its query or fragment suffix.
 */
export function fileModuleDescriptor(root: string, id: string): ModuleDescriptor {
  const file = cleanModuleId(id);
  const { syntax, format } = fileSourceKind(file);
  return {
    moduleId: compilerModuleId(root, file),
    document: {
      uri: pathToFileURL(file).href,
      format,
    },
    checkerFileName: file,
    projectionFileName: compilerModuleId(root, file),
    syntax,
  };
}

/**
 * Removes a filesystem identity's query or fragment before a disk operation.
 *
 * @param id - Resolver identity, including any query or fragment suffix.
 * @returns The identity up to its first `?` or `#`. IDs beginning with NUL or `virtual:` are
 * returned unchanged.
 */
export function cleanModuleId(id: string): string {
  if (id.startsWith("\0") || id.startsWith("virtual:")) {
    return id;
  }
  return id.replace(/[?#].*$/, "");
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
function semanticDependencies(summary: ModuleSummary, requested: string | null) {
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

/** Diagnostic behavior and generated stylesheet imports for one module compilation. */
export interface ModuleCompileOptions {
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
 * Compiles an entry buffer and its demanded semantic dependencies into a project session.
 * The host descriptor supplies the module identity and parser metadata for each source.
 * Type facts are requested lazily for original-coordinate probes in JS/TS or SFC inputs.
 *
 * Returns the entry output with composition metadata, or undefined when normal mode
 * has no generated entry code. Collect mode retains compiler diagnostics across outputs.
 * Normal mode throws on compiler diagnostics and deny-level lint findings.
 * Checker contract errors use original coordinates and participate in collect mode.
 * Known syntax, projection, and checker contract failures use the shared reports.
 * Host I/O, checker process, and malformed native transport failures propagate.
 * Session updates, watches, and warnings are not rolled back on failure.
 *
 * @param session - Native project session with an initialized design system.
 * @param factsService - Supplies the type-facts service when the source graph needs semantic facts.
 * @param code - Entry source before Stillwind transformation.
 * @param id - Full resolver identity of the entry source, including its query suffix.
 * @param host - Source metadata, resolution, reading, dependency tracking, and warning operations.
 * @param options - Diagnostic behavior and stylesheet import configuration for the entry source.
 * @returns A promise for the entry output and composition metadata, or undefined when normal mode
 * produces no entry code.
 * @throws If compilation rejects diagnostics, source preparation fails, or host or checker operations fail.
 */
export async function compileModule(
  session: NativeProjectSession,
  factsService: () => TypeScriptFactsService,
  code: string,
  id: string,
  host: CompilerHost,
  options: ModuleCompileOptions = {},
) {
  const entryId = id;
  const entryDescriptor = host.describe(entryId);
  const attempted = new Map<string, PreparedSource>();

  let hasModuleCssImport = false;

  const rejectPreparation = (input: ProjectInput | PreparedSource, error: PreparationError) => {
    const batch = session.reportSourceFailure({
      moduleId: input.moduleId,
      document: input.document,
      issue: error.issue,
      converter: error.converter,
      range: error.range,
    });
    return new CompilationFailure([batch], error);
  };

  const prepare = async (input: ProjectInput) => {
    host.onModule?.(input.moduleId, input.resolverId);
    try {
      const source = await prepareSource(input);
      attempted.set(source.moduleId, source);
      return source;
    } catch (error) {
      if (error instanceof PreparationError) {
        throw rejectPreparation(input, error);
      }
      throw error;
    }
  };

  const scan = (source: PreparedSource) => {
    const result = session.scanSource(source);
    if (result.kind === "failed") {
      throw new CompilationFailure([result.batch]);
    }
    return result.summary;
  };

  try {
    const entrySource = await prepare({
      resolverId: entryId,
      ...entryDescriptor,
      document: {
        ...entryDescriptor.document,
        code,
      },
    });
    const entrySummary = scan(entrySource);
    hasModuleCssImport = entrySummary.moduleCssImports > 0;
    if (entrySummary.macroImports > 0) {
      entrySource.stylesheetImports = options.stylesheetImports;
    }
    if (hasModuleCssImport) {
      entrySource.moduleCssImport = options.moduleCssImport;
    }
    const needsTypeFacts = entrySummary.typeProbes.length > 0;
    const summaries = new Map<string, ModuleSummary>();
    const sourcesByResolverId = new Map<string, PreparedSource>();
    const resolverIdsByModuleId = new Map<string, string>();
    const registerSource = (source: PreparedSource) => {
      const previous = resolverIdsByModuleId.get(source.moduleId);
      if (previous !== undefined && previous !== source.resolverId) {
        fail(
          `Module identity ${source.moduleId} is shared by ${previous} and ${source.resolverId}`,
        );
      }
      const existing = sourcesByResolverId.get(source.resolverId);
      if (existing !== undefined && existing.moduleId !== source.moduleId) {
        fail(`Resolver identity ${source.resolverId} changed module identity`);
      }
      resolverIdsByModuleId.set(source.moduleId, source.resolverId);
      sourcesByResolverId.set(source.resolverId, source);
    };
    registerSource(entrySource);
    const resolutions = new Map<string, Map<string, string>>();
    const visited = new Map<string, Set<string | null>>();
    const pending: { source: PreparedSource; requested: string | null }[] = [
      {
        source: entrySource,
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
      const summary =
        summaries.get(source.moduleId) ??
        (source.resolverId === entryId ? entrySummary : scan(source));
      if (summary.moduleId !== source.moduleId) {
        fail(`Scanner changed module identity for ${source.resolverId}`);
      }
      summaries.set(source.moduleId, summary);
      if (!needsTypeFacts) {
        continue;
      }
      for (const dependency of semanticDependencies(summary, requested)) {
        const resolved = await host.resolve(dependency.source, source.resolverId, {
          skipSelf: true,
        });
        if (!resolved || resolved.external) {
          continue;
        }
        const dependencyId = resolved.id;
        const dependencyName = cleanModuleId(dependencyId);
        // Package runtime exports do not identify their declaration files. Leave
        // external package type resolution (for example React) to TypeScript.
        if (/(?:^|[\\/])node_modules[\\/]/.test(dependencyName)) {
          continue;
        }
        if (!/\.(?:[cm]?[jt]sx?|astro|svelte|vue)$/.test(dependencyName)) {
          continue;
        }
        let sourceResolutions = resolutions.get(source.resolverId);
        if (!sourceResolutions) {
          resolutions.set(source.resolverId, (sourceResolutions = new Map()));
        }
        sourceResolutions.set(dependency.source, dependencyId);
        host.watch(dependencyId);
        let dependencySource = sourcesByResolverId.get(dependencyId);
        if (!dependencySource) {
          const descriptor = host.describe(dependencyId);
          dependencySource = await prepare({
            resolverId: dependencyId,
            ...descriptor,
            document: {
              ...descriptor.document,
              code: await host.read(dependencyId),
            },
          });
          registerSource(dependencySource);
        }
        pending.push({
          source: dependencySource,
          requested: dependency.imported,
        });
      }
    }

    const sources = [...sourcesByResolverId.values()];
    const typeFacts: Map<string, TypeFact[]> = needsTypeFacts
      ? factsService().collect(
          sources.map((source) => {
            const summary = summaries.get(source.moduleId)!;
            let environment: CheckerEnvironment;
            try {
              environment = checkerEnvironment(source);
            } catch (error) {
              if (error instanceof PreparationError) {
                throw rejectPreparation(source, error);
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
                const id = resolutions.get(source.resolverId)?.get(specifier.source);
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
          }),
          new Map([...summaries].map(([path, summary]) => [path, summary.assets])),
        )
      : new Map<string, TypeFact[]>();
    const typedSources = sources.map((source) => ({
      ...source,
      moduleCssImport:
        source.resolverId === entryId ? source.moduleCssImport : "stillwind/module-css",
      typeFacts: typeFacts.get(source.moduleId) ?? [],
    }));
    const projectOutput = session.compile(typedSources, entrySource.moduleId);
    for (const output of projectOutput.modules) {
      const resolverId = resolverIdsByModuleId.get(output.moduleId);
      if (resolverId === undefined) {
        fail(`Stillwind returned an unknown module: ${output.moduleId}`);
      }
      if (output.diagnostics.length > 0 && options.diagnostics !== "collect") {
        throw new CompilationFailure(
          projectOutput.modules
            .filter((module) => module.diagnostics.length > 0)
            .map((module) => ({
              module: module.moduleId,
              reports: module.reports,
            })),
        );
      }
      const deniedFindings: string[] = [];
      for (const finding of output.lintFindings ?? []) {
        const level = output.lintSettings?.[finding.rule] ?? "warn";
        if (level === "off") {
          continue;
        }
        if (level !== "deny") {
          host.warn(`${finding.rule}: ${finding.message} (${finding.uri}:${finding.start})`);
        } else {
          deniedFindings.push(
            `${finding.rule}: ${finding.message} (${finding.uri}:${finding.start})`,
          );
        }
      }
      if (deniedFindings.length > 0 && options.diagnostics !== "collect") {
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
    const output = projectOutput.modules.find(
      (candidate) => candidate.moduleId === entrySource.moduleId,
    );
    const composition = output?.composition ?? [];
    const compositionModules = [...new Set(composition.map(({ base }) => base.moduleId))].filter(
      (module) => module !== entrySource.moduleId,
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
        diagnostics: projectOutput.modules.flatMap((module) => module.diagnostics),
        reports: projectOutput.modules.flatMap((module) => module.reports),
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
  } catch (error) {
    let failure = error;
    if (error instanceof TypeFactsError) {
      const source = attempted.get(error.moduleId);
      if (!source) {
        throw error;
      }
      const range =
        error.start !== undefined && error.end !== undefined
          ? {
              start: byteToUtf16(source.document.code, error.start),
              end: byteToUtf16(source.document.code, error.end),
            }
          : undefined;
      failure = new CompilationFailure(
        [
          session.reportSourceFailure({
            moduleId: source.moduleId,
            document: source.document,
            issue: error.issue,
            range,
            converter: source.kind === "projected" ? source.projection.converter : undefined,
          }),
        ],
        error,
      );
    }
    if (failure instanceof CompilationFailure && options.diagnostics === "collect") {
      return failedOutput(entryDescriptor.moduleId, failure.batches, hasModuleCssImport);
    }
    throw failure;
  }
}
