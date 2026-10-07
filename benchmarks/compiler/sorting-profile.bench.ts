import { __napiBindingTarget } from "@stillwind/native";
import { expect, test, type BenchRunOptions } from "vite-plus/test";

import { CandidateQueries } from "../../packages/host/src/editor/candidates.ts";
import { sortSource } from "../../packages/host/src/editor/sort.ts";
import { SourceIndex } from "../../packages/host/src/editor/source-index.ts";
import { baseStyleCss, loadSortingDesignSystem, prepareSortingSource } from "./sorting-cases.ts";

const options: BenchRunOptions = { iterations: 3, time: 0, warmupIterations: 1, warmupTime: 0 };

for (const count of [10, 30, 100]) {
  for (const kind of ["refs", "bases-conflicting"] as const) {
    test(`sorting/profile/${kind}/${count}`, async ({ bench, task }) => {
      expect(__napiBindingTarget).toBe("native");
      const names = Array.from({ length: count }, (_, i) => `item${String(i).padStart(4, "0")}`);
      const body = names
        .map(
          (name, i) => `const ${name}=${kind === "refs" ? "style.ref()" : `style("p-${i + 1}")`};`,
        )
        .join("\n");
      const loader = loadSortingDesignSystem("authored");
      const { session, graph } = await prepareSortingSource(
        loader,
        `${body}\nconst value=style([${[...names].reverse().join(",")}]);`,
      );
      const index = new SourceIndex(graph, session);
      const candidates = new CandidateQueries(loader.editorQuery());
      const run = () =>
        sortSource(
          graph.sources[0],
          index,
          candidates,
          { stringLiterals: false, arrays: true },
          [],
          undefined,
          (asset) => baseStyleCss(index, candidates, asset),
        );
      const expected = run();
      expect(expected.length > 0).toBe(kind === "refs");

      const asset = index.asset.bind(index);
      const baseCandidates = index.baseCandidates.bind(index);
      const canReorderStyles = candidates.canReorderStyles.bind(candidates);
      let assetCalls = 0,
        baseCalls = 0,
        baseMs = 0,
        safetyCalls = 0,
        safetyMs = 0;
      index.asset = (...args) => {
        assetCalls++;
        return asset(...args);
      };
      index.baseCandidates = (...args) => {
        baseCalls++;
        const start = performance.now();
        const result = baseCandidates(...args);
        baseMs += performance.now() - start;
        return result;
      };
      candidates.canReorderStyles = (...args) => {
        safetyCalls++;
        const start = performance.now();
        const result = canReorderStyles(...args);
        safetyMs += performance.now() - start;
        return result;
      };
      const start = performance.now();
      const profiled = run();
      const totalMs = performance.now() - start;
      expect(profiled).toEqual(expected);
      index.asset = asset;
      index.baseCandidates = baseCandidates;
      candidates.canReorderStyles = canReorderStyles;

      task.meta.measurement = options;
      task.meta.notes = JSON.stringify({
        count,
        kind,
        assetCalls,
        baseCalls,
        baseMs,
        safetyCalls,
        safetyMs,
        totalMs,
        generation: graph.generation,
        unverified: graph.unverified.length,
        boundary:
          "sortSource after an initial request; excludes compilation and SourceIndex construction",
        profile:
          "One separate invocation counts asset lookups and times baseCandidates and canReorderStyles. baseMs includes nested asset lookups. The benchmark restores all original methods before timing.",
      });
      let result: ReturnType<typeof run>;
      await bench("sort-source", { async: false }, () => {
        result = run();
      }).run(options);
      expect(result!).toEqual(expected);
    });
  }
}
