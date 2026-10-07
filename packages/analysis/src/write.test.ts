import { readFile, writeFile } from "node:fs/promises";

import { expect, test, vi } from "vite-plus/test";

import { writeChangedFiles } from "./write.ts";

vi.mock("node:fs/promises", () => ({
  readFile: vi.fn(),
  writeFile: vi.fn(),
}));

test("unchanged bytes never open, replace, or write a file", async () => {
  vi.clearAllMocks();
  await writeChangedFiles([
    {
      file: "unchanged.ts",
      before: Buffer.from("same"),
      after: Buffer.from("same"),
    },
  ]);
  expect(readFile).not.toHaveBeenCalled();
  expect(writeFile).not.toHaveBeenCalled();
});

test("changed bytes are saved once and a stale batch saves nothing", async () => {
  vi.clearAllMocks();
  vi.mocked(readFile).mockResolvedValue(Buffer.from("before"));
  await writeChangedFiles([
    {
      file: "changed.ts",
      before: Buffer.from("before"),
      after: Buffer.from("after"),
    },
  ]);
  expect(writeFile).toHaveBeenCalledExactlyOnceWith("changed.ts", Buffer.from("after"));
  vi.clearAllMocks();
  vi.mocked(readFile).mockResolvedValue(Buffer.from("external"));
  await expect(
    writeChangedFiles([
      {
        file: "changed.ts",
        before: Buffer.from("before"),
        after: Buffer.from("after"),
      },
    ]),
  ).rejects.toThrow("Source changed");
  expect(writeFile).not.toHaveBeenCalled();
});
