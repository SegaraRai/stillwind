/**
 * Draws the landing backdrop: streamlines over a tilted lattice, fixed behind the whole page.
 * The wind calms while the hero leaves the viewport. The backdrop then dims and fades out of the
 * content column, and the streamlines straighten onto lattice rows by the end of the page.
 * Call mountBackdrop once per page; backdropState and streamlinePath compute the drawing from measurements.
 */

/**
 * Number of streamlines. The markup renders one base path for each index.
 */
export const STREAMLINE_COUNT = 15;

/**
 * Values that one scroll position assigns to the backdrop. Every value is in 0..1.
 */
export interface BackdropState {
  /** Playback rate of the gust animations. */
  wind: number;
  /** Opacity of the gust paths. */
  gustOpacity: number;
  /** Opacity of the base streamlines. */
  baseOpacity: number;
  /** Opacity of the whole backdrop. */
  opacity: number;
  /** Mask alpha over the content column. The gutters always keep alpha 1. */
  columnAlpha: number;
  /** 0 draws the original curves, and 1 draws straight lines on lattice rows. */
  straightness: number;
}

const clamp01 = (value: number) => Math.min(1, Math.max(0, value || 0));

/**
 * Returns the backdrop state for a scroll position.
 * heroEnd is the distance from the document top to the hero bottom, and scrollRange is the maximum
 * scroll position. All inputs are CSS pixels; a zero range counts as no scroll progress.
 */
export function backdropState(
  scrollTop: number,
  heroEnd: number,
  scrollRange: number,
): BackdropState {
  const calm = clamp01(scrollTop / (heroEnd * 0.8));
  const dim = clamp01((scrollTop - heroEnd * 0.6) / (heroEnd * 0.6));
  const page = clamp01(scrollTop / scrollRange);
  return {
    wind: 1 - calm,
    gustOpacity: 1 - clamp01((calm - 0.2) / 0.6),
    baseOpacity: 1 - 0.3 * calm,
    opacity: 1 - 0.5 * dim,
    columnAlpha: 1 - 0.85 * dim,
    straightness: 1 - (1 - page) ** 2,
  };
}

/**
 * Measurements of the backdrop in CSS pixels, and the lattice that the streamlines straighten onto.
 */
export interface StreamlineFrame {
  width: number;
  height: number;
  /** Distance from the document top to the hero bottom. The curves fit this height. */
  heroEnd: number;
  /** Distance between lattice rows. */
  spacing: number;
  /** Lattice row angle in radians. A negative angle makes rows rise to the right. */
  tilt: number;
  /** 0 draws the original curve, and 1 draws a straight line on a lattice row. */
  straightness: number;
}

/**
 * Returns the SVG path data of streamline index in 0..STREAMLINE_COUNT-1.
 * The curves spread over the hero on the left and converge into a band toward the lower right.
 * Each straight line lies on its own lattice row. The lattice is a layer twice the backdrop size,
 * rotated around the backdrop center, with 1px rows at multiples of spacing from its top edge.
 */
export function streamlinePath(index: number, frame: StreamlineFrame): string {
  const { width, height, spacing, tilt, straightness } = frame;
  const t = index / (STREAMLINE_COUNT - 1);
  const sx = width / 1440;
  const sy = Math.min(height, frame.heroEnd) / 820;
  const cx = width / 2;
  const cy = height / 2;
  // The layer top lies height above the center, and a row line covers 1px starting 1px above its multiple.
  const row = Math.round(height / spacing) + index - Math.floor(STREAMLINE_COUNT / 2);
  const offset = row * spacing - 0.5 - height;
  const rowY = (x: number) => cy + (offset + Math.sin(tilt) * (x - cx)) / Math.cos(tilt);
  const points = [
    [-80 * sx, (40 + t * 760) * sy],
    [420 * sx, (70 + t * 760 - Math.sin(t * Math.PI) * 90) * sy],
    [820 * sx, (280 + t * 240) * sy],
    [width + 80 * sx, (500 + t * 120) * sy],
  ].map(([x, y]) => `${x} ${y! + (rowY(x!) - y!) * straightness}`);
  return `M ${points[0]} C ${points[1]}, ${points[2]}, ${points[3]}`;
}

/**
 * Starts drawing a connected backdrop element and returns a function that stops it.
 * The backdrop must contain an svg, the lattice element marked data-backdrop-lattice, and paths marked
 * data-streamline with their index. Gust paths share the index of their base path.
 * landing.css defines the lattice spacing and the --backdrop-tilt angle; this function reads both.
 * The function sets the --backdrop-* custom properties on the backdrop and the playback rate of its animations.
 */
export function mountBackdrop(backdrop: HTMLElement, hero: HTMLElement): () => void {
  const document = backdrop.ownerDocument;
  const view = document.defaultView!;
  const svg = backdrop.querySelector("svg")!;
  const lattice = backdrop.querySelector<HTMLElement>("[data-backdrop-lattice]")!;
  const paths = [...backdrop.querySelectorAll<SVGPathElement>("path[data-streamline]")];
  const abort = new view.AbortController();

  let frame: number | undefined;

  const update = () => {
    frame = undefined;
    const width = svg.clientWidth;
    const height = svg.clientHeight;
    const heroEnd = hero.getBoundingClientRect().bottom + view.scrollY;
    const scrollRange = document.documentElement.scrollHeight - view.innerHeight;
    const state = backdropState(view.scrollY, heroEnd, scrollRange);
    const latticeStyle = view.getComputedStyle(lattice);
    const shape: StreamlineFrame = {
      width,
      height,
      heroEnd,
      spacing: Number.parseFloat(latticeStyle.backgroundSize),
      tilt: (Number.parseFloat(latticeStyle.getPropertyValue("--backdrop-tilt")) * Math.PI) / 180,
      straightness: state.straightness,
    };

    svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
    for (const path of paths) {
      path.setAttribute("d", streamlinePath(Number(path.dataset.streamline), shape));
    }
    for (const animation of backdrop.getAnimations({ subtree: true })) {
      animation.playbackRate = state.wind;
    }
    backdrop.style.setProperty("--backdrop-gust-opacity", String(state.gustOpacity));
    backdrop.style.setProperty("--backdrop-base-opacity", String(state.baseOpacity));
    backdrop.style.setProperty("--backdrop-opacity", String(state.opacity));
    backdrop.style.setProperty("--backdrop-column-alpha", String(state.columnAlpha));
  };

  const schedule = () => {
    frame ??= view.requestAnimationFrame(update);
  };

  view.addEventListener("scroll", schedule, { passive: true, signal: abort.signal });
  view.addEventListener("resize", schedule, { passive: true, signal: abort.signal });
  document.fonts.addEventListener("loadingdone", schedule, { signal: abort.signal });

  const observer = new view.ResizeObserver(schedule);
  observer.observe(document.body);

  update();

  return () => {
    abort.abort();
    observer.disconnect();
    if (frame !== undefined) {
      view.cancelAnimationFrame(frame);
    }
  };
}
