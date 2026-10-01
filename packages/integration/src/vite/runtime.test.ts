import { runInNewContext } from "node:vm";

import { expect, test, vi } from "vite-plus/test";

import { developmentRuntime } from "./runtime.ts";

function runtime(fetch: ReturnType<typeof vi.fn>) {
  const style = { dataset: {}, isConnected: false, textContent: "" };
  const context = {
    fetch,
    document: {
      createElement: () => style,
      head: {
        append: () => {
          style.isConnected = true;
        },
      },
    },
  };
  const code = developmentRuntime(
    { revision: 3, parts: [], dependencies: {} },
    false,
    "/__stillwind/css",
  )
    .replaceAll("export ", "")
    .replaceAll("import.meta.hot", "false");
  const api = runInNewContext(`${code}\n({register,synchronize});`, context) as {
    register(this: void, id: string): void;
    synchronize(this: void, revision: number): Promise<void>;
  };
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
