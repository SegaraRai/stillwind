import { fixtureProfiles } from "@stillwind-internal/e2e-harness";
import { registerScenario } from "@stillwind-internal/e2e-harness/scenario";
import { afterAll, afterEach, beforeAll, beforeEach, describe, test } from "vite-plus/test";

import fixture from "../fixture.ts";
import { createApplication } from "./support/solidstart-application.ts";
import * as contract from "./support/solidstart-contracts.ts";

for (const profile of fixtureProfiles(fixture)) {
  registerScenario(
    fixture.id + "/" + profile,
    new URL("../", import.meta.url),
    (context) => {
      const app = createApplication(context);
      beforeAll(() => app.initialize(), 600_000);
      describe("development", () => {
        beforeEach(() => app.prepareDevelopmentCase());
        test("renders SSR CSS, updates reactive styles, and loads lazy CSS", () =>
          contract.expectRendering(app));
        test("updates CSS after editing the theme", () => contract.expectThemeUpdate(app));
        test("removes and restores macro CSS", () => contract.expectMacroRetraction(app));
      });
      for (const temperature of ["cold", "warm"] as const) {
        test(`${temperature} production: SSR, hydration, navigation and source maps`, () =>
          contract.expectProduction(app, temperature));
      }
      afterEach(() => app.assertNoBrowserErrors());
      afterAll(() => app.dispose());
    },
    [profile],
  );
}

declare global {
  interface Window {
    __navigationSentinel: number | undefined;
  }
}
