import { fixtureProfiles } from "@stillwind/e2e-harness";
import { registerScenario } from "@stillwind/e2e-harness/scenario";
import { afterAll, beforeAll, test } from "vite-plus/test";

import fixture from "../fixture.ts";
import { createApplication } from "./support/composition-application.ts";
import * as contract from "./support/composition-contracts.ts";

for (const profile of fixtureProfiles(fixture)) {
  registerScenario(
    "composition/" + profile,
    new URL("../", import.meta.url),
    (context) => {
      const app = createApplication(context);
      beforeAll(() => app.initialize(), 600_000);
      test("resolves composed CSS and hover rules in both lazy loading orders", () =>
        contract.expectLoadingOrder(app));
      test("preserves SSR classes and CSS during hydration", () => contract.expectHydration(app));
      test("updates a composed dependency without changing precedence", () =>
        contract.expectDependencyEdit(app));
      test("updates CSS after changing the composition graph", () =>
        contract.expectTopologyEdit(app));
      test("production: preserves lazy loading order and SSR class identity", () =>
        contract.expectProduction(app));
      afterAll(() => app.dispose());
    },
    [profile],
    fixture.scenarios!.find((scenario) => scenario.name === "composition")!.directory,
  );
}

declare global {
  interface Window {
    goto: (route: string) => Promise<void>;
    hydratedProps: Record<string, string>;
    current: string;
    hydrated: boolean;
  }
}
