import {
  NativeDesignSystemLoader,
  type NativeCompilerOptions,
  type NativeCssFileInput,
  type NativeCssLoadStep,
  type NativeCssResource,
} from "@stillwind/native";

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
}

/** Completed design-system loader and resource references discovered in its CSS graph. */
export interface LoadedCssGraph {
  /** Native loader whose complete CSS graph is ready for compilation. */
  loader: NativeDesignSystemLoader;
  /** Authored resource references that the adapter must resolve and replace in emitted CSS. */
  resources: NativeCssResource[];
}

/**
 * Loads a CSS graph and returns the completed native loader and its resource references.
 * `root` must already be resolved and accepted by the host's read and watch methods.
 * `options` configures a new loader. If `suppliedLoader` is given, it must not have
 * received any input yet, and this function uses its existing configuration.
 * `rootContent` skips reading the root file. The function still watches that file
 * and all resolved dependencies.
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
 * @returns A promise for the completed native loader and its authored resource references.
 * @throws If resolving, reading, or loading the CSS graph fails.
 */
export async function loadCssGraph(
  root: string,
  options: NativeCompilerOptions,
  host: CssHost,
  rootContent?: string,
  suppliedLoader?: NativeDesignSystemLoader,
): Promise<LoadedCssGraph> {
  const loader = suppliedLoader ?? new NativeDesignSystemLoader(options);

  host.watch(root);

  const initial = loader.provide(
    0,
    root,
    rootContent ?? (await host.read(root)),
    host.documentUri(root),
    host.sourceMapSource(root),
  );

  let resourceReferences: Extract<
    NativeCssLoadStep,
    {
      /** Native loading-step discriminator whose resources are available after graph completion. */
      kind: "complete";
    }
  >["resources"] = [];
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
        resourceReferences = step.resources;
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
         * @returns A promise that resolves after queuing the response, or after observing a stopped load.
         * @throws If host resolution or reading fails. The attached rejection handler stops the load.
         */
        const read = async () => {
          const name = await host.resolve(request.specifier, request.importer);
          if (stopped) {
            return;
          }
          host.watch(name);
          const content = await host.read(name);
          if (stopped) {
            return;
          }
          ready.push({
            id: request.id,
            name,
            content,
            uri: host.documentUri(name),
            sourceMapSource: host.sourceMapSource(name),
          });
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
        // Handle every rejection, including reads finishing after another read failed.
        void read().catch(fail);
      }
    };
    advance(initial);
  });

  return {
    loader,
    resources: resourceReferences,
  };
}
