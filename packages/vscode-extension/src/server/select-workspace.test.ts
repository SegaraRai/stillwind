import { expect, test } from "vite-plus/test";

import { selectWorkspace } from "./select-workspace.ts";

test("a requested configuration selects its workspace among several", () => {
  const workspaces = new Map([
    ["/first.ts", "first"],
    ["/second.ts", "second"],
  ]);
  expect(selectWorkspace(workspaces, new Map(), "Build", "/second.ts")).toEqual([
    "/second.ts",
    "second",
  ]);
});

test("a requested configuration reports its failure or its absence", () => {
  const workspaces = new Map([["/first.ts", "first"]]);
  const failures = new Map([["/broken.ts", "Error: broken"]]);
  expect(() => selectWorkspace(workspaces, failures, "Build", "/broken.ts")).toThrow(
    "/broken.ts: Error: broken",
  );
  expect(() => selectWorkspace(workspaces, failures, "Build", "/absent.ts")).toThrow(
    "Build cannot find the Stillwind workspace /absent.ts",
  );
});

test("omission selects one workspace regardless of failures in other configurations", () => {
  const workspaces = new Map([["/first.ts", "first"]]);
  expect(selectWorkspace(workspaces, new Map([["other", "broken"]]), "Build")).toEqual([
    "/first.ts",
    "first",
  ]);
});

test("without a workspace the error shows why the configuration did not open", () => {
  expect(() =>
    selectWorkspace(new Map(), new Map([["/stillwind.config.ts", "Error: broken"]]), "Build"),
  ).toThrow("/stillwind.config.ts: Error: broken");
  expect(() => selectWorkspace(new Map(), new Map(), "Build")).toThrow(
    "Build requires a Stillwind configuration",
  );
});

test("omission reports several workspaces by count", () => {
  const workspaces = new Map([
    ["/first.ts", "first"],
    ["/second.ts", "second"],
  ]);
  expect(() => selectWorkspace(workspaces, new Map(), "Build")).toThrow(
    "Build accepts one Stillwind workspace, but 2 are open",
  );
});
