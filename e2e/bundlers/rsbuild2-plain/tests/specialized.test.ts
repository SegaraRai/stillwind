import { fixtureProfiles } from "@stillwind/e2e-harness";
import { registerScenario } from "@stillwind/e2e-harness/scenario";
import { afterAll, beforeAll, test } from "vite-plus/test";

import fixture from "../fixture.ts";
import { createApplication } from "./support/rsbuild-application.ts";
import * as contract from "./support/rsbuild-contracts.ts";

for (const profile of fixtureProfiles(fixture)) {
  registerScenario(
    fixture.id + "/" + profile,
    new URL("../", import.meta.url),
    (context) => {
      const app = createApplication(context);
      beforeAll(() => app.initialize());
      test.each([
        { stage: "cold", name: "cold build: emits CSS, assets and source maps" },
        { stage: "warm", name: "warm build: restores CSS from cached producer modules" },
        { stage: "css-edit", name: "rebuilds CSS after editing the theme" },
        { stage: "unreachable", name: "removes CSS from unreachable modules" },
        { stage: "macro-removal", name: "retracts CSS after removing a macro" },
      ] as const)("$name", ({ stage }) => contract.expectBuildStage(app, stage));
      afterAll(() => app.dispose());
    },
    [profile],
  );
}
