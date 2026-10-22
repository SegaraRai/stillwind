import { fingerprint } from "@stillwind/native";
import { expect, test, vi } from "vite-plus/test";

import { applySourceEdits, SourceRewriter } from "./rewrite.ts";

test("fixes restore failed buffers before converging dependent targets", async () => {
  const buffers = new Map<string, string>();
  const views: Map<string, string>[] = [];
  const workspace: ConstructorParameters<typeof SourceRewriter>[0] = {
    invalidate: vi.fn(),
    update: (file, source) => {
      buffers.set(file, source);
    },
    closeDocument: vi.fn(),
    analyze: async (file) => [
      {
        project: 0,
        config: "config",
        result: {
          entry: file,
          revision: 0,
          status: "complete",
          reports: [],
        },
      },
    ],
    query: async (file) => {
      views.push(new Map(buffers));
      const before = buffers.get(file)!;
      if (file === "broken" && before === "intermediate") {
        throw new Error("cannot continue");
      }
      const after =
        file === "broken"
          ? "intermediate"
          : buffers.get("broken") === "intermediate"
            ? "temporary"
            : "final";
      return [
        {
          project: 0,
          config: "config",
          result: {
            revision: fingerprint(before),
            edits: [
              {
                start: 0,
                end: before.length,
                newText: after,
              },
            ],
          },
        },
      ];
    },
  };
  const result = await new SourceRewriter(workspace).rewrite(
    [
      {
        file: "broken",
        source: "original",
      },
      {
        file: "healthy",
        source: "original",
      },
    ],
    "fixes",
  );
  expect(result.outputs).toMatchObject([
    {
      file: "healthy",
      code: "final",
      edits: [],
    },
  ]);
  expect(result.failures).toMatchObject([
    {
      file: "broken",
      message: "cannot continue",
    },
  ]);
  expect(views[0]).toEqual(
    new Map([
      ["broken", "original"],
      ["healthy", "original"],
    ]),
  );
  expect(views.at(-1)?.get("broken")).toBe("original");
  expect(workspace.closeDocument).toHaveBeenCalledTimes(2);
});

test.each(["cycle", "limit"])(
  "a fix %s fails only the target that cannot converge",
  async (mode) => {
    const buffers = new Map<string, string>();
    const workspace: ConstructorParameters<typeof SourceRewriter>[0] = {
      invalidate: vi.fn(),
      update: (file, source) => {
        buffers.set(file, source);
      },
      closeDocument: vi.fn(),
      analyze: async (file) => [
        {
          project: 0,
          config: "config",
          result: {
            entry: file,
            revision: 0,
            status: "complete",
            reports: [],
          },
        },
      ],
      query: async (file) => {
        const before = buffers.get(file)!;
        const after =
          file === "healthy"
            ? "final"
            : mode === "cycle"
              ? before === "a"
                ? "b"
                : "a"
              : `${before}b`;
        return [
          {
            project: 0,
            config: "config",
            result: {
              revision: fingerprint(before),
              edits: [
                {
                  start: 0,
                  end: before.length,
                  newText: after,
                },
              ],
            },
          },
        ];
      },
    };
    const result = await new SourceRewriter(workspace).rewrite(
      [
        {
          file: "broken",
          source: "a",
        },
        {
          file: "healthy",
          source: "original",
        },
      ],
      "fixes",
    );
    expect(result.outputs).toMatchObject([
      {
        file: "healthy",
        code: "final",
      },
    ]);
    expect(result.failures).toMatchObject([
      {
        file: "broken",
        message: expect.stringContaining(
          mode === "cycle" ? "did not converge" : "exceeded ten passes",
        ),
      },
    ]);
  },
);

test("source edits use original UTF-16 ranges and reject invalid plans", () => {
  expect(
    applySourceEdits("🦀abc", [
      { start: 2, end: 3, newText: "long" },
      { start: 4, end: 5, newText: "C" },
    ]),
  ).toBe("🦀longbC");
  expect(() =>
    applySourceEdits("abc", [
      { start: 0, end: 2, newText: "" },
      { start: 1, end: 3, newText: "" },
    ]),
  ).toThrow("overlapping");
  expect(() =>
    applySourceEdits("abc", [
      {
        start: -1,
        end: 0,
        newText: "",
      },
    ]),
  ).toThrow("Invalid");
  expect(() =>
    applySourceEdits("abc", [
      {
        start: 0,
        end: 4,
        newText: "",
      },
    ]),
  ).toThrow("Invalid");
});
