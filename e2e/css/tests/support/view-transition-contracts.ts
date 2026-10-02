import { expect } from "vite-plus/test";

import type { ViewTransitionApplication } from "./view-transition-application.ts";

export async function expectParticipantNames(app: ViewTransitionApplication): Promise<void> {
  const names = await app.page.evaluate(() => {
    const read = (id: string) =>
      getComputedStyle(document.querySelector<HTMLElement>(`#${id}`)!).viewTransitionName;
    return {
      static: read("static"),
      dynamic: read("dynamic"),
      unioned: read("unioned"),
      guarded: read("guarded"),
      guardedTogetherClass: getComputedStyle(
        document.querySelector<HTMLElement>("#guardedTogether")!,
      ).getPropertyValue("view-transition-class"),
      unnamed: read("unnamed"),
    };
  });
  expect(names.static, JSON.stringify(names)).toMatch(/^sw-vt-[0-9a-z]+-hero$/);
  expect(names.dynamic).toBe(`${names.static.replace(/-hero$/, "")}-item\\ 1`);
  expect(names.unioned, JSON.stringify(names)).toMatch(/-two$/);
  expect(names.guarded, JSON.stringify(names)).toMatch(/-guarded$/);
  expect(names.guardedTogetherClass.split(/\s+/).length, JSON.stringify(names)).toBe(3);
  expect(names.unnamed, JSON.stringify(names)).toBe("none");
  expect(names.static).not.toBe(names.dynamic);
}

export async function expectGroupDuration(app: ViewTransitionApplication): Promise<void> {
  const duration = await app.page.evaluate(async () => {
    const token = getComputedStyle(
      document.querySelector<HTMLElement>("#static")!,
    ).viewTransitionName;
    const transition = document.startViewTransition(() => {
      document.querySelector<HTMLElement>("#static")!.textContent = "changed";
    });
    await transition.ready;
    // The pseudo-element itself is reported by participant name; the rule that reaches
    // it was written for the class token, which is what the emitted stylesheet carries.
    const animation = document
      .getAnimations()
      .find(
        (entry) =>
          (entry.effect as KeyframeEffect | null)?.pseudoElement ===
          `::view-transition-group(${token})`,
      );
    const value = animation?.effect?.getTiming().duration;
    await transition.finished;
    return value;
  });
  expect(duration, "the group rule that matches by class token applies").toBe(100);
}
