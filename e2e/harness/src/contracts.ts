import { setTimeout as delay } from "node:timers/promises";

import type { Page } from "playwright";
import { expect } from "vite-plus/test";

import type { FixtureApplication } from "./application.ts";
import { expectCss, expectText, expectDocument, markDocument } from "./browser.ts";
import type { Edit, Fixture } from "./fixture.ts";
import {
  verifyInitialSemantics,
  verifyComposition,
  verifyInitialConditions,
  verifyHover,
  verifyConditionBranches,
} from "./semantics.ts";
import { editFile } from "./workspace.ts";

type Probe = NonNullable<Fixture["semantics"]>[number];

/**
 * Creates observable contracts for an initialized application; each contract opens its own page.
 */
export function fixtureContracts(app: FixtureApplication) {
  const fixture = app.fixture;
  async function initialCss(page: Page): Promise<void> {
    await expectCss(page, fixture.page.target, fixture.page.css);
    for (const probe of fixture.page.styles ?? []) {
      await expectCss(page, probe.selector, probe.css);
    }
  }
  async function interactivePage(): Promise<Page> {
    const page = await app.open();
    await page.locator(fixture.page.ready).waitFor();
    return page;
  }
  async function clickCounters(page: Page): Promise<void> {
    for (const counter of fixture.page.counters) {
      expect(await page.locator(counter.selector).textContent()).toBe(counter.initial);
      await page.locator(counter.selector).click();
      await expectText(page, counter.selector, counter.clicked);
    }
  }
  return {
    initialCss: async () => initialCss(await app.open()),
    serverRendering: async () => {
      const page = await app.open(false);
      await initialCss(page);
      await verifyInitialSemantics(page, fixture.semantics);
      for (const counter of fixture.page.counters) {
        expect(await page.locator(counter.selector).textContent()).toBe(counter.initial);
      }
      await app.captureHtml(page);
    },
    interactions: async () => {
      const page = await interactivePage();
      await clickCounters(page);
      await initialCss(page);
    },
    composition: async (probe: Probe) => verifyComposition(await app.open(), [probe]),
    initialConditions: async (probe: Probe) => verifyInitialConditions(await app.open(), [probe]),
    hover: async (probe: Probe) => verifyHover(await interactivePage(), [probe]),
    conditionBranches: async (probe: Probe) =>
      verifyConditionBranches(await interactivePage(), [probe]),
    navigation: async (navigation: NonNullable<Fixture["navigation"]>) => {
      const page = await app.open();
      const marker = await markDocument(page);
      await page.locator(navigation.link).click();
      await expectCss(page, navigation.target, navigation.css);
      await expectDocument(page, marker, navigation.document);
      await page.locator(navigation.back).click();
      await initialCss(page);
    },
    sourceEdit: async (edit: Edit) => {
      const page = await interactivePage();
      await initialCss(page);
      await clickCounters(page);
      const marker = await markDocument(page);
      const restore = await editFile(app.directory, edit);
      try {
        await expectCss(page, edit.target || fixture.page.target, edit.css);
        await delay(400);
        await expectDocument(page, marker, edit.document);
        if (edit.state !== "not-applicable") {
          for (const counter of fixture.page.counters) {
            await expectText(
              page,
              counter.selector,
              edit.state === "preserve" ? counter.clicked : counter.initial,
            );
          }
        }
      } finally {
        await restore();
      }
      await initialCss(page);
      await app.navigateHome(page);
      await initialCss(page);
    },
  };
}
