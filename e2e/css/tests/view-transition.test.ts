import { registerScenario } from "@stillwind-internal/e2e-harness/scenario";
import { afterAll, beforeAll, test } from "vite-plus/test";

import { createApplication } from "./support/view-transition-application.ts";
import * as contract from "./support/view-transition-contracts.ts";

registerScenario("view-transition", new URL("../", import.meta.url), (context) => {
  const app = createApplication(context);
  beforeAll(() => app.initialize());
  beforeAll(() => app.initializePage());
  test("names participants by literal and runtime keys and resolves condition branches", () =>
    contract.expectParticipantNames(app));
  test("applies the group duration through its class token", () =>
    contract.expectGroupDuration(app));
  afterAll(() => app.dispose());
});
