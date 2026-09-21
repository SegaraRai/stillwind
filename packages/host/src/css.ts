import { NativeDesignSystemLoader } from "@stillwind/native";
import type { NativeCompilerOptions, NativeCssLoadStep } from "@stillwind/native";
/**
 * Host-owned CSS resolution, text loading, and dependency watch registration.
 * Resolved identifiers must be accepted by read and watch.
 */
export interface CssHost {
  resolve(specifier: string, importer: string): Promise<string>;
  read(id: string): Promise<string>;
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
) {
  const loader = new NativeDesignSystemLoader(options);
  host.watch(root);
  type LoadStep = NativeCssLoadStep;
  let resourceReferences: Extract<LoadStep, { kind: "complete" }>["resources"] = [];
  const initial = loader.provide(0, root, rootContent ?? (await host.read(root)));
  await new Promise<void>((resolveLoad, rejectLoad) => {
    const started = new Set<number>();
    let stopped = false;
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
          // JS callbacks serialize provide/step processing; I/O remains concurrent.
          advance(loader.provide(request.id, name, content));
        };
        // Handle every rejection, including reads finishing after another read failed.
        void read().catch(fail);
      }
    };
    advance(initial);
  });
  return { loader, resources: resourceReferences };
}
