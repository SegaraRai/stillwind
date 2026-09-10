import { NativeDesignSystemLoader } from "@stillwind/native";
export interface CssHost {
  resolve(specifier: string, importer: string): Promise<string>;
  read(id: string): Promise<string>;
  watch(id: string): void;
}
export async function loadCssGraph(
  root: string,
  optionsJson: string,
  host: CssHost,
  rootContent?: string,
) {
  const loader = new NativeDesignSystemLoader(optionsJson);
  host.watch(root);
  type LoadStep =
    | { kind: "needFile"; requests: { id: number; importer: string; specifier: string }[] }
    | {
        kind: "complete";
        resources: { placeholder: string; owner: string; specifier: string }[];
      };
  let resourceReferences: Extract<LoadStep, { kind: "complete" }>["resources"] = [];
  const initial = JSON.parse(
    loader.provide(0, root, rootContent ?? (await host.read(root))),
  ) as LoadStep;
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
        if (started.has(request.id)) continue;
        started.add(request.id);
        const read = async () => {
          const name = await host.resolve(request.specifier, request.importer);
          if (stopped) return;
          host.watch(name);
          const content = await host.read(name);
          if (stopped) return;
          // JS callbacks serialize provide/step processing; I/O remains concurrent.
          advance(JSON.parse(loader.provide(request.id, name, content)) as LoadStep);
        };
        // Handle every rejection, including reads finishing after another read failed.
        void read().catch(fail);
      }
    };
    advance(initial);
  });
  return { loader, resources: resourceReferences };
}
