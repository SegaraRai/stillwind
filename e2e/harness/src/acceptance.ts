import { fileURLToPath } from "node:url";

import { afterAll, afterEach, beforeAll, beforeEach, describe, test } from "vite-plus/test";

import { FixtureApplication } from "./application.ts";
import { fixtureContracts } from "./contracts.ts";
import {
  defineFixture,
  fixtureProfiles,
  failureReason,
  type Fixture,
  type ExpectedFailure,
} from "./fixture.ts";
import { type CasePolicy, type ContractId } from "./policy.ts";
import { shadowDomContracts } from "./shadow-dom.ts";
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
      const policies = new Map<string, CasePolicy>();
      const declareCase = (
        id: ContractId,
        name: string,
        run: () => Promise<void>,
        options: { skip?: boolean; expectedFailure?: ExpectedFailure } = {},
      ) => {
        const label =
          name + (options.expectedFailure ? ` (${failureReason(options.expectedFailure)})` : "");
        policies.set(label, { contract: id, expectedFailure: options.expectedFailure });
        const declare = options.skip ? test.skip : options.expectedFailure ? test.fails : test;
        declare(label, run);
      };
      beforeAll(() => app.initialize());
      beforeEach(({ task, onTestFinished }) => {
        const name = fullCaseName(task);
        app.beginCase(name);
        onTestFinished(() =>
          app.recordCase(
            name,
            task.result?.state === (task.fails ? "fail" : "pass"),
            task.fails,
            policies.get(task.name),
          ),
        );
      });
      afterEach(({ task }) => app.closeCase(Boolean(task.result?.errors?.length)));
      afterAll(() => app.dispose());

      if (fixture.platformFailure?.platforms.includes(process.platform)) {
        afterAll(() => app.stop());
        declareCase("cssRendering", "builds the application", () => app.build("cold"), {
          expectedFailure: fixture.platformFailure,
        });
        return;
      }

      const renderingCases = () => {
        declareCase("cssRendering", "applies initial CSS", contract.initialCss);
        if (fixture.shadowDom) {
          const shadow = shadowDomContracts(app, fixture.shadowDom);
          declareCase(
            "cssRendering",
            "isolates generated CSS inside each shadow root",
            shadow.isolation,
          );
          declareCase(
            "interactions",
            "updates custom-element attributes and properties independently",
            shadow.properties,
          );
          declareCase(
            "cssRendering",
            "restores shadow CSS after reconnecting an element",
            shadow.reconnect,
          );
        }
        if (fixture.ssr === true || fixture.prerender === true) {
          declareCase(
            "serverRendering",
            "uses matching SSR and client classes",
            contract.classAgreement,
          );
        }
        for (const [name, capability] of [
          ["SSR", fixture.ssr],
          ["prerender", fixture.prerender],
        ] as const) {
          if (capability) {
            declareCase(
              "serverRendering",
              `applies ${name} CSS without JavaScript${capability === true ? "" : ` (${capability.reason})`}`,
              contract.serverRendering,
              { skip: capability !== true },
            );
          }
        }
        declareCase(
          "interactions",
          fixture.hydration === true
            ? "hydrates server-rendered components and responds to interactions"
            : "responds to component interactions",
          contract.interactions,
          { skip: !fixture.page.counters.length },
        );
        for (const probe of fixture.semantics ?? []) {
          declareCase(
            "semantics",
            `${probe.id}: composes styles and resolves diamond dependencies`,
            () => contract.composition(probe),
          );
          declareCase("semantics", `${probe.id}: applies initial condition branches`, () =>
            contract.initialConditions(probe),
          );
          declareCase("semantics", `${probe.id}: inherits hover styles`, () =>
            contract.hover(probe),
          );
          declareCase(
            "semantics",
            `${probe.id}: switches nested condition branches and aria values`,
            () => contract.conditionBranches(probe),
            { skip: !probe.interactive },
          );
        }
        if (fixture.navigation) {
          declareCase("navigation", "applies route CSS during navigation", () =>
            contract.navigation(fixture.navigation!),
          );
        }
      };

      describe("development", () => {
        beforeAll(() => app.start("dev"));
        afterAll(() => app.stop());
        renderingCases();
        for (const mapping of fixture.sourceMaps ?? []) {
          if (mapping.development) {
            declareCase(
              "sourceMaps",
              `maps development JavaScript classes to ${mapping.macro}`,
              () => contract.developmentSourceMap(mapping),
              { expectedFailure: mapping.development.expectedFailure },
            );
          }
        }
        for (const [name, edit] of Object.entries(fixture.edits)) {
          declareCase(
            "sourceUpdate",
            `reflects ${name} edits (${edit.file})`,
            () => contract.sourceEdit(edit),
            { expectedFailure: edit.expectedFailure },
          );
          if (fixture.ssr === true || fixture.prerender === true) {
            declareCase("serverRendering", `renders fresh SSR CSS after ${name} edits`, () =>
              contract.freshServerRendering(edit),
            );
          }
        }
      });
      if (fixture.production !== false && process.env.STILLWIND_E2E_DEV_ONLY !== "1") {
        for (const temperature of ["cold", "warm"] as const) {
          describe(`${temperature} production build`, () => {
            beforeAll(() => app.build(temperature));
            afterAll(() => app.stop());
            renderingCases();
            if (temperature === "warm" && fixture.cachedProducer) {
              declareCase(
                "buildInvalidation",
                "restores a cached framework producer",
                contract.cachedProducer,
              );
            }
            for (const mapping of fixture.sourceMaps ?? []) {
              const issue =
                mapping.expectedFailure && "reason" in mapping.expectedFailure
                  ? (mapping.expectedFailure as ExpectedFailure)
                  : mapping.expectedFailure?.[profile];
              declareCase(
                "sourceMaps",
                `maps generated ${mapping.kind} classes to ${mapping.file}: ${mapping.macro}`,
                () => contract.sourceMap(mapping),
                { expectedFailure: issue },
              );
            }
          });
        }
        describe("production invalidation", () => {
          beforeAll(() => app.build("cold"));
          afterAll(() => app.stop());
          for (const [name, edit] of Object.entries(fixture.edits)) {
            declareCase("buildInvalidation", `rebuilds CSS after ${name} edits`, () =>
              contract.rebuiltCss(edit),
            );
          }
        });
      }
    });
  }
}
