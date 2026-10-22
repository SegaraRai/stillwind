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

test("changed bytes are saved once and stale files are retained", async () => {
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
  expect(
    await writeChangedFiles([
      {
        file: "changed.ts",
        before: Buffer.from("before"),
        after: Buffer.from("after"),
      },
    ]),
  ).toMatchObject({
    changed: [],
    failures: [
      {
        file: "changed.ts",
        message: expect.stringContaining("Source changed"),
      },
    ],
  });
  expect(writeFile).not.toHaveBeenCalled();
});

test("saving continues after stale bytes and write failures and reports only successful writes", async () => {
  vi.clearAllMocks();
  vi.mocked(readFile).mockImplementation(async (file) =>
    Buffer.from(file === "stale.ts" ? "external" : "before"),
  );
  vi.mocked(writeFile).mockImplementation(async (file) => {
    if (file === "denied.ts") {
      throw new Error("permission denied");
    }
  });
  const result = await writeChangedFiles(
    ["first.ts", "stale.ts", "denied.ts", "last.ts"].map((file) => ({
      file,
      before: Buffer.from("before"),
      after: Buffer.from("after"),
    })),
  );
  expect(result).toMatchObject({
    changed: ["first.ts", "last.ts"],
    failures: [
      {
        file: "stale.ts",
        message: expect.stringContaining("Source changed"),
      },
      {
        file: "denied.ts",
        message: "permission denied",
      },
    ],
  });
  expect(vi.mocked(writeFile).mock.calls.map(([file]) => file)).toEqual([
    "first.ts",
    "denied.ts",
    "last.ts",
  ]);
});
