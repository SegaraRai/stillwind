import type { Page } from "playwright";
import { expect } from "vite-plus/test";

import type { Fixture } from "./fixture.ts";

type Probe = NonNullable<Fixture["semantics"]>[number];

/** Checks composed declarations and the initial condition branches, including HTML without JavaScript. */
export async function verifyInitialSemantics(page: Page, probes: Probe[] = []): Promise<void> {
  for (const { id } of probes) {
    await css(page, `${id}-composed`, { marginTop: "12px", paddingTop: "7px" });
    await css(page, `${id}-diamond`, {
      marginTop: "2px",
      paddingTop: "7px",
      color: "rgb(18, 52, 86)",
      backgroundColor: "rgb(171, 205, 239)",
    });
    await css(page, `${id}-conditional`, { marginTop: "40px" });
    await css(page, `${id}-conditional-active`, { marginTop: "16px" });
  }
}

/** Checks inherited hover rules and all runtime branches; each interactive probe ends in its initial state. */
export async function verifySemanticInteractions(page: Page, probes: Probe[] = []): Promise<void> {
  for (const { id, interactive } of probes) {
    await page.getByTestId(`${id}-composed`).hover();
    await css(page, `${id}-composed`, { marginTop: "20px" });
    await page.mouse.move(0, 0);
    await css(page, `${id}-composed`, { marginTop: "12px" });
    if (!interactive) {
      continue;
    }
    for (const [margin, busy] of [
      ["24px", "true"],
      ["16px", "true"],
      ["40px", "false"],
    ]) {
      await page.getByTestId(`${id}-toggle`).click();
      await css(page, `${id}-conditional`, { marginTop: margin });
      await expect
        .poll(() => page.getByTestId(`${id}-conditional`).getAttribute("aria-busy"))
        .toBe(busy);
    }
  }
}

async function css(page: Page, id: string, expected: Record<string, string>) {
  await expect
    .poll(
      () =>
        page.getByTestId(id).evaluate((element, properties) => {
          const actual = getComputedStyle(element);
          return Object.fromEntries(
            properties.map((property) => [property, actual[property as keyof CSSStyleDeclaration]]),
          );
        }, Object.keys(expected)),
      { timeout: 30_000 },
    )
    .toEqual(expected);
}
