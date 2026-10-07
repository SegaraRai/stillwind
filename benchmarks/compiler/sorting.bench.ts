import { readFileSync } from "node:fs";

import { NativeDesignSystemLoader, __napiBindingTarget } from "@stillwind/native";
import { expect, inject, test, type BenchRunOptions } from "vite-plus/test";

import { CandidateQueries } from "../../packages/host/src/editor/candidates.ts";
import { EditorSnapshot, type EditorEdit } from "../../packages/host/src/editor/snapshot.ts";
import { applyEdits, sortSource } from "../../packages/host/src/editor/sort.ts";
import { SourceIndex } from "../../packages/host/src/editor/source-index.ts";
import { baseStyleCss, loadSortingDesignSystem, prepareSortingSource } from "./sorting-cases.ts";

const repeated: BenchRunOptions = { iterations: 9, time: 0, warmupIterations: 2, warmupTime: 0 };
const stress: BenchRunOptions = { ...repeated, iterations: 3, warmupIterations: 1 };

test("sorting/runtime", () => {
  expect(__napiBindingTarget).toBe(inject("benchmarkWasm") ? "wasm32-wasi" : "native");
});

for (const mode of ["authored", "tailwind"] as const) {
  const loader = loadSortingDesignSystem(mode);
  const query = loader.editorQuery();

  for (const [name, left, right, expected] of [
    ["plain", "p-4", "p-8", -1],
    ["theme-color", "bg-brand", "bg-red-500", -1],
    ["modifier", "bg-brand/50", "bg-red-500/50", -1],
    ["variant-stack", "md:hover:p-4", "md:focus:p-8", null],
    ["self", "hover:p-4", "hover:p-4", 0],
    ["unresolved", "external-class", "p-4", null],
  ] as const) {
    test(`sorting/${mode}/compare-256/${name}`, async ({ bench, task }) => {
      const baseline = query.compareCandidates(left, right);
      if (name !== "variant-stack") {
        expect(baseline).toBe(expected);
      } else {
        expect(baseline).not.toBeNull();
      }
      let answer: number | null;
      task.meta.measurement = repeated;
      task.meta.notes =
        "256 comparisons; divide batch latency by 256 for one comparison. The production comparator has no candidate-result cache.";
      await bench("compare-batch", { async: false }, () => {
        for (let i = 0; i < 256; i++) {
          answer = query.compareCandidates(left, right);
        }
      }).run(repeated);
      expect(answer!).toBe(baseline);
    });
  }

  for (const count of [10, 30, 100]) {
    const tokens = Array.from(
      { length: count },
      (_, i) => `${["p", "m", "w", "h", "gap"][i % 5]}-${i + 1}`,
    ).sort((a, b) => query.compareCandidates(a, b)!);
    for (const shape of ["string", "array"] as const) {
      for (const order of ["sorted", "reverse", "shuffled"] as const) {
        const input =
          order === "sorted"
            ? tokens
            : order === "reverse"
              ? [...tokens].reverse()
              : shuffle(tokens);
        const body =
          shape === "string"
            ? JSON.stringify(input.join(" "))
            : `[${input.map((token) => JSON.stringify(token)).join(",")}]`;
        registerSource(
          `${mode}/${shape}/${order}/${count}`,
          loader,
          `const value=style(${body});`,
          shape,
          order !== "sorted",
        );
      }
    }
  }

  const stacked = Array.from(
    { length: 30 },
    (_, i) => `${["md:hover:", "focus:", "sm:"][i % 3]}p-${i + 1}`,
  ).sort((a, b) => query.compareCandidates(a, b)!);
  for (const shape of ["string", "array"] as const) {
    const input = shuffle(stacked);
    const body =
      shape === "string"
        ? JSON.stringify(input.join(" "))
        : `[${input.map((token) => JSON.stringify(token)).join(",")}]`;
    registerSource(
      `${mode}/${shape}/variants/30`,
      loader,
      `const value=style(${body});`,
      shape,
      true,
    );
  }

  const long = Array.from({ length: 1000 }, (_, i) => `p-${i + 1}`).sort((a, b) =>
    query.compareCandidates(a, b)!,
  );
  registerSource(
    `${mode}/string/sorted/1000`,
    loader,
    `const value=style(${JSON.stringify(long.join(" "))});`,
    "string",
    false,
    stress,
  );

  if (mode === "authored") {
    const properties = independentProperties(query, 100);
    for (const count of [10, 30, 100]) {
      for (const kind of ["refs", "bases-disjoint", "bases-conflicting"] as const) {
        const names = Array.from({ length: count }, (_, i) => `item${String(i).padStart(4, "0")}`);
        const declarations = names
          .map(
            (name, i) =>
              `const ${name}=${kind === "refs" ? "style.ref()" : `style(${JSON.stringify(kind === "bases-disjoint" ? `[${properties[i]}:initial]` : `p-${i + 1}`)})`};`,
          )
          .join("\n");
        const source = `${declarations}\nconst value=style([${[...names].reverse().join(",")}]);`;
        registerSource(
          `${mode}/array/${kind}/${count}`,
          loader,
          source,
          "array",
          kind !== "bases-conflicting",
        );
      }
    }
  }
}

/**
 * Selects `count` published properties whose declarations pass `query`'s safety check
 * against every previously selected property. Throws when too few properties qualify.
 */
function independentProperties(
  query: ReturnType<NativeDesignSystemLoader["editorQuery"]>,
  count: number,
): string[] {
  const definitions = readFileSync(
    new URL("../../crates/css/assets/webref-properties.rs", import.meta.url),
    "utf8",
  );
  const selected: string[] = [];
  for (const match of definitions.matchAll(/name: "([a-z][a-z-]*)"/gu)) {
    const property = match[1];
    if (
      selected.every((other) =>
        query.canReorderStyles(`&{${property}:initial;}`, `&{${other}:initial;}`),
      )
    ) {
      selected.push(property);
      if (selected.length === count) {
        return selected;
      }
    }
  }
  throw new Error(`Only ${selected.length} independent CSS properties were found`);
}

/** Measures actual editor sorting after CSS loading, compilation, and syntax indexing. */
function registerSource(
  name: string,
  loader: NativeDesignSystemLoader,
  body: string,
  shape: "string" | "array",
  changes: boolean,
  options = repeated,
): void {
  for (const reuse of ["fresh", "repeated"] as const) {
    test(`sorting/source/${name}/${reuse}`, async ({ bench, task }) => {
      const { entry, source, session, graph } = await prepareSortingSource(loader, body);
      expect(
        graph.batches
          .flatMap((batch) => batch.reports)
          .filter((report) => report.severity === "error"),
      ).toEqual([]);
      const config = { sort: { stringLiterals: shape === "string", arrays: shape === "array" } };
      const create = () => new EditorSnapshot(graph, session, loader, config);
      let snapshot = create();
      const expected = snapshot.query(entry, { kind: "sort" })!.edits!;
      expect(expected.length > 0).toBe(changes);
      const expectedSource = applyEdits(source, expected);

      const candidates = new CandidateQueries(loader.editorQuery());
      const compare = candidates.compare.bind(candidates);
      const canReorder = candidates.canReorderStyles.bind(candidates);
      let comparisons = 0,
        styleComparisons = 0;
      candidates.compare = (a, b) => {
        comparisons++;
        return compare(a, b);
      };
      candidates.canReorderStyles = (a, b) => {
        styleComparisons++;
        return canReorder(a, b);
      };
      const index = new SourceIndex(graph, session);
      const instrumented = sortSource(
        graph.sources[0],
        index,
        candidates,
        config.sort,
        [],
        undefined,
        (asset) => baseStyleCss(index, candidates, asset),
      );
      expect(applyEdits(source, instrumented)).toBe(expectedSource);
      task.meta.measurement = options;
      task.meta.notes = JSON.stringify({
        comparisons,
        styleComparisons,
        sourceUtf16: source.length,
        edits: expected.length,
        generation: graph.generation,
        unverified: graph.unverified.length,
        boundary:
          "EditorSnapshot.query(sort); excludes CSS loading, compilation, SourceIndex construction, transport, and applying edits",
        cache:
          reuse === "fresh"
            ? "new snapshot per sample; preview cache empty"
            : "same snapshot after first sort; previews reused; comparisons still resolve candidates",
      });
      let edits: EditorEdit[];
      await bench(
        "editor-sort",
        {
          async: false,
          beforeEach: () => {
            if (reuse === "fresh") {
              snapshot = create();
            }
          },
        },
        () => {
          edits = snapshot.query(entry, { kind: "sort" })!.edits!;
        },
      ).run(options);
      expect(edits!).toEqual(expected);
    });
  }
}

/** Uses a fixed seed so random input order remains reproducible across runtimes. */
function shuffle(values: readonly string[]): string[] {
  const result = [...values];
  let seed = 0x51a7;
  for (let i = result.length - 1; i > 0; i--) {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    const target = seed % (i + 1);
    [result[i], result[target]] = [result[target], result[i]];
  }
  return result;
}
