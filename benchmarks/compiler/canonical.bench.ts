import { NativeProjectSession, __napiBindingTarget } from "@stillwind/native";
import { expect, inject, test, type BenchRunOptions } from "vite-plus/test";

import {
  canonicalCases,
  canonicalLoader,
  canonicalPalette,
  canonicalSource,
} from "./canonical-cases.ts";

const options: BenchRunOptions = {
  iterations: 9,
  time: 0,
  warmupIterations: 1,
  warmupTime: 0,
};

test("canonical/runtime", () => {
  expect(__napiBindingTarget).toBe(inject("benchmarkWasm") ? "wasm32-wasi" : "native");
});

for (const scenario of canonicalCases) {
  test(`canonical/cold-query/${scenario.name}`, async ({ bench, task }) => {
    const loader = canonicalLoader();
    let query = loader.editorQuery();
    let answer: string | null;
    task.meta.measurement = options;
    const result = await bench(
      "canonical",
      {
        async: false,
        beforeEach: () => {
          query = loader.editorQuery();
        },
      },
      () => {
        answer = query.canonicalCandidate(scenario.candidate);
      },
    ).run(options);
    expect(answer!).toBe(scenario.expected);
    expect(result.latency.samplesCount).toBeGreaterThanOrEqual(9);
  });
}

test("canonical/prepare-css-revision", async ({ bench, task }) => {
  let query = canonicalLoader().editorQuery();
  task.meta.measurement = options;
  await bench("load-css-and-index", { async: false }, () => {
    query = canonicalLoader().editorQuery();
  }).run(options);
  expect(query.canonicalCandidate("[border-color:#123456]")).toBeNull();
});

test("canonical/reuse-prepared-revision", async ({ bench, task }) => {
  const loader = canonicalLoader();
  let query = loader.editorQuery();
  task.meta.measurement = options;
  await bench("create-query", { async: false }, () => {
    query = loader.editorQuery();
  }).run(options);
  expect(query.canonicalCandidate("bg-[#f00]")).toBe("bg-brand");
});

test("canonical/cached-2000", async ({ bench, task }) => {
  const query = canonicalLoader().editorQuery();
  expect(query.canonicalCandidate("bg-[#123456]")).toBeNull();
  let answer: string | null;
  task.meta.measurement = options;
  await bench("cached-batch", { async: false }, () => {
    for (let i = 0; i < 2000; i++) {
      answer = query.canonicalCandidate("bg-[#123456]");
    }
  }).run(options);
  expect(answer!).toBeNull();
});

for (const count of [10, 100, 1000]) {
  test(`canonical/palette/${count}`, async ({ bench, task }) => {
    const loader = canonicalPalette(count);
    let query = loader.editorQuery();
    let answer: string | null;
    task.meta.measurement = options;
    await bench(
      "color-miss",
      {
        async: false,
        beforeEach: () => {
          query = loader.editorQuery();
        },
      },
      () => {
        answer = query.canonicalCandidate("bg-[rgb(171_205_239)]");
      },
    ).run(options);
    expect(answer!).toBeNull();
  });
}

for (const unique of [false, true]) {
  test(`canonical/module/${unique ? "unique" : "repeated"}/100`, async ({ bench, task }) => {
    const loader = canonicalLoader();
    const source = canonicalSource(100, unique);
    let session = new NativeProjectSession(loader, false);
    let reports: ReturnType<NativeProjectSession["diagnose"]>;
    task.meta.measurement = options;
    await bench(
      "diagnose",
      {
        async: false,
        beforeEach: () => {
          session = new NativeProjectSession(loader, false);
        },
      },
      () => {
        reports = session.diagnose([source]);
      },
    ).run(options);
    expect(reports!).toHaveLength(1);
    expect(reports![0].reports.map((report) => report.code)).toEqual(
      Array.from({ length: (100 * 99) / 2 }, () =>
        unique ? "conflicting-utilities" : "duplicate-utilities",
      ),
    );
  });
}
