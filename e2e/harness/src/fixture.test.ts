import { expect, test, vi, afterEach } from "vite-plus/test";

import { defineFixture, fixtureProfiles, type Fixture } from "./fixture.ts";

const fixture: Fixture = {
  id: "example",
  driver: "vite",
  versions: [],
  profiles: ["default", "split"],
  page: { path: "/", ready: "#target", target: "#target", css: { color: "red" }, counters: [] },
  ssr: { status: "not-applicable", reason: "Only a client entrypoint exists." },
  hydration: { status: "not-applicable", reason: "Only a client entrypoint exists." },
  edits: {},
};

afterEach(() => vi.unstubAllEnvs());

test("rejects contradictory capabilities and incomplete edit expectations before a run", () => {
  expect(() => defineFixture({ ...fixture, hydration: true })).toThrow("Hydration requires");
  expect(() => defineFixture({ ...fixture, profiles: ["default", "default"] })).toThrow("unique");
  expect(() =>
    defineFixture({
      ...fixture,
      edits: {
        theme: {
          file: "root.css",
          from: "red",
          to: "blue",
          css: { color: "blue" },
          document: "preserve",
          state: "preserve",
        },
      },
    }),
  ).toThrow("requires a counter");
  expect(() =>
    defineFixture({ ...fixture, ssr: { status: "not-applicable", reason: " " } }),
  ).toThrow("requires a reason");
});

test("selects only declared profiles", () => {
  vi.stubEnv("STILLWIND_E2E_PROFILE", "split");
  expect(fixtureProfiles(fixture)).toEqual(["split"]);
  vi.stubEnv("STILLWIND_E2E_PROFILE", "missing");
  expect(() => fixtureProfiles(fixture)).toThrow("Unknown profile");
  vi.stubEnv("STILLWIND_E2E_PROFILE", undefined);
  expect(fixtureProfiles(fixture)).toEqual(["default", "split"]);
});
