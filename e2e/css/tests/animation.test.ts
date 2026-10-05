import { registerScenario } from "@stillwind-internal/e2e-harness/scenario";
import { afterAll, beforeAll, test } from "vite-plus/test";

import { createApplication } from "./support/animation-application.ts";
import * as contract from "./support/animation-contracts.ts";

registerScenario("animation", new URL("../", import.meta.url), (context) => {
  const app = createApplication(context);
  beforeAll(() => app.initialize());
  for (const [reducedMotion, expected] of [
    ["no-preference", [0, 1]],
    ["reduce", [0.25, 0.5]],
  ] as const) {
    test(`uses complete animation keyframes for reduced-motion=${reducedMotion}`, () =>
      contract.expectKeyframes(app, reducedMotion, expected));
  }
  afterAll(() => app.dispose());
});
