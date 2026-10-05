import { fixtureProfiles } from "@stillwind-internal/e2e-harness";
import { registerScenario } from "@stillwind-internal/e2e-harness/scenario";
import { afterAll, beforeAll, test } from "vite-plus/test";

import fixture from "../fixture.ts";
import { createApplication } from "./support/conditional-application.ts";
import * as contract from "./support/conditional-contracts.ts";

for (const profile of fixtureProfiles(fixture)) {
  registerScenario(
    "conditional/" + profile,
    new URL("../", import.meta.url),
    (context) => {
      const app = createApplication(context);
      beforeAll(() => app.initialize(), 600_000);
      test("selects each nested condition branch and its aria value", () =>
        contract.expectConditionBranches(app));
      test("reflects an edit inside a condition branch", () => contract.expectBranchEdit(app));
      test("preserves conditional classes during SSR hydration", () =>
        contract.expectHydration(app));
      test("production: selects condition branches and preserves SSR classes", () =>
        contract.expectProduction(app));
      afterAll(() => app.dispose());
    },
    [profile],
    fixture.scenarios!.find((scenario) => scenario.name === "conditional")!.directory,
  );
}

declare global {
  interface Window {
    setState: (state: { enabled: boolean; nested: boolean }) => void;
    classes: string;
    hydrated: boolean;
  }
}
