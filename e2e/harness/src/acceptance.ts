import { fileURLToPath } from "node:url";

import { afterAll, afterEach, beforeAll, beforeEach, describe, test } from "vite-plus/test";

import { FixtureApplication } from "./application.ts";
import { fixtureContracts } from "./contracts.ts";
import { defineFixture, fixtureProfiles, type Fixture } from "./fixture.ts";
import { fullCaseName } from "./suite.ts";

/**
 * Declares the shared browser contracts for every selected fixture profile.
 */
export function registerFixture(input: Fixture, packageUrl: URL): void {
  const fixture = defineFixture(input);
  for (const profile of fixtureProfiles(fixture)) {
    describe(`${fixture.id}/${profile}`, () => {
      const app = new FixtureApplication(fixture, fileURLToPath(packageUrl), profile);
      const contract = fixtureContracts(app);
      beforeAll(() => app.initialize());
      beforeEach(({ task, onTestFinished }) => {
        const name = fullCaseName(task);
        app.beginCase(name);
        onTestFinished(() => app.recordCase(name, task.result?.state === "pass"));
      });
      afterEach(({ task }) => app.closeCase(Boolean(task.result?.errors?.length)));
      afterAll(() => app.dispose());

      const renderingCases = () => {
        test("applies initial CSS", contract.initialCss);
        for (const [name, capability] of [
          ["SSR", fixture.ssr],
          ["prerender", fixture.prerender],
        ] as const) {
          if (capability) {
            test.skipIf(capability !== true)(
              `applies ${name} CSS without JavaScript${capability === true ? "" : ` (${capability.reason})`}`,
              contract.serverRendering,
            );
          }
        }
        test.skipIf(!fixture.page.counters.length)(
          fixture.hydration === true
            ? "hydrates server-rendered components and responds to interactions"
            : "responds to component interactions",
          contract.interactions,
        );
        for (const probe of fixture.semantics ?? []) {
          test(`${probe.id}: composes styles and resolves diamond dependencies`, () =>
            contract.composition(probe));
          test(`${probe.id}: applies initial condition branches`, () =>
            contract.initialConditions(probe));
          test(`${probe.id}: inherits hover styles`, () => contract.hover(probe));
          test.skipIf(!probe.interactive)(
            `${probe.id}: switches nested condition branches and aria values`,
            () => contract.conditionBranches(probe),
          );
        }
        if (fixture.navigation) {
          test("applies route CSS during navigation", () =>
            contract.navigation(fixture.navigation!));
        }
      };

      describe("development", () => {
        beforeAll(() => app.start("dev"));
        afterAll(() => app.stop());
        renderingCases();
        for (const [name, edit] of Object.entries(fixture.edits)) {
          test(`reflects ${name} edits (${edit.file})`, () => contract.sourceEdit(edit));
        }
      });
      if (fixture.production !== false && process.env.STILLWIND_E2E_DEV_ONLY !== "1") {
        for (const temperature of ["cold", "warm"] as const) {
          describe(`${temperature} production build`, () => {
            beforeAll(() => app.build(temperature));
            afterAll(() => app.stop());
            renderingCases();
          });
        }
      }
    });
  }
}
