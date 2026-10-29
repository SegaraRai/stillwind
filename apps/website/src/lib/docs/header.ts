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
 * Returns whether following `href` scrolls the document at `current` to a fragment without
 * loading another page. Both arguments are absolute URLs.
 */
export function isFragmentLink(href: string, current: string): boolean {
  const target = new URL(href);
  const page = new URL(current);
  return (
    target.hash !== "" &&
    target.origin === page.origin &&
    target.pathname === page.pathname &&
    target.search === page.search
  );
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
  // Set when the page scrolls to a fragment. The next update hides the header at the position
  // after that scroll, so an upward jump shorter than the viewport does not reveal it.
  let fragmentJump = false;

  const update = () => {
    frame = undefined;
    const floating = slot.getBoundingClientRect().bottom <= 0;
    scroll =
      floating && !fragmentJump
        ? nextHeaderScroll(scroll, view.scrollY, view.innerHeight)
        : {
            visible: false,
            anchor: view.scrollY,
            previous: view.scrollY,
          };
    fragmentJump = false;
    setHeaderState(header, floating, !floating || scroll.visible);
  };

  const schedule = () => {
    frame ??= view.requestAnimationFrame(update);
  };

  const hide = () => {
    fragmentJump = true;
    schedule();
  };

  // The client router handles an in-page link with the History API, which does not fire
  // `hashchange`. The capturing listener runs before the router and the browser scroll during the
  // click, and the scheduled update runs after that scroll.
  const hideForFragmentLink = (event: MouseEvent) => {
    const link = (event.target as Element | null)?.closest?.("a[href]");
    if (
      link instanceof view.HTMLAnchorElement &&
      event.button === 0 &&
      !event.metaKey &&
      !event.ctrlKey &&
      !event.shiftKey &&
      !event.altKey &&
      (!link.target || link.target === "_self") &&
      isFragmentLink(link.href, view.location.href)
    ) {
      hide();
    }
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
  view.document.addEventListener("click", hideForFragmentLink, {
    capture: true,
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
