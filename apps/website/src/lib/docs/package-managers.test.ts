import { expect, it } from "vite-plus/test";

import { type PackageManager, packageManagerCommand } from "./package-managers";

function commands(line: string) {
  const managers: PackageManager[] = ["vp", "npm", "pnpm", "yarn", "bun"];
  return Object.fromEntries(
    managers.map((manager) => [manager, packageManagerCommand(line, manager)]),
  );
}

it("translates package installs and development dependencies", () => {
  expect(commands("vp i stillwind")).toEqual({
    vp: "vp i stillwind",
    npm: "npm install stillwind",
    pnpm: "pnpm add stillwind",
    yarn: "yarn add stillwind",
    bun: "bun add stillwind",
  });
  expect(commands("vp i -D @stillwind/integration @stillwind/cli")).toEqual({
    vp: "vp i -D @stillwind/integration @stillwind/cli",
    npm: "npm install -D @stillwind/integration @stillwind/cli",
    pnpm: "pnpm add -D @stillwind/integration @stillwind/cli",
    yarn: "yarn add -D @stillwind/integration @stillwind/cli",
    bun: "bun add --dev @stillwind/integration @stillwind/cli",
  });
  expect(commands("vp install")).toMatchObject({
    npm: "npm install",
    yarn: "yarn install",
  });
});

it("runs bins by the bin name", () => {
  expect(commands("vp exec stillwind lint --fix")).toEqual({
    vp: "vp exec stillwind lint --fix",
    npm: "npx stillwind lint --fix",
    pnpm: "pnpm exec stillwind lint --fix",
    yarn: "yarn stillwind lint --fix",
    bun: "bunx stillwind lint --fix",
  });
});

it("keeps lines without a vp command", () => {
  expect(packageManagerCommand("# Check styles", "npm")).toBe("# Check styles");
});

it("rejects commands without a translation", () => {
  expect(() => packageManagerCommand("vp check", "npm")).toThrow("Unsupported vp command");
  expect(() => packageManagerCommand("vp i -E stillwind", "npm")).toThrow(
    "Unsupported install option -E",
  );
});
