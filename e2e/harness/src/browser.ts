import { randomUUID } from "node:crypto";

import type { Page } from "playwright";
import { expect } from "vite-plus/test";

/**
 * Waits until all named computed CSS properties on selector match expected.
 */
export async function expectCss(
  page: Page,
  selector: string,
  expected: Record<string, string>,
): Promise<void> {
  await expect
    .poll(
      () =>
        page.locator(selector).evaluate((element, properties) => {
          const style = getComputedStyle(element);
          return Object.fromEntries(
            properties.map((property) => [property, style[property as keyof CSSStyleDeclaration]]),
          );
        }, Object.keys(expected)),
      { timeout: 30_000, message: `CSS on ${selector}` },
    )
    .toEqual(expected);
}

/**
 * Waits until selector has the expected text content.
 */
export async function expectText(page: Page, selector: string, expected: string): Promise<void> {
  await expect
    .poll(() => page.locator(selector).textContent(), {
      timeout: 30_000,
      message: `text on ${selector}`,
    })
    .toBe(expected);
}

/**
 * Marks the current document so a later assertion can detect a full reload.
 */
export async function markDocument(page: Page): Promise<string> {
  const value = randomUUID();
  await page.evaluate(
    (value) => Object.defineProperty(window, "__stillwindDocument", { value, configurable: true }),
    value,
  );
  return value;
}

/**
 * Reads the current document's marker, or undefined after an unmarked document replaces it.
 */
export async function readDocumentMarker(page: Page): Promise<string | undefined> {
  return page.evaluate(
    () => (window as Window & { __stillwindDocument?: string }).__stillwindDocument,
  );
}

/**
 * Waits for document preservation or replacement according to the declared edit or navigation contract.
 */
export async function expectDocument(
  page: Page,
  marker: string,
  behavior: "preserve" | "reload",
): Promise<void> {
  await expect
    .poll(() => readDocumentMarker(page), { timeout: 30_000, message: "document identity" })
    .toBe(behavior === "preserve" ? marker : undefined);
}

/**
 * Waits for fixtures that mark the document after their client framework mounts.
 */
export async function waitForHydration(page: Page): Promise<void> {
  await page.waitForFunction(() => document.documentElement.dataset.hydrated === "yes");
}

/**
 * Waits for an existing element's className, including an empty value after macro removal.
 */
export async function expectClassName(
  page: Page,
  selector: string,
  expected: string,
): Promise<void> {
  await expect
    .poll(() => page.locator(selector).evaluate((element) => element.className), {
      timeout: 30_000,
      message: `className on ${selector}`,
    })
    .toBe(expected);
}
