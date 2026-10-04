import { relative } from "node:path";
import { pathToFileURL } from "node:url";

import { scanSource, type NativeProjectSession } from "@stillwind/native";
import { checkerEnvironment, prepareSource } from "@stillwind/projection";

import type {
  ModuleSummary,
  PreparedSource,
  ProjectInput,
  SourceDocument,
  ScriptSyntax,
} from "./protocol.ts";
import { TypeFactsError, type TypeScriptFactsService, type TypeFact } from "./type-facts.ts";

/**
 * Host services for resolving and reading semantic dependencies, registering watches,
 * and reporting lint warnings. Resolution may exclude external modules or return null.
 */
export interface CompilerHost {
  /** Describes one resolved source before it is scanned or compiled. */
  describe(
    id: string,
  ): Pick<
    ProjectInput,
    "moduleId" | "checkerFileName" | "projectionFileName" | "syntax" | "framework"
  > & { document: Omit<SourceDocument, "code"> };
  resolve(
    specifier: string,
    importer: string,
    options?: { skipSelf: boolean },
  ): Promise<{ id: string; external?: boolean | string } | null>;
  read(id: string): Promise<string>;
  /** Records the identity selected for each source in this compilation. */
  onModule?(moduleId: string, resolverId: string): void;
  watch(id: string): void;
  warn(message: string): void;
}

function fail(message: string): never {
  throw new Error(message);
}

function asRecord<const T extends Readonly<Record<string, any>>>(
  record: T,
): T extends Record<string, infer U> ? Partial<Readonly<Record<string, U>>> : never {
  return record as any;
}

/**
 * Returns the root-relative module identity with forward slashes. Paths outside
 * root retain `..` segments; this does not validate filesystem containment.
 */
export function compilerModuleId(root: string, id: string): string {
  return relative(root, id).replaceAll("\\", "/");
}

/**
 * Returns parser metadata and the original format inferred from id's extension,
 * ignoring query and fragment suffixes. Leaves the output adapter undefined.
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

/** Describes a filesystem module for the project compiler. */
export function fileModuleDescriptor(
  root: string,
  id: string,
): ReturnType<CompilerHost["describe"]> {
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
 * Removes the query or fragment suffix from a host module identifier.
 */
export function cleanModuleId(id: string): string {
  if (id.startsWith("\0") || id.startsWith("virtual:")) {
    return id;
  }
  return id.replace(/[?#].*$/, "");
}

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

/**
 * Compiles an entry buffer and its demanded semantic dependencies into a project session.
 * The host descriptor supplies the module identity and parser metadata for each source.
 * Type facts are requested lazily for original-coordinate probes in JS/TS or SFC inputs.
 *
 * Returns the entry output with composition metadata, or undefined when normal mode
 * has no generated entry code. Collect mode retains compiler diagnostics across outputs;
 * normal mode throws on compiler diagnostics and deny-level lint findings.
 * Checker contract errors use original coordinates and participate in collect mode.
 * Host I/O, projection, checker process, and native failures propagate in either mode.
 * Session updates, watches, and warnings are not rolled back on failure.
 */
export async function compileModule(
  session: NativeProjectSession,
  factsService: () => TypeScriptFactsService,
  code: string,
  id: string,
  projectRoot: string,
  host: CompilerHost,
  options: {
    diagnostics?: "throw" | "collect";
    stylesheetImports?: string[];
    moduleCssImport?: string;
  } = {},
) {
  const entryId = id;
  const entryDescriptor = host.describe(entryId);
  const entrySource = await prepareSource({
    resolverId: entryId,
    ...entryDescriptor,
    document: {
      ...entryDescriptor.document,
      code,
    },
  });
  const entrySummary = scanSource(entrySource);
  const hasModuleCssImport = entrySummary.moduleCssImports > 0;
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
      fail(`Module identity ${source.moduleId} is shared by ${previous} and ${source.resolverId}`);
    }
    const existing = sourcesByResolverId.get(source.resolverId);
    if (existing !== undefined && existing.moduleId !== source.moduleId) {
      fail(`Resolver identity ${source.resolverId} changed module identity`);
    }
    resolverIdsByModuleId.set(source.moduleId, source.resolverId);
    sourcesByResolverId.set(source.resolverId, source);
    host.onModule?.(source.moduleId, source.resolverId);
  };
  registerSource(entrySource);
  const resolutions = new Map<string, Map<string, string>>();
  const visited = new Map<string, Set<string | null>>();
  const pending: Array<{ source: PreparedSource; requested: string | null }> = [
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
      (source.resolverId === entryId ? entrySummary : scanSource(source));
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
        dependencySource = await prepareSource({
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
  let typeFacts: Map<string, TypeFact[]>;
  try {
    typeFacts = needsTypeFacts
      ? factsService().collect(
          sources.map((source) => {
            const summary = summaries.get(source.moduleId)!;
            return {
              ...source,
              ...checkerEnvironment(source),
              checkerSpans: summary.checkerSpans,
              checkerBindings: summary.checkerBindings,
              probes: summary.typeProbes,
              resolutions: summary.moduleSpecifiers.flatMap((specifier) => {
                const id = resolutions.get(source.resolverId)?.get(specifier.source);
                return id ? [{ start: specifier.start, end: specifier.end, id }] : [];
              }),
            };
          }),
          new Map([...summaries].map(([path, summary]) => [path, summary.assets])),
        )
      : new Map<string, TypeFact[]>();
  } catch (error) {
    if (!(error instanceof TypeFactsError) || options.diagnostics !== "collect") {
      throw error;
    }
    const source = sources.find((source) => source.moduleId === error.moduleId)!;
    return {
      moduleId: entrySource.moduleId,
      code: null,
      css: "",
      resources: [],
      mappings: [],
      classes: [],
      symbols: [],
      diagnostics: [error.diagnostic(source.document.code, source.document.uri)],
      reports: [],
      rejectedNames: [],
      lintFindings: [],
      lintSettings: {} as Record<string, "off" | "warn" | "deny">,
      composition: [],
      hasModuleCssImport,
      compositionModules: [],
      compositionTopology: "[]",
    };
  }
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
      const diagnostic = output.diagnostics[0];
      fail(`${diagnostic.code}: ${diagnostic.message} (${diagnostic.uri}:${diagnostic.start})`);
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
      fail(deniedFindings.join("\n"));
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
}
