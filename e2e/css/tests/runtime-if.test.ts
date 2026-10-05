import { registerScenario } from "@stillwind-internal/e2e-harness/scenario";
import { afterAll, beforeAll, test } from "vite-plus/test";

import { createApplication } from "./support/runtime-if-application.ts";
import * as contract from "./support/runtime-if-contracts.ts";

registerScenario("runtime-if", new URL("../", import.meta.url), (context) => {
  const app = createApplication(context);
  beforeAll(() => app.initialize());
  for (const useCssIf of [false, true]) {
    test(`production: applies computed-value conditions with useCssIf=${useCssIf}`, () =>
      contract.expectComputedConditions(app, useCssIf));
  }
  afterAll(() => app.dispose());
});
