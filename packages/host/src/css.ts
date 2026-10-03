import { setImmediate } from "node:timers";

import {
  NativeDesignSystemLoader,
  type NativeCompilerOptions,
  type NativeCssFileInput,
  type NativeCssLoadStep,
} from "@stillwind/native";

/**
 * The host resolves CSS imports, reads their text, and watches files.
 * Resolved identifiers must be accepted by read and watch.
 */
export interface CssHost {
  resolve(specifier: string, importer: string): Promise<string>;
  read(id: string): Promise<string>;
  documentUri(id: string): string;
  sourceMapSource(id: string): string;
  watch(id: string): void;
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
 */
export async function loadCssGraph(
  root: string,
  options: NativeCompilerOptions,
  host: CssHost,
  rootContent?: string,
  suppliedLoader?: NativeDesignSystemLoader,
) {
  const loader = suppliedLoader ?? new NativeDesignSystemLoader(options);
  host.watch(root);
  type LoadStep = NativeCssLoadStep;
  let resourceReferences: Extract<LoadStep, { kind: "complete" }>["resources"] = [];
  const initial = loader.provide(
    0,
    root,
    rootContent ?? (await host.read(root)),
    host.documentUri(root),
    host.sourceMapSource(root),
  );
  await new Promise<void>((resolveLoad, rejectLoad) => {
    const started = new Set<number>();
    let stopped = false;
    let queued = false;
    let ready: NativeCssFileInput[] = [];
    const fail = (error: unknown) => {
      stopped = true;
      rejectLoad(error);
    };
    const advance = (step: LoadStep) => {
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
            setImmediate(() => {
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
  return { loader, resources: resourceReferences };
}
