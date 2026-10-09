import { expect, test } from "vite-plus/test";

import { builtinArgs, configArgs, outputArgs, parseInvocation } from "./arguments.ts";

const definitions = {
  ...configArgs,
  ...outputArgs,
  ...builtinArgs,
};

test("command selection distinguishes option values and keeps leading options", () => {
  expect(
    parseInvocation(["--config", "lint", "lint", "--format", "json"], definitions),
  ).toMatchObject({
    name: "lint",
    rawArgs: ["--config", "lint", "--format", "json"],
  });
});

test("compact string options do not turn characters in their value into builtin flags", () => {
  expect(parseInvocation(["-cpath", "lint"], definitions)).toMatchObject({
    name: "lint",
    rawArgs: ["-cpath"],
    help: undefined,
  });
});

test("the option terminator keeps flag-shaped globs and ends command selection", () => {
  expect(parseInvocation(["fmt", "--", "--help", "--format=json"], definitions)).toMatchObject({
    name: "fmt",
    rawArgs: ["--", "--help", "--format=json"],
    help: undefined,
  });
  expect(parseInvocation(["--", "lint"], definitions).name).toBeUndefined();
});
