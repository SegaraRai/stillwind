import { TraceMap, originalPositionFor } from "@jridgewell/trace-mapping";
import { expect, test, vi } from "vite-plus/test";

import sourceMapLoader, {
  type LoaderAdditionalData,
  type LoaderCompletion,
  type SourceTransformContext,
  type SourceTransformData,
} from "./source-map-loader.ts";

for (const runner of ["webpack", "rspack"] as const) {
  test(`${runner} composes pitch maps and preserves additional loader data`, async () => {
    const resource = "/Component.vue";
    const inputMap = {
      version: 3 as const,
      sources: [resource],
      sourcesContent: ["transformed"],
      names: [],
      mappings: "AAAA",
    };
    const data: SourceTransformData = {
      stillwindSourceTransform: {
        code: "transformed",
        resource,
        map: {
          ...inputMap,
          sources: ["original.vue"],
          sourcesContent: ["original"],
        },
      },
    };
    const additionalData: LoaderAdditionalData = {
      stillwind: { sourceTransform: { resource, fingerprint: "digest" } },
      anotherLoader: { retained: true },
    };
    const callback = vi.fn<LoaderCompletion>();
    const context = {
      async: () => callback,
      resourcePath: resource,
      mode: "production",
      loaders: [
        runner === "webpack"
          ? {
              data,
            }
          : {
              loaderItem: { data },
            },
      ],
    } as unknown as SourceTransformContext<unknown>;
    sourceMapLoader.call(context, "framework output", inputMap, additionalData);
    await vi.waitFor(() => expect(callback).toHaveBeenCalledOnce());
    const [error, source, map, forwarded] = callback.mock.calls[0]!;
    expect(error).toBeNull();
    expect(source).toBe("framework output");
    expect(forwarded).toBe(additionalData);
    const trace = new TraceMap(JSON.stringify(map));
    expect(originalPositionFor(trace, { line: 1, column: 0 }).source).toBe("/original.vue");
  });
}
