import { clearHeaderState, setHeaderState } from "../site/header";

/**
 * Visibility of the floating header and the scroll positions that decide its next change.
 */
export interface HeaderScroll {
  visible: boolean;
  /** Scroll position where the header last changed or the current scroll direction began, in pixels. */
  anchor: number;
  /** Scroll position of the previous update, in pixels. */
  previous: number;
}

/** Distance in pixels that the page must scroll against the current direction to change visibility. */
const TOLERANCE = 8;

/**
 * Returns the next visibility of the floating header for scroll position `y`. Scrolling down
 * hides the header, and scrolling up by more than the tolerance shows it. A jump farther than
 * `viewportHeight`, such as one to a fragment or the page end, hides the header.
 */
export function nextHeaderScroll(
  state: HeaderScroll,
  y: number,
  viewportHeight: number,
): HeaderScroll {
  if (Math.abs(y - state.previous) > viewportHeight) {
    return {
      visible: false,
      anchor: y,
      previous: y,
    };
  }
  if (state.visible ? y <= state.anchor : y >= state.anchor) {
    return {
      ...state,
      anchor: y,
      previous: y,
    };
  }
  if (Math.abs(y - state.anchor) > TOLERANCE) {
    return {
      visible: !state.visible,
      anchor: y,
      previous: y,
    };
  }
  return {
    ...state,
    previous: y,
  };
}

/**
 * Keeps the header in document flow at the top. After the header leaves the viewport, it floats
 * and appears while the reader scrolls up. Navigation to a fragment hides it so that the header
 * does not cover the target. Returns a disposer that releases listeners and clears the states.
 */
export function mountDocsHeader(slot: HTMLElement): () => void {
  const view = slot.ownerDocument.defaultView!;
  const header = slot.querySelector<HTMLElement>("[data-site-header]")!;
  const abort = new view.AbortController();

  let frame: number | undefined;
  let scroll: HeaderScroll = {
    visible: false,
    anchor: view.scrollY,
    previous: view.scrollY,
  };

  const update = () => {
    frame = undefined;
    const floating = slot.getBoundingClientRect().bottom <= 0;
    scroll = floating
      ? nextHeaderScroll(scroll, view.scrollY, view.innerHeight)
      : {
          visible: false,
          anchor: view.scrollY,
          previous: view.scrollY,
        };
    setHeaderState(header, floating, !floating || scroll.visible);
  };

  const schedule = () => {
    frame ??= view.requestAnimationFrame(update);
  };

  const hide = () => {
    scroll = {
      visible: false,
      anchor: view.scrollY,
      previous: view.scrollY,
    };
    schedule();
  };

  view.addEventListener("scroll", schedule, {
    passive: true,
    signal: abort.signal,
  });
  view.addEventListener("resize", schedule, {
    passive: true,
    signal: abort.signal,
  });
  view.addEventListener("hashchange", hide, {
    signal: abort.signal,
  });

  update();

  return () => {
    abort.abort();
    if (frame !== undefined) {
      view.cancelAnimationFrame(frame);
    }
    clearHeaderState(header);
  };
}
