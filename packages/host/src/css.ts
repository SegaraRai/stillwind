import { setImmediate } from "node:timers";

import {
  NativeDesignSystemLoader,
  type NativeCompilerOptions,
  type NativeCssFileInput,
  type NativeCssLoadStep,
} from "@stillwind/native";

/**
 * Host-owned CSS resolution, text loading, and dependency watch registration.
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
 * Loads a CSS dependency graph through the native loader and returns its completed
 * loader and resources. rootContent, when supplied, replaces only the initial read;
 * the root and resolved dependencies are still registered with the host watcher.
 *
 * Pending requests are loaded concurrently and each request id is scheduled once.
 * Resolution, reading, or native loading failures reject the operation. In-flight
 * reads are not cancelled, and watches already registered remain registered.
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
