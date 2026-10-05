import { registerScenario } from "@stillwind-internal/e2e-harness/scenario";
import { afterAll, beforeAll, describe, test } from "vite-plus/test";

import { createApplication } from "./support/emission-application.ts";
import * as contract from "./support/emission-contracts.ts";

registerScenario("emission", new URL("../", import.meta.url), (context) => {
  const app = createApplication(context);
  beforeAll(() => app.initialize());
  test("development: composes lazy CSS in both loading orders and shares Shadow DOM sheets", () =>
    contract.expectLoadingOrder(app));
  test("development: updates only CSS while retaining the style node and document", () =>
    contract.expectCssOnlyUpdate(app));
  test("development: retracts CSS when a module is deleted", () =>
    contract.expectModuleRetraction(app));
  describe("production", () => {
    beforeAll(() => app.prepareProduction());
    for (const [emission, cssMinify] of [
      ["single", false],
      ["split", false],
      ["single", true],
      ["split", true],
    ] as const) {
      test(`${emission}, minify=${cssMinify}: emits CSS, deduplicates shared rules and styles SSR HTML`, () =>
        contract.expectProduction(app, emission, cssMinify));
    }
  });
  for (const mode of ["dev", "build"]) {
    test(`Web Components ${mode}: adopts explicit CSS without document styles`, () =>
      contract.expectManualCss(app, mode));
  }
  afterAll(() => app.dispose());
});

declare global {
  interface Window {
    goto: (route: string) => Promise<void>;
    sharedSheet: boolean;
    originalStyle: Element | null;
  }
}
