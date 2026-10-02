import { expect } from "vite-plus/test";

import type { AnimationApplication } from "./animation-application.ts";

export async function expectKeyframes(
  app: AnimationApplication,
  reducedMotion: "no-preference" | "reduce",
  expected: readonly number[],
): Promise<void> {
  const page = await app.browser.newPage();
  await page.setContent(
    `<style>${app.state.output.css} #target {opacity:.25;animation-duration:1s;animation-fill-mode:both;animation-play-state:paused;}</style><div id="target" class="${app.state.props.className}"></div>`,
  );
  await page.emulateMedia({ reducedMotion });
  const values = await page.evaluate(async () => {
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
    const target = document.querySelector<HTMLElement>("#target")!;
    const animation = target.getAnimations()[0];
    animation.currentTime = 0;
    const start = Number(getComputedStyle(target).opacity);
    animation.currentTime = 1000;
    return [start, Number(getComputedStyle(target).opacity)];
  });
  expect(values, reducedMotion).toEqual(expected);
  await page.close();
}
