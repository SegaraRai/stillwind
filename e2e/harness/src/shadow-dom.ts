import { expect } from "vite-plus/test";

import type { FixtureApplication } from "./application.ts";
import { expectCss } from "./browser.ts";
import type { Fixture } from "./fixture.ts";

/** Creates browser contracts for two custom elements with independent `tone` properties. */
export function shadowDomContracts(
  app: FixtureApplication,
  options: NonNullable<Fixture["shadowDom"]>,
) {
  const [first, second] = options.hosts;
  const dynamic = (host: string) => `${host} [data-testid="dynamic"]`;
  return {
    isolation: async () => {
      const page = await app.open();
      for (const host of options.hosts) {
        await expectCss(page, `${host} ${options.target}`, app.fixture.page.css);
        expect(await page.locator(host).evaluate((element) => element.shadowRoot?.mode)).toBe(
          "open",
        );
      }
      const classes = await page.locator(`${first} ${options.target}`).getAttribute("class");
      expect(classes).toContain("sw_");
      await page.locator(options.outside).evaluate((element, classes) => {
        element.setAttribute("class", classes ?? "");
      }, classes);
      await expectCss(page, options.outside, {
        color: "rgb(1, 2, 3)",
        paddingTop: "0px",
        marginTop: "0px",
      });
    },
    properties: async () => {
      const page = await app.open();
      await expectCss(page, dynamic(first), { color: "rgb(255, 0, 0)" });
      await expectCss(page, dynamic(second), { color: "rgb(0, 128, 0)" });
      await page.locator(first).evaluate((element) => element.setAttribute("tone", "purple"));
      await expectCss(page, dynamic(first), { color: "rgb(128, 0, 128)" });
      await page.locator(first).evaluate((element) => Reflect.set(element, "tone", "blue"));
      await expectCss(page, dynamic(first), { color: "rgb(0, 0, 255)" });
      await expectCss(page, dynamic(second), { color: "rgb(0, 128, 0)" });
    },
    reconnect: async () => {
      const page = await app.open();
      await expectCss(page, `${first} ${options.target}`, app.fixture.page.css);
      await page.locator(first).evaluate(async (element) => {
        const parent = element.parentElement!;
        element.remove();
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
        parent.append(element);
      });
      await expectCss(page, `${first} ${options.target}`, app.fixture.page.css);
      await expectCss(page, dynamic(first), { color: "rgb(255, 0, 0)" });
    },
  };
}
