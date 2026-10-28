import { flushSync } from "react-dom";
import { style } from "stillwind/react";

/** Moves and resizes a named element between its old and new layout. */
export const layout = style.viewTransition({
  label: "layout",
  group: "duration-300 ease-out motion-reduce:duration-0",
});

/**
 * Applies a React state update inside a view transition. Browsers without the View Transition API
 * apply the update without animation.
 */
export function transition(update: () => void) {
  if (!document.startViewTransition) {
    update();
    return;
  }
  // The browser captures the new state when the callback returns, so React must render first.
  const viewTransition = document.startViewTransition(() => flushSync(update));
  // The browser can skip the animation, for example while the document is hidden. The update
  // still runs, and only the `ready` promise rejects.
  viewTransition.ready.catch(() => {});
}
