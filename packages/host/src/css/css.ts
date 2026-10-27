import type { StillwindPlugin } from "@stillwind/config";
import {
  editorCssSource,
  NativeDesignSystemLoader,
  type NativeCompilerOptions,
  type NativeCssFileInput,
  type NativeCssLoadStep,
  type NativeCssResource,
  type NativeSkippedCss,
} from "@stillwind/native";

import type { PluginHost, TailwindSettings } from "./plugins/host.ts";
import { PluginRuntime } from "./plugins/runtime.ts";

/**
 * Checks whether an identity names a stylesheet.
 * @param id - Resolver identity or file path.
 * @returns Whether the complete identity ends in `.css`, ignoring case. The check includes any
 * query suffix.
 */
export function isStylesheet(id: string): boolean {
  return /\.css$/i.test(id);
}

/** A CSS graph for interactive queries. Recovered text must never supply lint fixes or builds. */
export interface EditorCssGraph extends Omit<LoadedCssGraph, "skipped"> {
  /** Whether incomplete sources were replaced with recovered definitions. */
  recovered: boolean;
}

/**
 * Loads original CSS, then retries with recovered definitions if that load fails.
 * @param root - Resolved CSS entrypoint.
 * @param options - Compiler policies used by semantic queries.
 * @param host - Resolver and reads of the captured original buffers.
 * @param pluginInputs - Plugins for `@plugin` statements, as {@link loadCssGraph} describes.
 * @returns A query graph and whether its sources were recovered. Original buffers remain unchanged.
 * The recovered graph must only supply editor queries, because its text omits authored rules.
 * @throws If both loads fail, including unresolved imports or unreadable files.
 */
export async function loadEditorCssGraph(
  root: string,
  options: NativeCompilerOptions,
  host: CssHost,
  pluginInputs?: CssPluginInputs,
): Promise<EditorCssGraph> {
  try {
    return {
      ...(await loadCssGraph(root, options, host, undefined, undefined, pluginInputs)),
      recovered: false,
    };
  } catch {
    return {
      ...(await loadCssGraph(
        root,
        options,
        {
          ...host,
          read: async (id) => editorCssSource(await host.read(id)),
        },
        undefined,
        undefined,
        pluginInputs,
      )),
      recovered: true,
    };
  }
}

/**
 * Host operations that load the CSS graph and preserve document and source-map identities. Resolved
 * IDs must be accepted by the reader, identity functions, and watcher.
 */
export interface CssHost {
  /**
   * Resolves an import from one CSS file.
   *
   * @param specifier - Import request text from the stylesheet.
   * @param importer - Resolved identity of the importing stylesheet.
   * @returns A promise for a file identity accepted by the other host operations. Resolution
   * failures reject.
   */
  resolve(specifier: string, importer: string): Promise<string>;
  /**
   * Reads the text of a resolved CSS file.
   *
   * @param id - Identity returned by the resolver, or the resolved root identity.
   * @returns A promise for stylesheet text. Read failures reject.
   */
  read(id: string): Promise<string>;
  /**
   * Assigns the document identity used by compiler diagnostics.
   *
   * @param id - Resolved CSS file identity.
   * @returns A stable document URI for this file.
   */
  documentUri(id: string): string;
  /**
   * Assigns the source name recorded in CSS source maps.
   *
   * @param id - Resolved CSS file identity.
   * @returns The source identity used for mapping emitted CSS back to this file.
   */
  sourceMapSource(id: string): string;
  /**
   * Registers a resolved CSS file for dependency tracking.
   *
   * @param id - Root or dependency identity accepted by the host reader.
   */
  watch(id: string): void;
  /**
   * Registers a directory that a plugin lists, so that adding, removing, or renaming an entry
   * reloads the graph. Omission ignores directory registrations, which suits a host that loads
   * the graph once or reloads it after every file addition or removal.
   *
   * @param id - Absolute directory path.
   */
  watchDirectory?(id: string): void;
  /**
   * Decides whether stylesheet lint checks a file. Omission selects {@link lintsStylesheet}.
   *
   * @param id - Resolved CSS file identity.
   * @param importer - Identity of the importing file and whether lint checks it, or undefined for
   * the root.
   * @param specifier - Import request text, or undefined for the root.
   * @returns Whether to mark the file for stylesheet lint. Lint never changes compiled output.
   */
  lint?(
    id: string,
    importer?: {
      /** Resolved identity of the importing file. */
      id: string;
      /** Whether lint checks the importing file. */
      lint: boolean;
    },
    specifier?: string,
  ): boolean;
  /**
   * Loads plugin packages and the files that plugins read. Omission limits the graph to
   * configured plugins, whose `resolve` finds nothing and whose `read` fails.
   */
  plugins?: PluginHost;
}

/** Plugins, Tailwind settings, and project inputs for the `@plugin` statements of one CSS graph. */
export interface CssPluginInputs extends TailwindSettings {
  /** Plugins configured for the project. */
  plugins?: readonly StillwindPlugin[];
  /** Absolute project directory passed to plugins. */
  root: string;
  /** Runtime of an earlier load of the same project, whose candidates are resolved again. */
  previous?: PluginRuntime;
}

/**
 * Selects the user's own CSS for stylesheet lint. The root is linted unless it lies under
 * `node_modules`. An import is linted when lint checks its importer, its specifier is a relative
 * or absolute path, and its file does not lie under `node_modules`. A package specifier such as
 * `stillwind` or `tailwindcss` therefore excludes the package and everything it imports, even when
 * the package resolves outside `node_modules`.
 *
 * @param id - Resolved CSS file identity.
 * @param importer - Importing file and its lint mark, or undefined for the root.
 * @param specifier - Import request text, or undefined for the root.
 * @returns Whether stylesheet lint checks the file.
 */
export function lintsStylesheet(
  id: string,
  importer?: {
    /** Resolved identity of the importing file. */
    id: string;
    /** Whether lint checks the importing file. */
    lint: boolean;
  },
  specifier?: string,
): boolean {
  if (/[\\/]node_modules[\\/]/.test(id)) {
    return false;
  }
  return (
    !importer ||
    (importer.lint &&
      specifier !== undefined &&
      /^(?:\.{1,2}[\\/]|[\\/]|[a-z]:[\\/]|file:)/i.test(specifier))
  );
}

/** Completed design-system loader and resource references discovered in its CSS graph. */
export interface LoadedCssGraph {
  /** Native loader whose complete CSS graph is ready for compilation. */
  loader: NativeDesignSystemLoader;
  /** Authored resource references that the adapter must resolve and replace in emitted CSS. */
  resources: NativeCssResource[];
  /** Identities of the files marked for stylesheet lint, in the order the loader accepted them. */
  lintSources: string[];
  /**
   * Statements that loading skipped instead of failing, such as an import that cannot be resolved,
   * ordered by source and position. Hosts that do not run stylesheet lint report them as warnings.
   */
  skipped: NativeSkippedCss[];
  /**
   * Plugins of the graph's `@plugin` statements and the definitions that they have added. Absent
   * for a graph that a host assembled without loading plugins.
   */
  plugins?: PluginRuntime;
}

/**
 * Loads a CSS graph and returns the completed native loader and its resource references.
 * `root` must already be resolved and accepted by the host's read and watch methods.
 * `options` configures a new loader. If `suppliedLoader` is given, it must not have
 * received any input yet, and this function uses its existing configuration.
 * `rootContent` skips reading the root file. The function still watches that file
 * and all resolved dependencies. Each file is marked for stylesheet lint as `host.lint` decides,
 * or as {@link lintsStylesheet} decides when the host omits it.
 * Canonical queries build their indexes on first use.
 *
 * Reads run concurrently, and each request ID is read only once. Responses ready
 * in the same event-loop turn are sent together to reduce native calls. Faster reads
 * can reveal more imports without waiting for slower reads.
 * Resolution, read, and native loading errors reject the promise. Reads already
 * started continue, and files already registered with the watcher stay registered.
 *
 * @param root - Resolved CSS entrypoint accepted by the host's read and watch operations.
 * @param options - Compiler settings used when creating a new loader.
 * @param host - CSS resolver, reader, source identity operations, and dependency watcher.
 * @param rootContent - Optional entrypoint text that replaces the initial filesystem read.
 * @param suppliedLoader - Optional configured loader that has not received any CSS input.
 * @param pluginInputs - Plugins for `@plugin` statements. Omission uses no configured plugins,
 * the root's directory as the project directory, and the host's plugin operations.
 * @returns A promise for the completed native loader and its authored resource references.
 * @throws If resolving, reading, or loading the CSS graph fails.
 */
export async function loadCssGraph(
  root: string,
  options: NativeCompilerOptions,
  host: CssHost,
  rootContent?: string,
  suppliedLoader?: NativeDesignSystemLoader,
  pluginInputs?: CssPluginInputs,
): Promise<LoadedCssGraph> {
  const loader = suppliedLoader ?? new NativeDesignSystemLoader(options);
  loader.deferCanonicalIndex();

  const plugins = new PluginRuntime(
    options,
    pluginInputs?.plugins ?? [],
    pluginInputs?.root ?? root.replace(/[\\/][^\\/]*$/, ""),
    host.plugins,
    (path, kind): void => {
      if (kind === "directory") {
        host.watchDirectory?.(path);
      } else {
        host.watch(path);
      }
    },
    pluginInputs?.previous,
    pluginInputs && {
      tailwind: pluginInputs.tailwind,
      important: pluginInputs.important,
    },
  );

  host.watch(root);

  const lints = new Map<string, boolean>();
  const lint: typeof lintsStylesheet = (id, importer, specifier) =>
    host.lint ? host.lint(id, importer, specifier) : lintsStylesheet(id, importer, specifier);
  lints.set(root, lint(root));
  const rootAnswer = {
    name: root,
    content: rootContent ?? (await host.read(root)),
    uri: host.documentUri(root),
    sourceMapSource: host.sourceMapSource(root),
    lint: lints.get(root),
  };
  plugins.recordRoot(rootAnswer);
  const initial = loader.provide(
    0,
    rootAnswer.name,
    rootAnswer.content,
    rootAnswer.uri,
    rootAnswer.sourceMapSource,
    rootAnswer.lint,
  );

  let completion:
    | Extract<
        NativeCssLoadStep,
        {
          /** Native loading-step discriminator whose resources are available after graph completion. */
          kind: "complete";
        }
      >
    | undefined;
  await new Promise<void>((resolveLoad, rejectLoad) => {
    const started = new Set<number>();
    let stopped = false;
    let queued = false;
    let ready: NativeCssFileInput[] = [];
    /**
     * Stops graph advancement and rejects the pending load.
     * @param error - Resolution, reading, or native loading failure passed to the caller.
     */
    const fail = (error: unknown) => {
      stopped = true;
      rejectLoad(error);
    };
    /**
     * Completes the graph or starts reads for requests that have not already started.
     * @param step - Native loading step. Completion stores resources and resolves the load.
     */
    const advance = (step: NativeCssLoadStep) => {
      if (step.kind === "complete") {
        completion = step;
        stopped = true;
        resolveLoad();
        return;
      }
      for (const request of step.requests) {
        // NeedFile repeats outstanding IDs, including reads already in flight.
        if (started.has(request.id)) {
          continue;
        }
        started.add(request.id);
        /**
         * Resolves, watches, and reads this request, then schedules a batch of ready responses.
         * An import that the host cannot resolve or read is answered as unresolved.
         * @returns A promise that resolves after queuing the response, or after observing a stopped load.
         * @throws If a plugin fails to load. The attached rejection handler stops the load.
         */
        const read = async () => {
          if (request.pluginOptions !== undefined) {
            const answer = await plugins.answerPlugin(request);
            if (plugins.tailwind) {
              // `PluginRuntime.initialize` loads the graph again, so this load only provides its
              // theme and recorded answers.
              loader.deferCanonicalIndex();
            }
            if (!stopped) {
              ready.push({
                id: request.id,
                ...answer,
              });
              schedule();
            }
            return;
          }
          /**
           * Answers the request as unresolved, so the loader skips the import and stylesheet lint
           * reports it.
           */
          const unresolved = () => {
            if (stopped) {
              return;
            }
            const answer = {
              name: request.specifier,
              content: "",
              lint: false,
              unresolved: true,
            };
            plugins.recordImport(request, answer);
            ready.push({ id: request.id, ...answer });
            schedule();
          };
          let name: string;
          try {
            name = await host.resolve(request.specifier, request.importer);
          } catch {
            unresolved();
            return;
          }
          if (stopped) {
            return;
          }
          host.watch(name);
          let content: string;
          try {
            content = await host.read(name);
          } catch {
            unresolved();
            return;
          }
          if (stopped) {
            return;
          }
          const marked = lint(
            name,
            {
              id: request.importer,
              lint: lints.get(request.importer) ?? false,
            },
            request.specifier,
          );
          lints.set(name, (lints.get(name) ?? false) || marked);
          const answer = {
            name,
            content,
            uri: host.documentUri(name),
            sourceMapSource: host.sourceMapSource(name),
            lint: marked,
          };
          plugins.recordImport(request, answer);
          ready.push({ id: request.id, ...answer });
          schedule();
        };
        // Handle every rejection, including reads finishing after another read failed.
        void read().catch(fail);
      }
    };
    /** Sends the responses that are ready in the current event-loop turn together. */
    const schedule = () => {
      if (!queued) {
        queued = true;
        setTimeout(() => {
          queued = false;
          const responses = ready;
          ready = [];
          if (stopped) {
            return;
          }
          try {
            advance(loader.provideMany(responses));
          } catch (error) {
            fail(error);
          }
        });
      }
    };
    advance(initial);
  });

  return {
    loader: await plugins.initialize(loader),
    resources: completion?.resources ?? [],
    lintSources: completion?.lintSources ?? [],
    skipped: completion?.skipped ?? [],
    plugins,
  };
}
