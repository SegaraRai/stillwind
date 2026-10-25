import { fingerprint, type NativeAssetRecord } from "@stillwind/native";
import { minimatch } from "minimatch";
import * as path from "pathe";
import ts from "typescript-6";

import { TypeFactsError, type SemanticInputs, type TypeFactSource } from "../backend.ts";
import { checkerId, CheckerSource } from "../checker-source.ts";

/**
 * Supplies complete virtual files and TypeScript settings for a browser checker.
 */
export interface BrowserCheckerOptions {
  /** Absolute virtual project path. Paths use forward slashes. */
  root: string;
  /** Files include package metadata, declarations, standard libraries, and source documents. */
  files: ReadonlyMap<string, string>;
  /** Selected virtual tsconfig path. False uses isolated defaults. */
  tsconfig?: string | false;
}

/**
 * Maintains the TypeScript program over a browser workspace's virtual files.
 */
export class BrowserProgram {
  /** Root, settings, and dependency files used when preparing checker inputs. */
  readonly #options: BrowserCheckerOptions;

  /** Language service reused while snapshots and project versions change. */
  #service?: ts.LanguageService;

  /** Current captured inventory with overlays, checker buffers, and helper declarations applied. */
  #files = new Map<string, string>();

  /** Per-file change counters exposed as language-service script versions. */
  #versions = new Map<string, number>();

  /** Effective TypeScript options from the virtual tsconfig and enforced checker settings. */
  #compilerOptions: ts.CompilerOptions = {};

  /** Checker buffers and ambient declaration filenames selected as current project roots. */
  #roots: string[] = [];

  /** Advances when file text, roots, or compiler options change. */
  #revision = 0;

  /** Serialized compiler options and root filenames, used to detect settings changes. */
  #settings = "";

  /**
   * Creates a checker without loading its sources.
   *
   * After changing dependency files, call invalidate before preparing another program.
   * Do not change files while a query is running.
   *
   * @param options - Virtual root, complete dependency files, and optional tsconfig selection.
   */
  constructor(options: BrowserCheckerOptions) {
    this.#options = options;
  }

  /**
   * Builds checker buffers and updates the language service from the supplied files.
   *
   * @param sources - Sources with original UTF-8 probes and any framework checker buffers.
   * @param assets - Declarations grouped by module ID, used to mark producer calls.
   * @param inputs - Overlays and ambient declarations applied over the configured files.
   * @returns The updated program and mappings between original sources and checker buffers.
   * @throws If a probe cannot be mapped, tsconfig is invalid, or the language service has no program.
   */
  prepare(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs: SemanticInputs,
  ): { program: ts.Program; mappings: CheckerSource[] } {
    const files = new Map([
      ...this.#options.files,
      ...(inputs.overlayFiles ?? []),
      ...(inputs.ambientDeclarations ?? []),
    ]);
    const mappings: CheckerSource[] = [];
    const ids = new Map(sources.map((source) => [source.resolverId, checkerId(source)]));
    const extraRoots = new Set<string>(inputs.ambientDeclarations?.keys());
    for (const source of sources) {
      if (source.kind === "projected" && source.projection.checkerGlobals) {
        const id = path.join(
          this.#options.root,
          `.stillwind-globals-${fingerprint(source.projection.checkerGlobals)}.d.ts`,
        );
        files.set(id, source.projection.checkerGlobals);
        extraRoots.add(id);
      }
      const syntax = source.kind === "direct" ? source.syntax : source.projection.syntax;
      const buffers = source.checkerBuffers ?? [
        {
          source: source.kind === "direct" ? source.document.code : source.projection.code,
          language:
            syntax.language === "tsx" || syntax.language === "jsx"
              ? ("tsx" as const)
              : ("ts" as const),
        },
      ];
      for (const [index, buffer] of buffers.entries()) {
        const mapping = new CheckerSource(
          source,
          buffer,
          index,
          assets.get(source.moduleId) ?? [],
          ids,
        );
        files.set(mapping.id, mapping.text);
        mappings.push(mapping);
        if (mapping.helperModule) {
          files.set(mapping.helperModule.id, mapping.helperModule.text);
        }
      }
      for (const probe of source.probes) {
        if (
          !mappings.some(
            (mapping) =>
              mapping.source === source &&
              [...mapping.probes.values()].some(
                (candidate) => candidate.start === probe.start && candidate.end === probe.end,
              ),
          )
        ) {
          throw new TypeFactsError(
            source.moduleId,
            probe.start,
            probe.end,
            "checker",
            "Cannot map this expression to a checker source",
          );
        }
      }
      for (const id of source.checkerTypes ?? []) {
        extraRoots.add(id);
      }
    }
    const structureChanged =
      files.size !== this.#files.size || [...files.keys()].some((id) => !this.#files.has(id));
    const resolutionChanged =
      structureChanged ||
      [...files].some(([id, text]) => /\.json$/i.test(id) && this.#files.get(id) !== text);
    let changed = structureChanged;
    for (const [id, text] of files) {
      if (this.#files.get(id) !== text) {
        changed = true;
        this.#versions.set(id, (this.#versions.get(id) ?? 0) + 1);
      }
    }
    this.#files = files;
    const configPath = path.resolve(this.#options.root, this.#options.tsconfig || "tsconfig.json");
    let configured: ts.ParsedCommandLine | undefined;
    if (this.#options.tsconfig !== false && files.has(configPath)) {
      const config = ts.readConfigFile(configPath, (id) => files.get(id));
      if (config.error) {
        throw new Error(ts.flattenDiagnosticMessageText(config.error.messageText, "\n"));
      }
      configured = ts.parseJsonConfigFileContent(
        config.config,
        {
          useCaseSensitiveFileNames: true,
          fileExists: (id) => files.has(id),
          readFile: (id) => files.get(id),
          readDirectory: (directory, extensions, excludes, includes, depth) =>
            [...files.keys()].filter(
              (id) =>
                id.startsWith(directory + "/") &&
                (!extensions || extensions.some((ext) => id.endsWith(ext))) &&
                (depth === undefined ||
                  path.relative(directory, id).split("/").length - 1 <= depth) &&
                (includes ?? ["**/*"]).some((pattern) => matchesFile(directory, id, pattern)) &&
                !(excludes ?? []).some((pattern) => matchesFile(directory, id, pattern)),
            ),
        },
        path.dirname(configPath),
      );
      const error = configured.errors.find((item) => item.code !== 18003);
      if (error) {
        throw new Error(ts.flattenDiagnosticMessageText(error.messageText, "\n"));
      }
    } else if (typeof this.#options.tsconfig === "string") {
      throw new Error(`TypeScript configuration does not exist: ${configPath}`);
    }
    this.#compilerOptions = {
      target: ts.ScriptTarget.ESNext,
      module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.Bundler,
      jsx: ts.JsxEmit.Preserve,
      strict: true,
      types: [],
      ...configured?.options,
      allowJs: true,
      checkJs: true,
      allowImportingTsExtensions: true,
      noEmit: true,
      skipLibCheck: true,
    };
    this.#roots = [
      ...new Set([
        ...mappings.map((mapping) => mapping.id),
        ...extraRoots,
        ...(configured?.fileNames.filter((id) => /\.d\.[cm]?ts$/.test(id)) ?? []),
      ]),
    ];
    const settings = JSON.stringify([this.#compilerOptions, this.#roots]);
    if (changed || settings !== this.#settings) {
      this.#revision++;
    }
    this.#settings = settings;
    if (resolutionChanged) {
      this.invalidate();
    }
    if (!this.#service) {
      /**
       * Infers a virtual directory from the captured files beneath it.
       * @param directory - Absolute virtual directory with an optional trailing slash.
       * @returns Whether at least one file key starts with the directory prefix.
       */
      const directoryExists = (directory: string) =>
        [...this.#files.keys()].some((id) => id.startsWith(directory.replace(/\/$/, "") + "/"));
      this.#service = ts.createLanguageService({
        getCompilationSettings: () => this.#compilerOptions,
        getScriptFileNames: () => this.#roots,
        getScriptVersion: (id) => String(this.#versions.get(id) ?? 0),
        getProjectVersion: () => String(this.#revision),
        getScriptSnapshot: (id) => {
          const text = this.#files.get(id);
          return text === undefined ? undefined : ts.ScriptSnapshot.fromString(text);
        },
        getCurrentDirectory: () => this.#options.root,
        getDefaultLibFileName: () => "/node_modules/typescript/lib/lib.esnext.full.d.ts",
        fileExists: (id) => this.#files.has(id),
        readFile: (id) => this.#files.get(id),
        directoryExists,
        getDirectories: (directory) => [
          ...new Set(
            [...this.#files.keys()]
              .filter((id) => id.startsWith(directory + "/"))
              .map((id) => id.slice(directory.length + 1).split("/", 1)[0]),
          ),
        ],
        useCaseSensitiveFileNames: () => true,
      });
    }

    const program = this.#service.getProgram();
    if (!program) {
      throw new Error("Browser checker could not open the Stillwind project");
    }

    return {
      program,
      mappings,
    };
  }

  /**
   * Discards the language service after file or configuration changes.
   */
  invalidate(): void {
    this.#service?.dispose();
    this.#service = undefined;
  }
}

/**
 * Applies tsconfig file and directory patterns to paths in the virtual filesystem.
 *
 * @param directory - Absolute virtual base directory.
 * @param id - Absolute virtual file path.
 * @param pattern - Absolute or directory-relative glob. A plain directory name expands to its descendants.
 * @returns Whether the relative path matches, including dot-prefixed segments.
 */
function matchesFile(directory: string, id: string, pattern: string): boolean {
  const name = path.relative(directory, id);
  const glob = path.isAbsolute(pattern) ? path.relative(directory, pattern) : pattern;
  const expanded = !/[?*]/.test(glob) && !/\.[^/]+$/.test(glob) ? `${glob}/**/*` : glob;
  return minimatch(name, expanded, { dot: true });
}
