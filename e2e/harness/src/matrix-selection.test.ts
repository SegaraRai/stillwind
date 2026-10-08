import { expect, test, vi, afterEach } from "vite-plus/test";

import { fixtureProfiles, fixtureSupport, failureReason, type FixtureManifest } from "./fixture.ts";
import { matrixOptions, selectedProfiles, supportDocument } from "./matrix-selection.ts";
import { casePolicy } from "./policy.ts";

const fixture: FixtureManifest = {
  id: "next",
  profiles: ["turbopack", "webpack", "standalone"],
  support: {
    tier: "primary",
    profiles: {
      standalone: "compatibility",
    },
  },
};

afterEach(() => vi.unstubAllEnvs());

test("selects actual host profiles without dropping the other tier from the full matrix", () => {
  expect(selectedProfiles(fixture)).toEqual(fixture.profiles);
  expect(selectedProfiles(fixture, "primary")).toEqual(["turbopack", "webpack"]);
  expect(selectedProfiles(fixture, "compatibility")).toEqual(["standalone"]);
  expect(selectedProfiles(fixture, "primary", "standalone")).toEqual([]);
  vi.stubEnv("STILLWIND_E2E_SUPPORT", "compatibility");
  vi.stubEnv("STILLWIND_E2E_PROFILE", undefined);
  expect(fixtureProfiles(fixture)).toEqual(["standalone"]);
  vi.stubEnv("STILLWIND_E2E_PROFILE", "webpack");
  expect(fixtureProfiles(fixture)).toEqual([]);
});

test("rejects invalid CLI selectors and misspelled profile overrides", () => {
  expect(matrixOptions(["next", "--support", "primary", "--list"])).toMatchObject({
    selected: ["next"],
    support: "primary",
    list: true,
  });
  expect(() => matrixOptions(["--support"])).toThrow("requires");
  expect(() => matrixOptions(["--support", "minor"])).toThrow("requires");
  expect(() => matrixOptions(["--unknown"])).toThrow("Unknown");
  expect(() =>
    fixtureSupport(
      {
        ...fixture,
        support: {
          tier: "primary",
          profiles: {
            typo: "compatibility",
          },
        },
      },
      "webpack",
    ),
  ).toThrow("Unknown support profile");
});

test("reports source map limitations separately from high priority CSS contracts", () => {
  const failure = {
    reason: "Host omits selector positions",
    issue: "https://example.com/issues/1",
  };
  expect(
    casePolicy({
      contract: "sourceMaps",
      expectedFailure: failure,
    }),
  ).toEqual({
    contract: "sourceMaps",
    priority: "low",
    expectedFailure: failure,
  });
  expect(casePolicy({ contract: "sourceUpdate" }).priority).toBe("high");
  expect(() => failureReason({ reason: " " })).toThrow("requires a reason");
  const doc = supportDocument([
    {
      ...fixture,
      sourceMaps: [
        {
          file: "src/App.vue",
          macro: "style('p-2')",
          kind: "css",
          expectedFailure: {
            webpack: failure,
          },
        },
      ],
    },
  ]);
  expect(doc).toContain("| next | standalone | compatibility |");
  expect(doc).toContain("| next | webpack | sourceMaps (production)");
  expect(doc).toContain("[issue](https://example.com/issues/1)");
  expect(doc).not.toContain("| next | turbopack | sourceMaps (production)");
});
