import { relative } from "node:path";
import { pathToFileURL } from "node:url";

import { scanSource, type NativeProjectSession } from "@stillwind/native";
import { checkerEnvironment, projectFrameworkSource } from "@stillwind/projection";

import type { ModuleSummary, ProjectSource } from "./protocol.ts";
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
    ProjectSource,
    | "module_id"
    | "uri"
    | "checker_file_name"
    | "projection_file_name"
    | "language"
    | "module_kind"
    | "source_format"
    | "framework"
  >;
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
export function fileSourceKind(
  id: string,
): Pick<ProjectSource, "language" | "module_kind" | "source_format" | "framework"> {
  const name = cleanModuleId(id).toLowerCase();
  const extension = /\.[^.]$/.exec(name)?.[0];
  const source_format =
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
    language,
    module_kind: "unambiguous",
    source_format,
    framework: undefined,
  };
}

/** Describes a filesystem module for the project compiler. */
export function fileModuleDescriptor(
  root: string,
  id: string,
): ReturnType<CompilerHost["describe"]> {
  const file = cleanModuleId(id);
  return {
    module_id: compilerModuleId(root, file),
    uri: pathToFileURL(file).href,
    checker_file_name: file,
    projection_file_name: compilerModuleId(root, file),
    ...fileSourceKind(file),
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
    return summary.probe_imports;
  }
  const selected = summary.export_imports.find((entry) => entry.name === requested);
  if (requested !== "*" && selected) {
    return selected.imports;
  }
  const reExports = summary.re_exports.filter(
    (entry) => requested === "*" || entry.exported === requested || entry.exported === "*",
  );
  if (requested !== "*" && reExports.length > 0) {
    return reExports.map((entry) => ({
      source: entry.source,
      imported: entry.exported === "*" ? requested : entry.imported,
    }));
  }
  // Namespace imports and exports without a declaration slice remain conservative.
  return [...summary.imports, ...summary.re_exports];
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
  const entrySource = await projectFrameworkSource({
    resolver_id: entryId,
    ...host.describe(entryId),
    source: code,
  });
  const entrySummary = scanSource(entrySource);
  const hasModuleCssImport = entrySummary.module_css_imports > 0;
  if (entrySummary.macro_imports > 0) {
    entrySource.stylesheet_imports = options.stylesheetImports;
  }
  if (hasModuleCssImport) {
    entrySource.module_css_import = options.moduleCssImport;
  }
  const needsTypeFacts = entrySummary.type_probes.length > 0;
  const summaries = new Map<string, ModuleSummary>();
  const sourcesByResolverId = new Map<string, ProjectSource>();
  const resolverIdsByModuleId = new Map<string, string>();
  const registerSource = (source: ProjectSource) => {
    const previous = resolverIdsByModuleId.get(source.module_id);
    if (previous !== undefined && previous !== source.resolver_id) {
      fail(
        `Module identity ${source.module_id} is shared by ${previous} and ${source.resolver_id}`,
      );
    }
    const existing = sourcesByResolverId.get(source.resolver_id);
    if (existing !== undefined && existing.module_id !== source.module_id) {
      fail(`Resolver identity ${source.resolver_id} changed module identity`);
    }
    resolverIdsByModuleId.set(source.module_id, source.resolver_id);
    sourcesByResolverId.set(source.resolver_id, source);
    host.onModule?.(source.module_id, source.resolver_id);
  };
  registerSource(entrySource);
  const resolutions = new Map<string, Map<string, string>>();
  const visited = new Map<string, Set<string | null>>();
  const pending: Array<{ source: ProjectSource; requested: string | null }> = [
    {
      source: entrySource,
      requested: null,
    },
  ];
  while (pending.length > 0) {
    const { source, requested } = pending.pop()!;
    const demands = visited.get(source.resolver_id) ?? new Set<string | null>();
    if (demands.has(requested)) {
      continue;
    }
    demands.add(requested);
    visited.set(source.resolver_id, demands);
    const summary =
      summaries.get(source.module_id) ??
      (source.resolver_id === entryId ? entrySummary : scanSource(source));
    if (summary.module_id !== source.module_id) {
      fail(`Scanner changed module identity for ${source.resolver_id}`);
    }
    summaries.set(source.module_id, summary);
    if (!needsTypeFacts) {
      continue;
    }
    for (const dependency of semanticDependencies(summary, requested)) {
      const resolved = await host.resolve(dependency.source, source.resolver_id, {
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
      if (!/\.(?:[cm]?[jt]sx?|vue|svelte|astro)$/.test(dependencyName)) {
        continue;
      }
      let sourceResolutions = resolutions.get(source.resolver_id);
      if (!sourceResolutions) {
        resolutions.set(source.resolver_id, (sourceResolutions = new Map()));
      }
      sourceResolutions.set(dependency.source, dependencyId);
      host.watch(dependencyId);
      let dependencySource = sourcesByResolverId.get(dependencyId);
      if (!dependencySource) {
        dependencySource = await projectFrameworkSource({
          resolver_id: dependencyId,
          ...host.describe(dependencyId),
          source: await host.read(dependencyId),
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
            const summary = summaries.get(source.module_id)!;
            return {
              ...source,
              ...checkerEnvironment(source),
              checker_spans: summary.checkerSpans,
              probes: summary.type_probes,
              resolutions: summary.module_specifiers.flatMap((specifier) => {
                const id = resolutions.get(source.resolver_id)?.get(specifier.source);
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
    const source = sources.find((source) => source.module_id === error.module_id)!;
    return {
      module_id: entrySource.module_id,
      code: null,
      css: "",
      resources: [],
      mappings: [],
      classes: [],
      symbols: [],
      diagnostics: [error.diagnostic(source.fw_code ?? source.source, source.uri)],
      reports: [],
      rejected_names: [],
      lint_findings: [],
      lint_policy: {} as Record<string, "off" | "warn" | "deny">,
      composition: [],
      hasModuleCssImport,
      compositionModules: [],
      compositionTopology: "[]",
    };
  }
  const typedSources = sources.map((source) => ({
    ...source,
    module_css_import:
      source.resolver_id === entryId ? source.module_css_import : "stillwind/module-css",
    type_facts: typeFacts.get(source.module_id) ?? [],
  }));
  const projectOutput = session.compile(typedSources, entrySource.module_id);
  for (const output of projectOutput.modules) {
    const resolverId = resolverIdsByModuleId.get(output.module_id);
    if (resolverId === undefined) {
      fail(`Stillwind returned an unknown module: ${output.module_id}`);
    }
    if (output.diagnostics.length > 0 && options.diagnostics !== "collect") {
      const diagnostic = output.diagnostics[0];
      fail(`${diagnostic.code}: ${diagnostic.message} (${diagnostic.uri}:${diagnostic.start})`);
    }
    const deniedFindings: string[] = [];
    for (const finding of output.lint_findings ?? []) {
      const level = output.lint_policy?.[finding.rule] ?? "warn";
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
    for (const rejected of output.rejected_names ?? []) {
      host.warn(
        `stillwind: the ${rejected.kind} name for ${rejected.hash} was rejected: ${rejected.reason}`,
      );
    }
  }
  const output = projectOutput.modules.find(
    (candidate) => candidate.module_id === entrySource.module_id,
  );
  const composition = output?.composition ?? [];
  const compositionModules = [...new Set(composition.map(({ base }) => base.module_id))].filter(
    (module) => module !== entrySource.module_id,
  );
  const compositionTopology = JSON.stringify(
    composition
      .map(({ derived, base }) => [
        `${derived.module_id}:${derived.start}`,
        `${base.module_id}:${base.start}`,
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
