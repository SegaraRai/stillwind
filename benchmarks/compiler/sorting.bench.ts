import { readFileSync } from "node:fs";

import { NativeDesignSystemLoader, __napiBindingTarget } from "@stillwind/native";
import { expect, inject, test, type BenchRunOptions } from "vite-plus/test";

import { CandidateQueries } from "../../packages/host/src/editor/candidates.ts";
import { EditorSnapshot, type EditorEdit } from "../../packages/host/src/editor/snapshot.ts";
import { applyEdits, sortSource } from "../../packages/host/src/editor/sort.ts";
import { SourceIndex } from "../../packages/host/src/editor/source-index.ts";
import { assertBudget, budgets } from "../performance/budgets.ts";
import { loadSortingDesignSystem, prepareSortingSource } from "./sorting-cases.ts";

const repeated: BenchRunOptions = {
  iterations: 9,
  time: 0,
  warmupIterations: 2,
  warmupTime: 0,
};
const stress: BenchRunOptions = {
  ...repeated,
  iterations: 3,
  warmupIterations: 1,
};

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
    test(`sorting/${mode}/compare-256/${name}`, async (context) => {
      const { bench, task } = context;
      const baseline = query.compareCandidates(left, right);
      if (name !== "variant-stack") {
        expect(baseline).toBe(expected);
      } else {
        expect(baseline).not.toBeNull();
      }
      let answer: number | null;
      task.meta.measurement = repeated;
      task.meta.notes =
        "256 comparisons; divide batch latency by 256 for one comparison. Prepared candidate keys are retained by this immutable query.";
      const results = await bench("compare-batch", { async: false }, () => {
        for (let i = 0; i < 256; i++) {
          answer = query.compareCandidates(left, right);
        }
      }).run(repeated);
      expect(answer!).toBe(baseline);
      await assertBudget(
        context,
        "256 cached candidate comparisons",
        results,
        budgets.sorting.cachedComparisonBatch,
      );
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
          shape === "string" && order !== "sorted",
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
      shape === "string",
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

  for (const count of [10, 100, 1000]) {
    const declarations = 'const base=style({"hover:":"p-4"}); const ref=style.ref();';
    const groups = Array.from(
      { length: Math.ceil(count / 6) },
      (_, i) => `{"data-value":${i}},{"--value":${i}},"p-${i + 1}",base,ref.asContainer(),ref`,
    );
    registerSource(
      `${mode}/array/mixed/${count}`,
      loader,
      `${declarations} const value=style([${groups.join(",")}]);`,
      "array",
      true,
      count === 1000 ? stress : repeated,
    );
  }
  for (const count of [10, 100, 1000]) {
    const values = Array.from({ length: count }, (_, i) => `md:hover:p-${i + 1}`);
    for (const reuse of ["fresh", "repeated"] as const) {
      test(`sorting/${mode}/ranks/${count}/${reuse}`, async (context) => {
        const { bench, task } = context;
        let current = loader.editorQuery();
        const expected = current.candidateRanks(values);
        let result: ReturnType<typeof current.candidateRanks>;
        task.meta.measurement = repeated;
        task.meta.notes =
          "Batch resolves and sorts numeric candidate ranks; query creation is outside the timer.";
        const results = await bench(
          "candidate-ranks",
          {
            async: false,
            beforeEach: () => {
              if (reuse === "fresh") {
                current = loader.editorQuery();
              }
            },
          },
          () => {
            result = current.candidateRanks(values);
          },
        ).run(repeated);
        expect(result!).toEqual(expected);
        await assertBudget(
          context,
          "candidate ranks",
          results,
          count <= 100 ? budgets.sorting.ranks100 : budgets.sorting.ranks1000,
        );
      });
    }
  }

  for (const count of [100, 1000]) {
    const names = Array.from(
      { length: count },
      (_, i) => `data-${String(i).padStart(4, "0")}`,
    ).reverse();
    registerSource(
      `${mode}/object/attributes/${count}`,
      loader,
      `const value=style({${names.map((name) => `${JSON.stringify(name)}:1`).join(",")}});`,
      "array",
      true,
      count === 1000 ? stress : repeated,
      true,
    );
  }

  if (mode === "authored") {
    const properties = publishedProperties(100);
    for (const count of [10, 30, 100]) {
      for (const kind of ["refs", "bases-distinct-properties", "bases-shared-property"] as const) {
        const names = Array.from({ length: count }, (_, i) => `item${String(i).padStart(4, "0")}`);
        const declarations = names
          .map(
            (name, i) =>
              `const ${name}=${kind === "refs" ? "style.ref()" : `style(${JSON.stringify(kind === "bases-distinct-properties" ? `[${properties[i]}:initial]` : `p-${i + 1}`)})`};`,
          )
          .join("\n");
        const source = `${declarations}\nconst value=style([${[...names].reverse().join(",")}]);`;
        registerSource(`${mode}/array/${kind}/${count}`, loader, source, "array", false);
      }
    }
  }
}

/** Returns `count` distinct published properties for inputs with different base bodies. */
function publishedProperties(count: number): string[] {
  const definitions = readFileSync(
    new URL("../../crates/css/assets/webref-properties.rs", import.meta.url),
    "utf8",
  );
  const properties = [
    ...new Set([...definitions.matchAll(/name: "([a-z][a-z-]*)"/gu)].map((match) => match[1])),
  ];
  expect(properties.length).toBeGreaterThanOrEqual(count);
  return properties.slice(0, count);
}

/** Measures actual editor sorting after CSS loading, compilation, and syntax indexing. */
function registerSource(
  name: string,
  loader: NativeDesignSystemLoader,
  body: string,
  shape: "string" | "array",
  changes: boolean,
  options = repeated,
  attributes = false,
): void {
  const count = Number(name.split("/").at(-1));
  let prepared: ReturnType<typeof prepareSortingSource> | undefined;
  for (const reuse of ["fresh", "repeated", "construction"] as const) {
    test(`sorting/source/${name}/${reuse}`, async (context) => {
      const { bench, task } = context;
      prepared ??= prepareSortingSource(loader, body);
      const { entry, source, session, graph } = await prepared;
      expect(
        graph.batches
          .flatMap((batch) => batch.reports)
          .filter((report) => report.severity === "error"),
      ).toEqual([]);
      const config = {
        sort: { stringLiterals: shape === "string", arrays: shape === "array", attributes },
      };
      const create = () => new EditorSnapshot(graph, session, loader, config);
      let snapshot = create();
      const expected = snapshot.query(entry, { kind: "sort" })!.edits!;
      expect(expected.length > 0).toBe(changes);
      const expectedSource = applyEdits(source, expected);

      const candidates = new CandidateQueries(loader.editorQuery());
      const order = candidates.order.bind(candidates);
      let comparisons = 0;
      let rankBatches = 0;
      let candidateCount = 0;
      candidates.order = (values) => {
        rankBatches++;
        candidateCount += new Set(values).size;
        const compare = order(values);
        return (a, b) => {
          comparisons++;
          return compare(a, b);
        };
      };
      const index = new SourceIndex(graph, session);
      const instrumented = sortSource(graph.sources[0], index, candidates, config.sort, []);
      expect(applyEdits(source, instrumented)).toBe(expectedSource);
      task.meta.measurement = options;
      task.meta.notes = JSON.stringify({
        comparisons,
        rankBatches,
        candidateCount,
        sourceUtf16: source.length,
        edits: expected.length,
        generation: graph.generation,
        unverified: graph.unverified.length,
        boundary:
          reuse === "construction"
            ? "new EditorSnapshot including SourceIndex; excludes compilation, query, transport, and applying edits"
            : "EditorSnapshot.query(sort); excludes CSS loading, compilation, SourceIndex construction, transport, and applying edits",
        cache:
          reuse === "fresh"
            ? "new snapshot per sample; ordering keys empty"
            : reuse === "repeated"
              ? "same snapshot after first sort; prepared candidate keys reused"
              : "new snapshot and source index; keys empty",
      });
      let edits: EditorEdit[];
      const results = await bench(
        reuse === "construction" ? "editor-construction" : "editor-sort",
        {
          async: false,
          beforeEach: () => {
            if (reuse === "fresh") {
              snapshot = create();
            }
          },
        },
        () => {
          if (reuse === "construction") {
            snapshot = create();
          } else {
            edits = snapshot.query(entry, { kind: "sort" })!.edits!;
          }
        },
      ).run(options);
      expect(
        reuse === "construction" ? snapshot.query(entry, { kind: "sort" })!.edits! : edits!,
      ).toEqual(expected);
      await assertBudget(
        context,
        reuse === "construction" ? "editor construction" : "editor sort",
        results,
        reuse === "construction"
          ? count <= 100
            ? budgets.sorting.construction100
            : budgets.sorting.construction1000
          : count <= 100
            ? budgets.sorting.query100
            : budgets.sorting.query1000,
      );
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
