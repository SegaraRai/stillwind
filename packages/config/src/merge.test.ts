import { expect, test } from "vite-plus/test";

import type { BaseConfig } from "./compiler.ts";
import type { ProjectionProvider } from "./configuration.ts";
import { mergeConfig } from "./merge.ts";

test("configuration merge replaces arrays and promises", () => {
  expect(
    mergeConfig(
      {
        files: ["a"],
        automatic: true,
      },
      {
        files: ["b"],
        automatic: false,
      },
    ),
  ).toEqual({
    files: ["b"],
    automatic: false,
  });
  expect(
    mergeConfig(
      {
        kind: "custom",
        root: ".",
      },
      {
        kind: undefined,
      },
    ),
  ).toEqual({
    kind: "custom",
    root: ".",
  });
  const tailwind = Promise.resolve({
    darkMode: "class" as const,
  });
  expect(
    mergeConfig(
      {
        tailwind: Promise.resolve({}),
      },
      {
        tailwind,
      },
    ).tailwind,
  ).toBe(tailwind);
});

test("nested overrides preserve naming callbacks and replace providers when kind changes", () => {
  /**
   * Supplies a stable utility callback for checking identity preservation during merging.
   *
   * @returns The fixture's utility identifier.
   */
  const utilityClass = () => "utility";
  /**
   * Supplies a separate marker callback for checking identity preservation during merging.
   *
   * @returns The fixture's marker identifier.
   */
  const refMarker = () => "marker";
  const base: BaseConfig = {
    naming: {
      utilityClass,
      refMarker,
    },
  };
  expect(
    mergeConfig(base, {
      naming: {
        utilityClass: () => "override",
        refMarker: undefined,
      },
    }).naming?.refMarker,
  ).toBe(refMarker);
  const project: {
    /** Provider whose changed kind must replace the previous preparation callback. */
    projection: ProjectionProvider;
  } = {
    projection: {
      kind: "custom",
      prepare: async () => expect.fail("Preparation should not execute"),
    },
  };
  expect(
    mergeConfig(project, {
      projection: {
        kind: "framework",
      },
    }),
  ).toEqual({
    projection: {
      kind: "framework",
    },
  });
});
