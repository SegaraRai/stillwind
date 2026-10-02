import { fixtureProfiles } from "@stillwind/e2e-harness";
import { registerScenario } from "@stillwind/e2e-harness/scenario";
import { afterAll, afterEach, beforeAll, beforeEach, describe, test } from "vite-plus/test";

import fixture from "../fixture.ts";
import { createApplication } from "./support/next-application.ts";
import * as contract from "./support/next-contracts.ts";

for (const profile of fixtureProfiles(fixture)) {
  registerScenario(
    fixture.id + "/" + profile,
    new URL("../", import.meta.url),
    (context) => {
      const app = createApplication(context);
      beforeAll(() => app.initialize(), 600_000);
      describe("development", () => {
        beforeEach(() => app.prepareDevelopmentCase());
        test("hydrates an RSC page and applies client CSS", () => contract.expectRscHydration(app));
        test("updates theme CSS while preserving client state", () =>
          contract.expectThemeUpdate(app));
        test("removes and restores server component macro CSS", () =>
          contract.expectMacroRetraction(app));
        test("loads CSS on client navigation", () => contract.expectLazyNavigation(app));
        test("updates CSS after editing cross-module composition", () =>
          contract.expectCompositionEdit(app));
        test.runIf(app.mode === "turbopack" && !app.standalone)(
          "adds and retracts CSS when a route is added and deleted",
          () => contract.expectRouteRetraction(app),
        );
      });
      for (const temperature of ["cold", "warm"] as const) {
        test(`${temperature} production: SSR, hydration, navigation and source maps`, () =>
          contract.expectProduction(app, temperature));
      }
      afterEach(() => app.assertNoBrowserErrors());
      afterAll(() => app.dispose());
    },
    profile === "standalone" ? ["turbopack", "standalone"] : [profile],
  );
}
