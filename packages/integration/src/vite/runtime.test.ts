import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { createDevelopmentRuntime } from "./runtime.ts";

beforeEach(() => {
  Reflect.deleteProperty(globalThis, Symbol.for("stillwind.document"));
});

afterEach(() => {
  vi.unstubAllGlobals();
  Reflect.deleteProperty(globalThis, Symbol.for("stillwind.document"));
});

function runtime(fetch: ReturnType<typeof vi.fn>) {
  const style = { dataset: {}, isConnected: false, textContent: "" };
  vi.stubGlobal("fetch", fetch);
  vi.stubGlobal("document", {
    createElement: () => style,
    head: {
      append: () => {
        style.isConnected = true;
      },
    },
  });
  const api = createDevelopmentRuntime({
    snapshot: { revision: 3, parts: [], dependencies: {} },
    endpoint: "/__stillwind/css",
  });
  return { ...api, style };
}

test("a cached runtime fetches current CSS even when its embedded revision satisfies the producer", async () => {
  const fetch = vi.fn().mockResolvedValue({
    ok: true,
    json: async () => ({
      revision: 4,
      parts: [{ producers: ["button.tsx"], css: ".button{padding:32px}" }],
      dependencies: {},
    }),
  });
  const { register, synchronize, style } = runtime(fetch);
  await synchronize(3);
  register("button.tsx");
  expect(fetch).toHaveBeenCalledExactlyOnceWith("/__stillwind/css");
  expect(style.textContent).toBe(".button{padding:32px}");
});

test("concurrent producers share a fetch and retry until the required revision arrives", async () => {
  let release!: (response: unknown) => void;
  const fetch = vi
    .fn()
    .mockReturnValueOnce(
      new Promise((resolve) => {
        release = resolve;
      }),
    )
    .mockResolvedValueOnce({
      ok: true,
      json: async () => ({ revision: 5, parts: [], dependencies: {} }),
    });
  const { synchronize } = runtime(fetch);
  const first = synchronize(4);
  const second = synchronize(5);
  expect(fetch).toHaveBeenCalledTimes(1);
  release({ ok: true, json: async () => ({ revision: 4, parts: [], dependencies: {} }) });
  await Promise.all([first, second]);
  expect(fetch).toHaveBeenCalledTimes(2);
});

test("runtime instances share active producers and reject older snapshots", () => {
  const { register, apply, style } = runtime(vi.fn());
  apply({
    revision: 4,
    parts: [
      { producers: [], css: ".base{}" },
      { producers: ["button.tsx"], css: ".button{}" },
      { producers: ["shared.tsx"], css: ".shared{}" },
      { producers: ["unused.tsx"], css: ".unused{}" },
    ],
    dependencies: { "button.tsx": ["shared.tsx"] },
  });
  register("button.tsx");
  expect(style.textContent).toBe(".base{}.button{}.shared{}");
  const second = createDevelopmentRuntime({
    snapshot: { revision: 3, parts: [], dependencies: {} },
    endpoint: "/__stillwind/css",
  });
  expect(style.textContent).toBe(".base{}.button{}.shared{}");
  second.unregister("button.tsx");
  expect(style.textContent).toBe(".base{}");
});

test("synchronization without a document does not fetch", async () => {
  const fetch = vi.fn();
  vi.stubGlobal("fetch", fetch);
  vi.stubGlobal("document", undefined);
  const api = createDevelopmentRuntime({
    snapshot: { revision: 3, parts: [], dependencies: {} },
    endpoint: "/__stillwind/css",
  });
  api.register("button.tsx");
  await api.synchronize(4);
  expect(fetch).not.toHaveBeenCalled();
});

test("a failed fetch releases the pending request for a later synchronization", async () => {
  const fetch = vi
    .fn()
    .mockResolvedValueOnce({ ok: false })
    .mockResolvedValueOnce({
      ok: true,
      json: async () => ({ revision: 4, parts: [], dependencies: {} }),
    });
  const { synchronize } = runtime(fetch);
  await expect(synchronize(4)).rejects.toThrow("Stillwind stylesheet synchronization failed");
  await synchronize(4);
  expect(fetch).toHaveBeenCalledTimes(2);
});
