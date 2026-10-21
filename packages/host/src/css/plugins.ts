import type { PluginContext, StillwindPlugin, TailwindConfig } from "@stillwind/config";
import {
  NativeDesignSystemLoader,
  type NativeCompilerOptions,
  type NativeCssFileInput,
  type NativeCssLoadStep,
  type NativeCssRequest,
  type NativeEditorQuery,
  type NativeProjectSession,
} from "@stillwind/native";

import type { PluginPathCompletion } from "./plugin-paths.ts";
import type { configurePlugin, TailwindRuntime } from "./tailwind/plugin.ts";

/**
 * A plugin written against Tailwind CSS v4's JavaScript plugin API: a handler, `{ handler,
 * config }`, or a factory from `plugin.withOptions`. The host loads its adapter, and with it the
 * `tailwindcss` package, only when a `@plugin` statement selects such a plugin.
 */
export type TailwindPlugin =
  | ((...args: never[]) => unknown)
  | {
      /** Handler that receives Tailwind's plugin API. */
      handler: (...args: never[]) => unknown;
      /** v3-style config, such as `theme.extend`. */
      config?: object;
    };

/** Project settings that the Tailwind plugin adapter applies to the whole CSS graph. */
export interface TailwindSettings {
  /** Tailwind configuration, or a promise for one, which replaces Tailwind's `@config`. */
  tailwind?: TailwindConfig | Promise<TailwindConfig>;
  /** Value of Tailwind's `important` configuration key. */
  important?: true | string;
}

/**
 * Specifier of the `@plugin` request that the native loader adds after the root stylesheet for
 * the Tailwind settings. Decoded CSS never contains NUL, so no authored statement has it.
 */
const SETTINGS = "\0stillwind-tailwind-settings";

/** A native Stillwind plugin or a plugin using Tailwind v4's JavaScript API. */
export type LoadedPlugin = StillwindPlugin | TailwindPlugin;

/**
 * Recognizes Tailwind's plugin export forms without executing a factory.
 * @param value - Default export of a plugin module.
 * @returns Whether `value` is a function or an object with a `handler` function.
 */
function isTailwindPlugin(value: unknown): value is TailwindPlugin {
  return (
    typeof value === "function" ||
    (typeof value === "object" &&
      value !== null &&
      typeof (
        value as {
          handler?: unknown;
        }
      ).handler === "function")
  );
}

/**
 * Loads the Tailwind plugin adapter on first use.
 * @returns The adapter module.
 */
const tailwindAdapter = () => import("./tailwind/plugin.ts");

/**
 * Whether a loaded plugin uses the Stillwind plugin contract.
 * @param plugin - Configured or loaded plugin.
 * @returns Whether `plugin` has a `css` function, which Tailwind plugins lack.
 */
function nativePlugin(plugin: LoadedPlugin): plugin is StillwindPlugin {
  return (
    typeof plugin === "object" &&
    plugin !== null &&
    "css" in plugin &&
    typeof plugin.css === "function"
  );
}

/**
 * Host operations for plugin packages and the files that plugins read. Paths are absolute host
 * identities, which the host's `read` accepts.
 */
export interface PluginHost {
  /**
   * Lists complete specifiers for an editor without evaluating their modules.
   * @param prefix - Decoded path text before the CSS cursor.
   * @param importer - Absolute identity of the stylesheet containing the path.
   * @param root - Absolute project directory used to find package metadata.
   * @returns Full specifiers with trailing slashes for directory prefixes.
   */
  complete?(
    prefix: string,
    importer: string,
    root: string,
  ): Promise<readonly PluginPathCompletion[]>;
  /**
   * Loads the plugin package that an `@plugin` statement names when no configured plugin has
   * that name.
   *
   * @param specifier - Package specifier or path from the statement.
   * @param importer - Resolved identity of the stylesheet that contains the statement.
   * @param watch - Registers each file whose change must load the plugin again. A host that
   * returns the same plugin for unchanged files still registers them on every call.
   * @returns The plugin that the module exports by default, as {@link defaultPluginExport} checks.
   * @throws If resolution or evaluation fails, or the default export is not a plugin.
   */
  load(specifier: string, importer: string, watch: (path: string) => void): Promise<LoadedPlugin>;
  /**
   * Resolves a module request for plugin file reads and editor navigation without evaluating it.
   *
   * @param specifier - Package specifier with an optional subpath, or a relative or absolute path.
   * @param importer - Resolved identity of the stylesheet that contains the statement.
   * @returns The resolved path, or undefined when the request does not resolve.
   */
  resolve(specifier: string, importer: string): Promise<string | undefined>;
  /**
   * Reads a text file for {@link PluginContext.read}.
   *
   * @param path - Absolute file path.
   * @returns The file's text.
   * @throws If the file does not exist or cannot be read.
   */
  read(path: string): Promise<string>;
}

/**
 * Selects the plugin that a module exports by default.
 *
 * @param module - Evaluated module namespace.
 * @param specifier - Request that loaded the module, used in the error message.
 * @returns The default export.
 * @throws If the default export is neither a Stillwind plugin object nor a Tailwind v4 plugin.
 */
export function defaultPluginExport(module: unknown, specifier: string): LoadedPlugin {
  const plugin = (module as { default?: unknown } | null)?.default;
  if (isTailwindPlugin(plugin)) {
    return plugin;
  }
  if (
    typeof plugin !== "object" ||
    plugin === null ||
    typeof (plugin as StillwindPlugin).name !== "string" ||
    typeof (plugin as StillwindPlugin).css !== "function"
  ) {
    throw new Error(
      `Stillwind plugin ${specifier} must export a plugin object by default, or a Tailwind v4 handler`,
    );
  }
  return plugin as StillwindPlugin;
}

/** Number of nonempty editor inputs whose catalogs a runtime keeps. */
const CATALOG_INPUTS = 16;

/**
 * Returns the utility text that an editor input contains before its cursor, for
 * {@link PluginRuntime.catalog}.
 *
 * @param text - Text before the cursor. The candidate ends at the cursor and starts after the
 * last whitespace, quote, backtick, `;`, `{`, or `}`.
 * @returns The candidate without its variants, such as `icon-mdi--cl` for `hover:icon-mdi--cl`.
 */
export function catalogInput(text: string): string {
  const candidate = /[^\s"'`;{}]*$/.exec(text)![0];
  let depth = 0;
  for (let index = candidate.length - 1; index >= 0; index--) {
    const character = candidate[index];
    if (character === "]" || character === ")") {
      depth++;
    } else if ((character === "[" || character === "(") && depth > 0) {
      depth--;
    } else if (character === ":" && depth === 0) {
      return candidate.slice(index + 1);
    }
  }
  return candidate;
}

/** Matches `@stillwind-dynamic <prefix>*;` or `<prefix>[*];` and captures the prefix. */
const DYNAMIC = /@stillwind-dynamic\s+([^\s;*[\]]+)(?:\[\*\]|\*)\s*;/g;

/** One `@plugin` statement with its plugin, options, and the CSS answered for it. */
interface PluginInstance {
  /** Plugin that the statement selected. */
  plugin: LoadedPlugin;
  /** Configured Tailwind handler, evaluated after the complete CSS theme is available. */
  tailwind?: ReturnType<typeof configurePlugin>;
  /** Options and host operations passed to every plugin call. */
  context: PluginContext;
  /** CSS that {@link StillwindPlugin.css} returned. */
  css: string;
  /** Prefixes of the dynamic families that `css` declares. */
  prefixes: string[];
  /** Candidates passed to {@link StillwindPlugin.resolve}, including ones it left undefined. */
  candidates: string[];
  /** CSS returned by {@link StillwindPlugin.resolve}, in the order of its calls. */
  definitions: string[];
}

/** A recorded answer to one loader request, keyed by importer, specifier, and options. */
type RecordedAnswer = Omit<NativeCssFileInput, "id">;

/**
 * Runs the plugins of one CSS graph and defines candidates of their dynamic families on demand.
 *
 * The runtime records every answer of the graph's load, so {@link PluginRuntime.replay} loads the
 * same graph again without host I/O, with the definitions that plugins have added since. A
 * candidate that a plugin leaves undefined is not requested again for the runtime's lifetime.
 *
 * Tailwind plugins of the graph share one adapter runtime, which {@link PluginRuntime.initialize}
 * creates after the first load because their handlers read the complete CSS theme.
 */
export class PluginRuntime {
  /** Plugin statements keyed by importer, specifier, and options text. */
  readonly #instances = new Map<string, PluginInstance>();
  /** Recorded answers keyed by {@link answerKey}. */
  readonly #answers = new Map<string, RecordedAnswer>();
  /** Requests of the load by importer and request ID, which order the plugin statements. */
  readonly #requests = new Map<string, Map<number, NativeCssRequest>>();
  /** Root answer, recorded by {@link PluginRuntime.recordRoot}. */
  #root?: RecordedAnswer;
  /** Candidates already passed to a plugin, including ones it left undefined. */
  readonly #requested = new Set<string>();
  /** Editor catalogs keyed by input, oldest first. The empty input is never evicted. */
  readonly #catalogs = new Map<string, Promise<readonly string[]>>();
  /**
   * Number of {@link PluginRuntime.resolve} calls that added CSS. The loader of the original
   * load contains the definitions of generation zero.
   */
  #generation = 0;
  /** Runtime of every Tailwind plugin of the graph, which share one config and theme. */
  #tailwind?: TailwindRuntime;
  /** Loader of the original load, whose source IDs replays share without a session. */
  #loader?: NativeDesignSystemLoader;
  /** Tailwind statements in stylesheet order; an index is the statement's owner index. */
  #tailwindInstances: PluginInstance[] = [];
  /** Answer to the {@link SETTINGS} request, which the Tailwind runtime generates. */
  #settingsCss = "";

  /**
   * Creates a runtime for one load.
   *
   * @param options - Compiler options of the load, reused by {@link PluginRuntime.replay}.
   * @param plugins - Plugins configured for the project, which take precedence over packages.
   * @param root - Absolute project directory passed to plugins.
   * @param host - Loads plugin packages and plugin files, or undefined when the host can only use
   * configured plugins.
   * @param watch - Registers files that plugins read.
   * @param previous - Runtime of an earlier load of the same project. A statement with the same
   * specifier, options, and plugin resolves the earlier load's candidates of its families again.
   * @param settings - Project's Tailwind settings. Either field makes the graph use the Tailwind
   * plugin adapter, as a Tailwind `@plugin` statement does.
   */
  constructor(
    readonly options: NativeCompilerOptions,
    readonly plugins: readonly StillwindPlugin[],
    readonly root: string,
    readonly host: PluginHost | undefined,
    readonly watch: (path: string) => void,
    readonly previous?: PluginRuntime,
    readonly settings?: TailwindSettings,
  ) {}

  /** Number of definition batches added after the original load. */
  get generation(): number {
    return this.#generation;
  }

  /**
   * Whether the project has Tailwind settings or a loaded statement selects a Tailwind plugin, so
   * that {@link PluginRuntime.initialize} loads the graph again and the original load can skip
   * its canonical spelling index.
   */
  get tailwind(): boolean {
    return (
      this.settings?.tailwind !== undefined ||
      this.settings?.important !== undefined ||
      [...this.#instances.values()].some((instance) => instance.tailwind)
    );
  }

  /** Whether any loaded plugin declares a dynamic family. */
  get dynamic(): boolean {
    return (
      !!this.#tailwind ||
      [...this.#instances.values()].some((instance) => instance.prefixes.length > 0)
    );
  }

  /**
   * Records the root answer of the load.
   *
   * @param answer - Root input without its request ID.
   */
  recordRoot(answer: RecordedAnswer): void {
    this.#root = answer;
  }

  /**
   * Records the answer to an `@import` request so that a replay can repeat it.
   *
   * @param request - Loader request.
   * @param answer - File input supplied for it, without its request ID.
   */
  recordImport(request: NativeCssRequest, answer: RecordedAnswer): void {
    this.#recordRequest(request);
    this.#answers.set(answerKey(request), answer);
  }

  /**
   * Runs the plugin of an `@plugin` request and returns the answer for it.
   *
   * @param request - Loader request whose `pluginOptions` is set.
   * @returns File input without its request ID. Its name identifies the specifier and options.
   * @throws If no plugin has the specifier's name and the host cannot load it, or the plugin fails.
   */
  async answerPlugin(request: NativeCssRequest): Promise<RecordedAnswer> {
    this.#recordRequest(request);
    const key = instanceKey(request);
    let instance = this.#instances.get(key);
    if (!instance) {
      const plugin =
        this.plugins.find((candidate) => candidate.name === request.specifier) ??
        (await this.host?.load(request.specifier, request.importer, this.watch));
      if (!plugin) {
        throw new Error(
          `Stillwind plugin ${request.specifier} is not configured, and this host cannot load packages`,
        );
      }
      const context: PluginContext = {
        options: Object.fromEntries(
          (request.pluginOptions ?? []).map((option) => [option.name, option.value]),
        ),
        root: this.root,
        watch: this.watch,
        resolve: async (specifier) => this.host?.resolve(specifier, request.importer),
        read: async (path) => {
          if (!this.host) {
            throw new Error(`This host cannot read plugin files: ${path}`);
          }
          return this.host.read(path);
        },
      };
      const css = nativePlugin(plugin) ? await plugin.css(context) : "";
      const prefixes = [...css.matchAll(DYNAMIC)].map((match) => match[1]);
      const previous = this.previous ? this.previous.#instances.get(key) : undefined;
      // Resolving again, instead of copying the earlier definitions, reads files that changed
      // since the earlier load, such as an edited icon set.
      const candidates =
        previous?.plugin === plugin && nativePlugin(plugin) && plugin.resolve
          ? previous.candidates.filter((candidate) =>
              prefixes.some((prefix) => candidate.startsWith(prefix)),
            )
          : [];
      const resolved =
        candidates.length && nativePlugin(plugin) ? await plugin.resolve!(candidates, context) : "";
      for (const candidate of candidates) {
        this.#requested.add(candidate);
      }
      instance = {
        plugin,
        context,
        css,
        prefixes,
        candidates,
        definitions: resolved.trim() ? [resolved] : [],
        tailwind: nativePlugin(plugin)
          ? undefined
          : (await tailwindAdapter()).configurePlugin(plugin, request.pluginOptions ?? []),
      };
      this.#instances.set(key, instance);
    }
    const answer = this.#pluginAnswer(key, instance);
    this.#answers.set(answerKey(request), answer);
    return answer;
  }

  /**
   * Evaluates Tailwind handlers with the complete CSS theme, then resolves the candidates that
   * the graph's CSS applies with `@apply` and `@variant`.
   *
   * Tailwind handlers run after the first load because their `theme()` calls read the complete
   * CSS theme. Their output then needs one more load of the recorded graph, and a second one when
   * the graph's CSS uses dynamic candidates. Only the returned loader prepares its canonical
   * spelling index, which takes most of the time of a load.
   *
   * Each later load requests the Tailwind settings after the root stylesheet, as
   * {@link TailwindRuntime.settingsCss} describes.
   *
   * @param loader - Loader of the completed original load. With Tailwind plugins or settings, it
   * can skip its canonical spelling index, as {@link PluginRuntime.tailwind} allows.
   * @returns `loader` without Tailwind plugins and settings, and otherwise a loader with their CSS.
   * @throws If the Tailwind configuration is rejected or invalid, a handler or a callback fails,
   * or the native loader rejects the generated CSS.
   */
  async initialize(loader: NativeDesignSystemLoader): Promise<NativeDesignSystemLoader> {
    this.#loader = loader;
    const instances = this.#orderedInstances().filter((instance) => instance.tailwind);
    const project = await this.#projectConfig();
    if (!instances.length && !project) {
      return loader;
    }
    this.#tailwindInstances = instances;
    const { TailwindRuntime } = await tailwindAdapter();
    this.#tailwind = new TailwindRuntime(
      instances.map((instance) => instance.tailwind!),
      loader.editorQuery().themeTokens(),
      project,
    );
    if (this.previous) {
      await this.resolve(this.previous.#requested);
    }
    this.#updateTailwindCss();
    let current = this.replay(undefined, true);
    // Only `@apply` and `@variant` uses count. A scan of the whole text would also report the
    // names in definition headers and registry metadata, such as `@stillwind-utility duration-*`,
    // so every graph that imports Stillwind would load once more.
    const query = current.editorQuery();
    const sources = [
      this.#root?.content ?? "",
      ...[...this.#answers.values()].map((a) => a.content),
    ];
    if (await this.resolve(sources.flatMap((css) => query.stylesheetDynamicCandidates(css)))) {
      current = this.replay(undefined, true);
    }
    current.prepareCanonicalIndex();
    return current;
  }

  /**
   * Resolves one editor candidate in a separate loader without changing a session's used CSS.
   * @param candidate - Candidate as an editor offers it, such as `probe-4` or `state-open:`.
   * @returns Queries over a replayed design system that contains the candidate's definitions.
   * @throws If a plugin fails or the native loader rejects the generated CSS.
   */
  async query(candidate: string): Promise<NativeEditorQuery> {
    const query = this.replay(undefined, true).editorQuery();
    return (await this.resolve(query.dynamicCandidates(candidate)))
      ? this.replay(undefined, true).editorQuery()
      : query;
  }

  /**
   * Passes new candidates to the plugins that declare their families.
   *
   * @param candidates - Compiler requests for native utility names, complete Tailwind utility
   * keys, or variant names with an `@variant ` prefix. Previously requested keys are skipped.
   * @returns Whether any plugin returned CSS, so that the design system must be loaded again.
   * @throws If a plugin fails.
   */
  async resolve(candidates: Iterable<string>): Promise<boolean> {
    const pending = new Map<PluginInstance, string[]>();
    const tailwind: string[] = [];
    for (const candidate of candidates) {
      if (this.#requested.has(candidate)) {
        continue;
      }
      this.#requested.add(candidate);
      if (this.#tailwind?.accepts(candidate)) {
        tailwind.push(candidate);
      }
      for (const instance of this.#instances.values()) {
        if (
          nativePlugin(instance.plugin) &&
          instance.plugin.resolve &&
          instance.prefixes.some((prefix) => candidate.startsWith(prefix))
        ) {
          pending.set(instance, [...(pending.get(instance) ?? []), candidate]);
          break;
        }
      }
    }
    let changed = this.#tailwind?.resolve(tailwind) ?? false;
    if (changed) {
      this.#updateTailwindCss();
    }
    for (const [instance, names] of pending) {
      instance.candidates.push(...names);
      const css = nativePlugin(instance.plugin)
        ? await instance.plugin.resolve!(names, instance.context)
        : "";
      if (css.trim()) {
        instance.definitions.push(css);
        changed = true;
      }
    }
    if (changed) {
      this.#generation++;
    }
    return changed;
  }

  /**
   * Lists the candidates that editors complete for the loaded dynamic families.
   *
   * @param input - Utility text before an editor's cursor, without variants, or an empty string
   * for the candidates that editors always offer.
   * @returns Candidate names that start with `input`, from every plugin with a catalog, sorted
   * and deduplicated. Names that end with `-` are incomplete. The runtime keeps the answers for
   * the empty input and for the {@link CATALOG_INPUTS} most recent other inputs.
   */
  catalog(input = ""): Promise<readonly string[]> {
    let names = this.#catalogs.get(input);
    if (names) {
      // Moves the input to the end, so eviction removes the least recently used input.
      this.#catalogs.delete(input);
    } else {
      names = Promise.all(
        [...this.#instances.values()].map(async (instance): Promise<readonly string[]> =>
          nativePlugin(instance.plugin) && instance.plugin.catalog
            ? await instance.plugin.catalog(instance.context, input)
            : [],
        ),
      ).then((lists) =>
        [...new Set([...lists.flat(), ...(this.#tailwind?.catalog() ?? [])])]
          .filter((name) => name.startsWith(input))
          .sort(),
      );
    }
    this.#catalogs.set(input, names);
    for (const key of this.#catalogs.keys()) {
      if (this.#catalogs.size <= CATALOG_INPUTS + 1) {
        break;
      }
      if (key) {
        this.#catalogs.delete(key);
      }
    }
    return names;
  }

  /**
   * Loads the recorded graph again into a loader of `session`, with the current definitions.
   *
   * @param session - Session whose source IDs the new loader shares. Omission shares the
   * original loader's source IDs.
   * @param intermediate - Skips preparing the canonical spelling index for a loader that only
   * serves a later load or a short query. Canonical queries still prepare it on first use.
   * @returns A complete loader with the load's compiler options, for
   * {@link NativeProjectSession.reload}.
   * @throws If the recorded graph is incomplete or the native loader rejects it.
   */
  replay(session?: NativeProjectSession, intermediate = false): NativeDesignSystemLoader {
    if (!this.#root) {
      throw new Error("Stillwind plugin runtime has no recorded CSS graph");
    }
    const loader = session
      ? session.newLoader(this.options)
      : (this.#loader?.newLoader(this.options) ?? new NativeDesignSystemLoader(this.options));
    if (intermediate) {
      loader.deferCanonicalIndex();
    }
    if (this.#tailwind) {
      loader.requestTrailingPlugin(SETTINGS);
    }
    let step: NativeCssLoadStep = loader.provide(
      0,
      this.#root.name,
      this.#root.content,
      this.#root.uri,
      this.#root.sourceMapSource,
      this.#root.lint,
    );
    while (step.kind !== "complete") {
      step = loader.provideMany(
        step.requests.map((request) => {
          if (
            this.#tailwind &&
            request.specifier === SETTINGS &&
            request.pluginOptions !== undefined
          ) {
            return {
              id: request.id,
              name: `stillwind-plugin:${SETTINGS.slice(1)}#${this.#generation}`,
              content: this.#settingsCss,
            };
          }
          const instance =
            request.pluginOptions !== undefined
              ? this.#instances.get(instanceKey(request))
              : undefined;
          const answer = instance
            ? this.#pluginAnswer(instanceKey(request), instance)
            : this.#answers.get(answerKey(request));
          if (!answer) {
            throw new Error(`Stillwind plugin runtime has no answer for ${request.specifier}`);
          }
          return {
            id: request.id,
            ...answer,
          };
        }),
      );
    }
    return loader;
  }

  /**
   * Returns the answer to a plugin request with the instance's current definitions.
   *
   * @param key - Importer, specifier, and options text of the instance.
   * @param instance - Loaded plugin statement.
   * @returns An internal buffer whose name changes with its content.
   */
  #pluginAnswer(key: string, instance: PluginInstance): RecordedAnswer {
    return {
      name: `stillwind-plugin:${key.replaceAll("\0", "#")}#${this.#generation}#${instance.definitions.length}`,
      content: [instance.css, ...instance.definitions].join("\n"),
    };
  }

  /** Stores the generated CSS of each Tailwind statement and of the settings as their answers. */
  #updateTailwindCss(): void {
    for (const [owner, instance] of this.#tailwindInstances.entries()) {
      instance.css = this.#tailwind!.css(owner);
    }
    this.#settingsCss = this.#tailwind!.settingsCss();
  }

  /**
   * Awaits the project's Tailwind configuration and applies its `important` field.
   * @returns The configuration with `important` applied, or undefined without settings.
   * @throws If the configuration promise rejects, its value is not an object, or both the field
   * and the configuration set `important`.
   */
  async #projectConfig(): Promise<TailwindConfig | undefined> {
    const { important } = this.settings ?? {};
    const tailwind: unknown = await this.settings?.tailwind;
    if (tailwind === undefined) {
      return important === undefined
        ? undefined
        : {
            important,
          };
    }
    if (typeof tailwind !== "object" || tailwind === null || Array.isArray(tailwind)) {
      throw new Error("Stillwind config tailwind must resolve to a Tailwind configuration object");
    }
    const config = tailwind as TailwindConfig;
    if (important === undefined) {
      return config;
    }
    if (config.important !== undefined) {
      throw new Error(
        "Stillwind config important must not be set together with tailwind.important",
      );
    }
    return {
      ...config,
      important,
    };
  }

  /**
   * Records a request of the load, which orders the plugin statements.
   * @param request - Import or plugin request.
   */
  #recordRequest(request: NativeCssRequest): void {
    let requests = this.#requests.get(request.importer);
    if (!requests) {
      this.#requests.set(request.importer, (requests = new Map()));
    }
    requests.set(request.id, request);
  }

  /**
   * Orders the plugin statements as the import-expanded stylesheet does.
   * @returns Each statement once, depth first from the root in request order.
   */
  #orderedInstances(): PluginInstance[] {
    const result = new Set<PluginInstance>();
    const visited = new Set<string>();
    /**
     * Adds the statements of one stylesheet and of the stylesheets it imports.
     * @param name - Recorded identity of the stylesheet.
     */
    const visit = (name: string) => {
      if (visited.has(name)) {
        return;
      }
      visited.add(name);
      for (const request of [...(this.#requests.get(name)?.values() ?? [])].sort(
        (a, b) => a.id - b.id,
      )) {
        const instance = this.#instances.get(instanceKey(request));
        if (request.pluginOptions !== undefined && instance) {
          result.add(instance);
        } else {
          const answer = this.#answers.get(answerKey(request));
          if (answer) {
            visit(answer.name);
          }
        }
      }
    };
    if (this.#root) {
      visit(this.#root.name);
    }
    return [...result];
  }
}

/**
 * Identifies one `@plugin` statement independently of its request ID.
 * @param request - Plugin request.
 * @returns Importer, specifier, and JSON options joined by NUL characters.
 */
function instanceKey(request: NativeCssRequest): string {
  return `${request.importer}\0${request.specifier}\0${JSON.stringify(request.pluginOptions ?? [])}`;
}

/**
 * Identifies one request independently of its ID, which changes between loads.
 *
 * @param request - Loader request.
 * @returns Importer, specifier, and JSON plugin options joined by NUL characters. An `@import`
 * has no options part.
 */
function answerKey(request: NativeCssRequest): string {
  return `${request.importer}\0${request.specifier}\0${JSON.stringify(request.pluginOptions) ?? ""}`;
}

/** A session's plugin runtime and the definition generation that its design system contains. */
interface SessionPlugins {
  /** Runtime of the CSS graph that the session was created or reloaded from. */
  runtime: PluginRuntime;
  /** Value of {@link PluginRuntime.generation} that the session's design system contains. */
  generation: number;
  /** Loader of the session's design system after the last reload with definitions. */
  loader?: NativeDesignSystemLoader;
}

/** Runtimes of sessions whose design systems load plugins with dynamic families. */
const sessions = new WeakMap<NativeProjectSession, SessionPlugins>();

/**
 * Associates a session with the plugin runtime of the CSS graph that it was created or reloaded
 * from, so that compilation can define dynamic candidates.
 *
 * @param session - Native session.
 * @param runtime - Runtime returned with the graph, or undefined to detach.
 * @param generation - Definition generation of the loader that the session uses. Defaults to
 * zero, the generation of the graph's original loader.
 */
export function attachPlugins(
  session: NativeProjectSession,
  runtime?: PluginRuntime,
  generation = 0,
): void {
  if (runtime?.dynamic) {
    sessions.set(session, {
      runtime,
      generation,
    });
  } else {
    sessions.delete(session);
  }
}

/**
 * Compiles with the definitions of the attached plugins, and defines the dynamic candidates that
 * the compilation reports.
 *
 * A session whose design system lacks definitions that the runtime added later is reloaded
 * first. After each compilation, new candidates go to the plugins, and when they return CSS the
 * session is reloaded and `compile` runs again, at most `rounds` more times. A session without
 * plugins runs `compile` once.
 *
 * @param session - Native session whose output `compile` returns.
 * @param compile - Compiles the graph into the session and returns its module outputs.
 * @param rounds - Most reloads after the first compilation.
 * @returns The output of the last compilation.
 * @throws If a plugin, a reload, or `compile` fails.
 */
export async function compileWithPlugins<
  Output extends {
    modules: readonly {
      dynamicCandidates: readonly string[];
    }[];
  },
>(session: NativeProjectSession, compile: () => Output, rounds: number): Promise<Output> {
  const binding = sessions.get(session);
  if (!binding) {
    return compile();
  }
  /** Reloads the session with the runtime's current definitions. */
  const sync = () => {
    const loader = binding.runtime.replay(session);
    session.reload(loader);
    binding.generation = binding.runtime.generation;
    binding.loader = loader;
  };
  if (binding.generation < binding.runtime.generation) {
    sync();
  }
  let output = compile();
  for (let round = 0; round < rounds; round++) {
    const candidates = output.modules.flatMap((module) => module.dynamicCandidates);
    if (!(await binding.runtime.resolve(candidates))) {
      break;
    }
    sync();
    output = compile();
  }
  return output;
}

/**
 * Returns the loader whose design system a session uses after plugin definitions were added.
 *
 * @param session - Native session.
 * @returns The loader of the last reload with definitions, or undefined when the session still
 * uses the loader that it was created or attached with.
 */
export function pluginLoader(session: NativeProjectSession): NativeDesignSystemLoader | undefined {
  return sessions.get(session)?.loader;
}
