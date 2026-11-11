import { expect, test } from "vite-plus/test";

import { LoaderCache, recordInputs } from "./loader-cache.ts";

/**
 * Creates an in-memory file system for input checks.
 *
 * @param files - Initial file contents keyed by path.
 * @returns The mutable contents and a reader that resolves undefined for a missing file.
 */
function memoryFiles(files: Record<string, string>) {
  const contents = new Map(Object.entries(files));
  return {
    contents,
    read: async (id: string) => contents.get(id),
  };
}

test("a value is shared while its inputs stay unchanged and closed after a replacement ends its last use", async () => {
  const { contents, read } = memoryFiles({ "/theme.css": "a" });
  const closed: number[] = [];
  const cache = new LoaderCache<number>((value) => closed.push(value));
  let created = 0;
  const create = async () => {
    const recorder = recordInputs(read);
    await recorder.read("/theme.css");
    recorder.watchMissing("/preset.css");
    return { value: ++created, inputs: await recorder.finish() };
  };

  const [first, concurrent] = await Promise.all([
    cache.use("key", read, create),
    cache.use("key", read, create),
  ]);
  expect([first.value, concurrent.value]).toEqual([1, 1]);

  contents.set("/theme.css", "b");
  const second = await cache.use("key", read, create);
  expect(second.value).toBe(2);
  first.done();
  expect(closed).toEqual([]);
  concurrent.done();
  expect(closed).toEqual([1]);

  contents.set("/preset.css", "");
  const third = await cache.use("key", read, create);
  expect(third.value).toBe(3);
  second.done();
  expect(closed).toEqual([1, 2]);
});

test("a failed creation and a value with directory inputs are created again", async () => {
  const { read } = memoryFiles({});
  const cache = new LoaderCache<string>();
  await expect(
    cache.use("key", read, async () => {
      throw new Error("invalid CSS");
    }),
  ).rejects.toThrow("invalid CSS");

  let created = 0;
  const create = async () => {
    const recorder = recordInputs(read);
    recorder.watchDirectory("/plugins");
    return { value: `created ${++created}`, inputs: await recorder.finish() };
  };
  (await cache.use("key", read, create)).done();
  const second = await cache.use("key", read, create);
  expect(second.value).toBe("created 2");
  expect(second.inputs.directories).toEqual(new Set(["/plugins"]));
});
