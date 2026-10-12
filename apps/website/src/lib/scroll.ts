/**
 * Publishes an element's progress through the viewport as a CSS custom property.
 * Call trackScroll after layout is available, and dispose its controller when the element leaves.
 */

/** Options for one scroll range. */
export interface ScrollOptions {
  /** Property that receives progress in the inclusive range 0..1. Defaults to --scroll-progress. */
  property?: `--${string}`;
  /** Media query that enables tracking. A nonmatching query removes the published property. */
  media?: string;
  /** Receives progress after the property changes, including the first layout read. */
  onProgress?: (progress: number) => void;
  /** Receives whether tracking is enabled, including the initial media-query result. */
  onEnable?: (enabled: boolean) => void;
}

/** Controls one element's scroll range and releases its browser subscriptions. */
export interface ScrollController {
  /** Scrolls to clamped progress in 0..1. Behavior defaults to an immediate scroll. */
  scrollTo(progress: number, behavior?: ScrollBehavior): void;
  /** Stops tracking and restores the original inline custom property. Safe to call repeatedly. */
  dispose(): void;
}

/**
 * Maps the top of a range to progress while its height exceeds the viewport height.
 * A shorter range changes from 0 to 1 when its top crosses the viewport top.
 * All inputs must be finite measurements in CSS pixels from the same viewport.
 */
export function scrollProgress(top: number, height: number, viewportHeight: number): number {
  const distance = height - viewportHeight;
  if (distance <= 0) {
    return top < 0 ? 1 : 0;
  }
  return Math.min(1, Math.max(0, -top / distance));
}

/**
 * Tracks a connected element in its owner window, coalescing layout reads into animation frames.
 * An optional media query controls both publication and onEnable. Disposal removes listeners,
 * cancels the queued frame, disconnects the observer, and restores the original inline property.
 */
export function trackScroll(element: HTMLElement, options: ScrollOptions = {}): ScrollController {
  const view = element.ownerDocument.defaultView;
  if (!view) {
    throw new Error("Scroll tracking requires an element with an owner window.");
  }

  const property = options.property ?? "--scroll-progress";
  const original = element.style.getPropertyValue(property);
  const priority = element.style.getPropertyPriority(property);
  const media = options.media ? view.matchMedia(options.media) : undefined;
  let frame: number | undefined;
  let enabled = media?.matches ?? true;
  let disposed = false;

  const restore = () => {
    if (original) {
      element.style.setProperty(property, original, priority);
    } else {
      element.style.removeProperty(property);
    }
  };
  const update = () => {
    frame = undefined;
    if (!enabled || disposed) {
      return;
    }
    const rect = element.getBoundingClientRect();
    const progress = scrollProgress(rect.top, rect.height, view.innerHeight);
    element.style.setProperty(property, progress.toFixed(5));
    options.onProgress?.(progress);
  };
  const schedule = () => {
    if (enabled && !disposed && frame === undefined) {
      frame = view.requestAnimationFrame(update);
    }
  };
  const change = () => {
    enabled = media?.matches ?? true;
    if (!enabled) {
      if (frame !== undefined) {
        view.cancelAnimationFrame(frame);
        frame = undefined;
      }
      restore();
    }
    options.onEnable?.(enabled);
    schedule();
  };

  view.addEventListener("scroll", schedule, { passive: true });
  view.addEventListener("resize", schedule, { passive: true });
  media?.addEventListener("change", change);
  const observer = new view.ResizeObserver(schedule);
  observer.observe(element);
  options.onEnable?.(enabled);
  schedule();

  return {
    scrollTo(progress, behavior: ScrollBehavior = "auto") {
      const rect = element.getBoundingClientRect();
      const distance = Math.max(0, rect.height - view.innerHeight);
      view.scrollTo({
        top: view.scrollY + rect.top + Math.min(1, Math.max(0, progress)) * distance,
        behavior,
      });
    },
    dispose() {
      if (disposed) {
        return;
      }
      disposed = true;
      view.removeEventListener("scroll", schedule);
      view.removeEventListener("resize", schedule);
      media?.removeEventListener("change", change);
      observer.disconnect();
      if (frame !== undefined) {
        view.cancelAnimationFrame(frame);
      }
      restore();
    },
  };
}
