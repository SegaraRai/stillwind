import { parsePluginOptions, type PluginContext, type StillwindPlugin } from "@stillwind/config";
import type {
  NativeCompilerOptions,
  NativeCssFileInput,
  NativeCssLoadStep,
  NativeCssRequest,
  NativeDesignSystemLoader,
  NativeProjectSession,
} from "@stillwind/native";

/**
 * Host operations for plugin packages and the files that plugins read. Paths are absolute host
 * identities, which the host's `read` accepts.
 */
export interface PluginHost {
  /**
   * Loads the plugin package that an `@plugin` statement names when no configured plugin has
   * that name.
   *
   * @param specifier - Package specifier or path from the statement.
   * @param importer - Resolved identity of the stylesheet that contains the statement.
   * @returns The plugin that the module exports by default, as {@link defaultPluginExport} checks.
   * @throws If resolution or evaluation fails, or the default export is not a plugin.
   */
  load(specifier: string, importer: string): Promise<StillwindPlugin>;
  /**
   * Resolves a module request for {@link PluginContext.resolve}.
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
 * @throws If the default export is not an object with a string `name` and a `css` function.
 */
export function defaultPluginExport(module: unknown, specifier: string): StillwindPlugin {
  const plugin = (module as { default?: unknown } | null)?.default;
  if (
    typeof plugin !== "object" ||
    plugin === null ||
    typeof (plugin as StillwindPlugin).name !== "string" ||
    typeof (plugin as StillwindPlugin).css !== "function"
  ) {
    throw new Error(`Stillwind plugin ${specifier} must export a plugin object by default`);
  }
  return plugin as StillwindPlugin;
}

/** Matches `@stillwind-dynamic <prefix>*;` or `<prefix>[*];` and captures the prefix. */
const DYNAMIC = /@stillwind-dynamic\s+([^\s;*[\]]+)(?:\[\*\]|\*)\s*;/g;

/** One `@plugin` statement with its plugin, options, and the CSS answered for it. */
interface PluginInstance {
  /** Plugin that the statement selected. */
  plugin: StillwindPlugin;
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
 */
export class PluginRuntime {
  /** Plugin statements keyed by specifier and options text, in load order. */
  readonly #instances = new Map<string, PluginInstance>();
  /** Recorded answers keyed by {@link answerKey}. */
  readonly #answers = new Map<string, RecordedAnswer>();
  /** Root answer, recorded by {@link PluginRuntime.recordRoot}. */
  #root?: RecordedAnswer;
  /** Candidates already passed to a plugin, including ones it left undefined. */
  readonly #requested = new Set<string>();
  /** Editor catalogs of each instance, loaded on first use. */
  #catalog?: Promise<readonly string[]>;
  /**
   * Number of {@link PluginRuntime.resolve} calls that added CSS. The loader of the original
   * load contains the definitions of generation zero.
   */
  #generation = 0;

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
   */
  constructor(
    readonly options: NativeCompilerOptions,
    readonly plugins: readonly StillwindPlugin[],
    readonly root: string,
    readonly host: PluginHost | undefined,
    readonly watch: (path: string) => void,
    readonly previous?: PluginRuntime,
  ) {}

  /** Number of definition batches added after the original load. */
  get generation(): number {
    return this.#generation;
  }

  /** Whether any loaded plugin declares a dynamic family. */
  get dynamic(): boolean {
    return [...this.#instances.values()].some((instance) => instance.prefixes.length > 0);
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
    const key = `${request.specifier}\0${request.pluginOptions ?? ""}`;
    let instance = this.#instances.get(key);
    if (!instance) {
      const plugin =
        this.plugins.find((candidate) => candidate.name === request.specifier) ??
        (await this.host?.load(request.specifier, request.importer));
      if (!plugin) {
        throw new Error(
          `Stillwind plugin ${request.specifier} is not configured, and this host cannot load packages`,
        );
      }
      const context: PluginContext = {
        options: parsePluginOptions(request.pluginOptions ?? ""),
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
      const css = await plugin.css(context);
      const prefixes = [...css.matchAll(DYNAMIC)].map((match) => match[1]);
      const previous = this.previous ? this.previous.#instances.get(key) : undefined;
      // Resolving again, instead of copying the earlier definitions, reads files that changed
      // since the earlier load, such as an edited icon set.
      const candidates =
        previous?.plugin === plugin && plugin.resolve
          ? previous.candidates.filter((candidate) =>
              prefixes.some((prefix) => candidate.startsWith(prefix)),
            )
          : [];
      const resolved = candidates.length ? await plugin.resolve!(candidates, context) : "";
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
      };
      this.#instances.set(key, instance);
    }
    const answer = this.#pluginAnswer(key, instance);
    this.#answers.set(answerKey(request), answer);
    return answer;
  }

  /**
   * Passes new candidates to the plugins that declare their families.
   *
   * @param candidates - Named utility spellings reported by compilation. Duplicates and
   * candidates already requested are skipped.
   * @returns Whether any plugin returned CSS, so that the design system must be loaded again.
   * @throws If a plugin fails.
   */
  async resolve(candidates: Iterable<string>): Promise<boolean> {
    const pending = new Map<PluginInstance, string[]>();
    for (const candidate of candidates) {
      if (this.#requested.has(candidate)) {
        continue;
      }
      this.#requested.add(candidate);
      for (const instance of this.#instances.values()) {
        if (
          instance.plugin.resolve &&
          instance.prefixes.some((prefix) => candidate.startsWith(prefix))
        ) {
          pending.set(instance, [...(pending.get(instance) ?? []), candidate]);
          break;
        }
      }
    }
    let changed = false;
    for (const [instance, names] of pending) {
      instance.candidates.push(...names);
      const css = await instance.plugin.resolve!(names, instance.context);
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
   * @returns Candidate names from every plugin with a catalog, sorted and deduplicated. The
   * catalog is loaded once per runtime.
   */
  catalog(): Promise<readonly string[]> {
    this.#catalog ??= Promise.all(
      [...this.#instances.values()].map(async (instance): Promise<readonly string[]> =>
        instance.plugin.catalog ? await instance.plugin.catalog(instance.context) : [],
      ),
    ).then((lists) => [...new Set(lists.flat())].sort());
    return this.#catalog;
  }

  /**
   * Loads the recorded graph again into a loader of `session`, with the current definitions.
   *
   * @param session - Session whose source IDs the new loader shares.
   * @returns A complete loader with the load's compiler options, for
   * {@link NativeProjectSession.reload}.
   * @throws If the recorded graph is incomplete or the native loader rejects it.
   */
  replay(session: NativeProjectSession): NativeDesignSystemLoader {
    if (!this.#root) {
      throw new Error("Stillwind plugin runtime has no recorded CSS graph");
    }
    const loader = session.newLoader(this.options);
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
          const instance =
            request.pluginOptions !== undefined
              ? this.#instances.get(`${request.specifier}\0${request.pluginOptions}`)
              : undefined;
          const answer = instance
            ? this.#pluginAnswer(`${request.specifier}\0${request.pluginOptions}`, instance)
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
   * @param key - Specifier and options text of the instance.
   * @param instance - Loaded plugin statement.
   * @returns An internal buffer whose name changes with its content.
   */
  #pluginAnswer(key: string, instance: PluginInstance): RecordedAnswer {
    return {
      name: `stillwind-plugin:${key.replace("\0", "#")}#${instance.definitions.length}`,
      content: [instance.css, ...instance.definitions].join("\n"),
    };
  }
}

/**
 * Identifies one request independently of its ID, which changes between loads.
 *
 * @param request - Loader request.
 * @returns Importer, specifier, and plugin options joined by NUL characters.
 */
function answerKey(request: NativeCssRequest): string {
  return `${request.importer}\0${request.specifier}\0${request.pluginOptions ?? "\u0001"}`;
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
