import { fixtureProfiles } from "@stillwind-internal/e2e-harness";
import { registerScenario } from "@stillwind-internal/e2e-harness/scenario";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, test } from "vite-plus/test";

import fixture from "../fixture.ts";
import { createApplication } from "./support/react-router-application.ts";
import * as contract from "./support/react-router-contracts.ts";

for (const profile of fixtureProfiles(fixture)) {
  registerScenario(
    fixture.id + "/" + profile,
    new URL("../", import.meta.url),
    (context) => {
      const app = createApplication(context);
      beforeAll(() => app.initialize(), 600_000);
      describe("development", () => {
        beforeEach(() => app.prepareDevelopmentCase());
        test("renders SSR CSS, hydrates, and loads CSS during navigation", () =>
          contract.expectRendering(app));
        test("updates CSS after editing the theme", () => contract.expectThemeUpdate(app));
        test("removes and restores macro CSS", () => contract.expectMacroRetraction(app));
        test("updates projected CSS after editing a server module", () =>
          contract.expectServerProjectionEdit(app));
      });
      for (const temperature of ["cold", "warm"] as const) {
        test(`${temperature} production: SSR, hydration, navigation and source maps`, () =>
          contract.expectProduction(app, temperature));
      }
      test("production rejects a server import retained by client code", async () => {
        await app.stop();
        await app.write("app/routes/home.jsx", app.home + '\nimport "../server.server";\n');
        try {
          await expect(app.compile()).rejects.toThrow("Server-only module referenced by client");
        } finally {
          await app.write("app/routes/home.jsx", app.home);
        }
      });
      afterEach(() => app.assertNoBrowserErrors());
      afterAll(() => app.dispose());
    },
    [profile],
  );
}

declare global {
  interface Window {
    __navigationSentinel: number | undefined;
    __stillwindServerMarker: unknown;
  }
}
