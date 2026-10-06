import { expect, test } from "vite-plus/test";

import { createMemoryAnalysisWorkspace } from "./memory.ts";

test("memory services preserve URI identities, capture dependency buffers, and drop stale or cancelled results", async () => {
  const uri = "inmemory://play/src/entry.tsx";
  const dependency = "inmemory://play/src/tokens.ts";
  const entered = Promise.withResolvers<void>();
  const release = Promise.withResolvers<void>();
  let first = true;
  let closes = 0;
  const snapshots: ReadonlyMap<string, string>[] = [];
  const workspace = createMemoryAnalysisWorkspace({
    config: "inmemory://play/stillwind.config.ts",
    includes: (id) => id === uri,
    compile: async (input) => {
      snapshots.push(input.documents);
      if (first) {
        first = false;
        entered.resolve();
        await release.promise;
      }
      return { status: "complete", reports: [] };
    },
    close: () => {
      closes++;
    },
  });
  try {
    await workspace.update(uri, "entry", 1);
    await workspace.update(dependency, "red", 1);
    const stale = workspace.analyze(uri);
    await entered.promise;
    await workspace.update(dependency, "blue", 2);
    const current = workspace.analyze(uri);
    release.resolve();
    expect((await stale)[0].result).toMatchObject({ entry: uri, status: "stale", reports: [] });
    expect((await current)[0]).toMatchObject({
      config: "inmemory://play/stillwind.config.ts",
      result: { entry: uri, status: "complete" },
    });
    expect(snapshots.map((snapshot) => snapshot.get(dependency))).toEqual(["red", "blue"]);
    expect((await workspace.analyze(uri, AbortSignal.abort()))[0].result.status).toBe("cancelled");
    expect(snapshots).toHaveLength(2);
    expect(await workspace.analyze(dependency)).toEqual([]);
  } finally {
    release.resolve();
    await Promise.all([workspace.close(), workspace.close()]);
  }
  expect(closes).toBe(1);
});
