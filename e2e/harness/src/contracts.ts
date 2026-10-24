import { mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { pathToFileURL } from "node:url";

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
import { verifyMacroSourceMap } from "./source-map.ts";
import { editFile } from "./workspace.ts";

type Probe = NonNullable<Fixture["semantics"]>[number];

/**
 * Creates observable contracts for an initialized application; each contract opens its own page.
 */
export function fixtureContracts(app: FixtureApplication) {
  const fixture = app.fixture;
  async function targetCss(
    page: Page,
    css: Record<string, string>,
    target?: string,
  ): Promise<void> {
    const shadow = fixture.shadowDom;
    const targets =
      shadow && !target
        ? shadow.hosts.map((host) => `${host} ${shadow.target}`)
        : [target ?? fixture.page.target];
    for (const selector of targets) {
      await expectCss(page, selector, css);
    }
  }
  async function initialCss(page: Page): Promise<void> {
    await targetCss(page, fixture.page.css);
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
    classAgreement: async () => {
      const server = await app.open(false);
      const classes = (page: Page) =>
        page.locator('[class*="sw_"]').evaluateAll((elements) =>
          elements
            .map((element) => ({
              tag: element.tagName,
              id: element.getAttribute("data-testid") ?? element.id,
              classes: [...element.classList].filter((name) => name.startsWith("sw_")).sort(),
            }))
            .filter((element) => element.classes.length),
        );
      const expected = await classes(server);
      await app.closeCase(false);
      const client = await interactivePage();
      await initialCss(client);
      await expect.poll(() => classes(client)).toEqual(expected);
    },
    freshServerRendering: async (edit: Edit) => {
      const page = await app.open(false);
      await initialCss(page);
      const restore = await editFile(app.directory, edit);
      try {
        await expect
          .poll(
            async () => {
              await app.navigateHome(page);
              return page
                .locator(edit.target || fixture.page.target)
                .evaluate(
                  (element, properties) =>
                    Object.fromEntries(
                      properties.map((property) => [
                        property,
                        getComputedStyle(element)[property as keyof CSSStyleDeclaration],
                      ]),
                    ),
                  Object.keys(edit.css),
                );
            },
            { timeout: 30_000 },
          )
          .toEqual(edit.css);
      } finally {
        await restore();
        await expect
          .poll(
            async () => {
              await app.navigateHome(page);
              return page
                .locator(fixture.page.target)
                .evaluate(
                  (element, properties) =>
                    Object.fromEntries(
                      properties.map((property) => [
                        property,
                        getComputedStyle(element)[property as keyof CSSStyleDeclaration],
                      ]),
                    ),
                  Object.keys(fixture.page.css),
                );
            },
            { timeout: 30_000 },
          )
          .toEqual(fixture.page.css);
      }
    },
    interactions: async () => {
      const page = await interactivePage();
      await clickCounters(page);
      await initialCss(page);
      for (const update of fixture.page.updates ?? []) {
        await page.locator(update.trigger).click();
        await expectCss(page, update.target, update.css);
      }
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
    cachedProducer: async () => {
      type Module = { name?: string; identifier?: string; built?: boolean; modules?: Module[] };
      const stats = JSON.parse(await readFile(join(app.directory, "build-stats.json"), "utf8")) as {
        modules: Module[];
      };
      const flatten = (modules: Module[]): Module[] =>
        modules.flatMap((module) => [module, ...flatten(module.modules ?? [])]);
      const producers = [
        ...new Map(
          flatten(stats.modules)
            .filter((module) =>
              module.name?.replaceAll("\\", "/").endsWith(fixture.cachedProducer!),
            )
            .map((module) => [module.identifier ?? module.name, module]),
        ).values(),
      ];
      expect(producers, "The selected producer must appear in build stats").toHaveLength(1);
      expect(producers[0].built, JSON.stringify(producers)).toBe(false);
    },
    developmentSourceMap: async (mapping: NonNullable<Fixture["sourceMaps"]>[number]) => {
      const page = await app.open();
      await initialCss(page);
      const directory = join(app.directory, ".development-maps");
      await mkdir(directory, { recursive: true });
      const scripts = await page
        .locator("script[src]")
        .evaluateAll((elements) => elements.map((element) => (element as HTMLScriptElement).src));
      for (const url of scripts) {
        const response = await page.request.get(url);
        expect(response.ok(), url).toBe(true);
        const code = await response.text();
        const reference = [...code.matchAll(/^[ \t]*\/\/[#@] sourceMappingURL=([^\s]+)/gm)].at(
          -1,
        )?.[1];
        if (!reference || reference.startsWith("data:")) {
          continue;
        }
        const map = await page.request.get(new URL(reference, url).href);
        expect(map.ok(), reference).toBe(true);
        await writeFile(join(directory, basename(new URL(url).pathname)), code);
        await writeFile(join(directory, basename(reference)), await map.body());
      }
      const source = await readFile(join(app.directory, mapping.file), "utf8");
      await verifyMacroSourceMap(directory, mapping.file, source, mapping.macro);
    },
    sourceMap: async (mapping: NonNullable<Fixture["sourceMaps"]>[number]) => {
      const source = await readFile(join(app.directory, mapping.file), "utf8");
      await verifyMacroSourceMap(
        join(app.directory, "dist"),
        mapping.file,
        source,
        mapping.macro,
        mapping.kind === "css",
        undefined,
        "start",
        pathToFileURL(join(app.directory, mapping.file)).href,
      );
    },
    rebuiltCss: async (edit: Edit) => {
      await app.stop();
      const restore = await editFile(app.directory, edit);
      try {
        await app.build("warm");
        await targetCss(await app.open(), edit.css, edit.target);
      } finally {
        await restore();
      }
    },
    sourceEdit: async (edit: Edit) => {
      const page = await interactivePage();
      await initialCss(page);
      await clickCounters(page);
      const marker = await markDocument(page);
      const restore = await editFile(app.directory, edit);
      try {
        await targetCss(page, edit.css, edit.target);
        await delay(400);
        await expectDocument(page, marker, edit.document);
        if (edit.state !== "not-applicable") {
          for (const counter of fixture.page.counters) {
            const edited = !edit.counters || edit.counters.includes(counter.selector);
            await expectText(
              page,
              counter.selector,
              !edited || edit.state === "preserve" ? counter.clicked : counter.initial,
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
