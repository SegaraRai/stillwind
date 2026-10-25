import { applyStyle, style } from "stillwind/vanilla";

interface CodePart {
  key: string;
  text: string;
  utility?: string;
  finding: boolean;
}

/**
 * Wraps single-space-separated utilities in a style() call for measured token animation.
 * Keys must identify each utility in source order. Only spaces and colons divide its text.
 * Each utility supplies a shared key for measuring its complete underline, including colons.
 * Marked keys color every segment. Throws if keys do not match utilities.
 */
export function styleParts(
  utilities: string,
  keys: readonly string[],
  marked: readonly string[] = [],
): CodePart[] {
  const words = utilities.split(" ");
  if (words.length !== keys.length || words.some((word) => !word)) {
    throw new Error("Each utility must have one stable key.");
  }
  const parts: CodePart[] = [{ key: "syntax-open", text: 'style("', finding: false }];
  words.forEach((word, index) => {
    const key = keys[index]!;
    const segments = word.split(":");
    segments.forEach((text, segment) => {
      parts.push({
        key: segment === segments.length - 1 ? key : key + "-variant-" + segment,
        text,
        utility: key,
        finding: marked.includes(key),
      });
      if (segment < segments.length - 1) {
        parts.push({
          key: key + "-colon-" + segment,
          text: ":",
          utility: key,
          finding: marked.includes(key),
        });
      }
    });
    if (index < words.length - 1) {
      parts.push({ key: key + "-space", text: " ", finding: false });
    }
  });
  parts.push({ key: "syntax-close", text: '")', finding: false });
  return parts;
}

/**
 * Maps finite progress to adjacent stops, their blend, and the nearest readable stop.
 * Progress is clamped to 0..1. Each interval holds its starting and ending states for 30% each,
 * with a smooth transition between them over the remaining 40%.
 * Throws when count is not an integer of at least two.
 */
export function morphPosition(progress: number, count: number) {
  if (!Number.isInteger(count) || count < 2) {
    throw new RangeError("A code morph requires at least two stops.");
  }
  const position = Math.min(1, Math.max(0, progress)) * (count - 1);
  const from = Math.min(count - 2, Math.floor(position));
  const transitionShare = 0.4;
  const hold = (1 - transitionShare) / 2;
  const blend = Math.min(1, Math.max(0, (position - from - hold) / transitionShare));
  return {
    from,
    to: from + 1,
    blend: blend * blend * (3 - 2 * blend),
    active: Math.round(position),
  };
}

interface MeasuredBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface MeasuredUtility extends MeasuredBox {
  finding: boolean;
}

interface MeasuredPart extends MeasuredUtility {
  text: string;
}

interface MeasuredFrame {
  parts: Map<string, MeasuredPart>;
  utilities: Map<string, MeasuredUtility>;
  height: number;
}

/**
 * Measures data-measure frames and animates whole tokens with CSS custom properties.
 * Each frame contains uniquely keyed data-part spans grouped by data-utility wrappers.
 * Marked wrappers supply data-finding. Scope contains the code canvas and diagnostic labels
 * whose data-diagnostic keys match those wrappers, with a matching SVG path for each label.
 * The host contains data-code-actors. Call render with finite progress while visible.
 * Changed tokens fade as complete segments; unchanged tokens move without splitting.
 * A data-code-punctuation span keeps its identity only when its text and measured position match.
 * A data-sort-source frame gives its utilities separate vertical paths during reordering.
 * Dispose releases resize work.
 */
export function createCodeMorph(host: HTMLElement, scope: HTMLElement) {
  const view = host.ownerDocument.defaultView;
  const layouts = [...host.querySelectorAll<HTMLElement>("[data-measure]")];
  const actors = host.querySelector<HTMLElement>("[data-code-actors]");
  const canvas = scope.querySelector<HTMLElement>("[data-code-canvas]");
  const diagnostics = [...scope.querySelectorAll<HTMLElement>("[data-diagnostics]")].map(
    (group) => ({
      labels: [...group.querySelectorAll<HTMLElement>("[data-diagnostic]")],
      lines: [...group.querySelectorAll<SVGPathElement>("[data-diagnostic-line]")],
    }),
  );
  if (!view || !actors || !canvas || layouts.length < 2) {
    throw new Error(
      "A code morph requires a window, an actor container, a canvas, and two measured frames.",
    );
  }
  let measured: MeasuredFrame[] | undefined;
  let prepared = -1;
  let lastProgress = 0;
  let frame: number | undefined;
  let disposed = false;
  const pixels = (value: number) => value.toFixed(3) + "px";

  const measure = () => {
    const origin = host.getBoundingClientRect();
    const box = (element: HTMLElement): MeasuredUtility => {
      const rect = element.getBoundingClientRect();
      return {
        x: rect.left - origin.left,
        y: rect.top - origin.top,
        width: rect.width,
        height: rect.height,
        finding: !!element.closest("[data-finding]"),
      };
    };
    measured = layouts.map((layout) => ({
      height: layout.getBoundingClientRect().height,
      parts: new Map(
        [...layout.querySelectorAll<HTMLElement>("[data-part]")].map((part) => {
          const measured = { ...box(part), text: part.textContent! };
          const key = part.hasAttribute("data-code-punctuation")
            ? "syntax-position-" +
              measured.text +
              "-" +
              pixels(measured.x) +
              "-" +
              pixels(measured.y)
            : part.dataset.part!;
          return [key, measured] as const;
        }),
      ),
      utilities: new Map(
        [...layout.querySelectorAll<HTMLElement>("[data-utility]")].map((utility) => [
          utility.dataset.utility!,
          box(utility),
        ]),
      ),
    }));
    const height = pixels(Math.max(...measured.map((layout) => layout.height)));
    host.style.setProperty("--code-height", height);
    prepared = -1;
  };

  const prepare = (index: number) => {
    const before = measured![index]!.parts;
    const after = measured![index + 1]!.parts;
    const utilities =
      view.matchMedia(style.media("inline")).matches &&
      layouts[index]!.hasAttribute("data-sort-source")
        ? [...measured![index]!.utilities.keys()]
        : [];
    const fragment = host.ownerDocument.createDocumentFragment();
    for (const key of new Set([...before.keys(), ...after.keys()])) {
      const from = before.get(key);
      const to = after.get(key);
      const color = (part: MeasuredPart | undefined) =>
        part?.finding
          ? "var(--color-danger)"
          : key.startsWith("syntax-")
            ? "var(--color-muted)"
            : "var(--color-foreground)";
      const addToken = (text: string, entering: boolean, leaving: boolean) => {
        const start = from ?? to!;
        const end = to ?? from!;
        const token = host.ownerDocument.createElement("span");
        applyStyle(token, style("code-token"));
        token.textContent = text;
        token.style.setProperty("--from-x", pixels(start.x));
        token.style.setProperty("--from-y", pixels(start.y + (entering ? 4 : 0)));
        token.style.setProperty("--to-x", pixels(end.x));
        token.style.setProperty("--to-y", pixels(end.y - (leaving ? 4 : 0)));
        const path =
          utilities.findIndex((utility) => key === utility || key.startsWith(utility + "-")) + 1;
        token.style.setProperty("--travel-y", pixels(path * start.height * 1.5));
        if (entering) {
          token.dataset.entering = "";
        } else if (leaving) {
          token.dataset.leaving = "";
        }
        token.style.setProperty("--from-color", color(from ?? to));
        token.style.setProperty("--to-color", color(to ?? from));
        fragment.append(token);
      };
      if (from?.text === to?.text) {
        addToken(from!.text, false, false);
      } else {
        if (from) {
          addToken(from.text, false, true);
        }
        if (to) {
          addToken(to.text, true, false);
        }
      }
    }
    const fromUtilities = measured![index]!.utilities;
    const toUtilities = measured![index + 1]!.utilities;
    for (const key of new Set([...fromUtilities.keys(), ...toUtilities.keys()])) {
      const from = fromUtilities.get(key);
      const to = toUtilities.get(key);
      if (from?.finding || to?.finding) {
        const start = from ?? to!;
        const end = to ?? from!;
        const line = host.ownerDocument.createElement("span");
        applyStyle(line, style("code-underline"));
        line.style.setProperty("--from-x", pixels(start.x));
        line.style.setProperty("--from-y", pixels(start.y + start.height + 4));
        line.style.setProperty("--to-x", pixels(end.x));
        line.style.setProperty("--to-y", pixels(end.y + end.height + 4));
        line.style.setProperty("--from-width", pixels(start.width));
        line.style.setProperty("--to-width", pixels(end.width));
        line.style.setProperty("--from-opacity", from?.finding ? "1" : "0");
        line.style.setProperty("--to-opacity", to?.finding ? "1" : "0");
        fragment.append(line);
      }
    }
    actors.replaceChildren(fragment);
    prepared = index;
  };

  const render = (progress: number) => {
    lastProgress = progress;
    if (!measured) {
      measure();
    }
    const position = morphPosition(progress, layouts.length);
    if (prepared !== position.from) {
      prepare(position.from);
    }
    scope.style.setProperty("--morph-progress", position.blend.toFixed(5));
    const origin = host.getBoundingClientRect();
    const bounds = canvas.getBoundingClientRect();
    const blend = (start: number, end: number) => start + (end - start) * position.blend;
    const sourceHeight = blend(measured![position.from]!.height, measured![position.to]!.height);
    scope.style.setProperty("--source-height", pixels(sourceHeight));
    for (const group of diagnostics) {
      const labels = group.labels
        .flatMap((element) => {
          const key = element.dataset.diagnostic!;
          const from = measured![position.from]!.utilities.get(key);
          const to = measured![position.to]!.utilities.get(key);
          const start = from ?? to;
          const end = to ?? from;
          const line = group.lines.find((path) => path.dataset.diagnosticLine === key);
          if (!start || !end || !line) {
            return [];
          }
          const x =
            origin.left - bounds.left + blend(start.x + start.width / 2, end.x + end.width / 2);
          const y = origin.top - bounds.top + blend(start.y + start.height, end.y + end.height) + 8;
          const width = element.offsetWidth;
          return [
            { element, line, x, y, width, left: Math.max(0, Math.min(x, bounds.width - width)) },
          ];
        })
        .sort((left, right) => left.x - right.x);
      const stacked = bounds.width < 600;
      let right = bounds.width;
      for (const label of labels.toReversed()) {
        label.left = stacked ? 0 : Math.min(label.left, right - label.width);
        right = label.left - 48;
      }
      let nextTop = origin.top - bounds.top + sourceHeight + (stacked ? 36 : 56);
      labels.forEach((label, index) => {
        const top = stacked ? nextTop : origin.top - bounds.top + sourceHeight + 56 + index * 24;
        nextTop = top + label.element.offsetHeight + 24;
        label.element.style.setProperty("--diagnostic-x", pixels(label.left));
        label.element.style.setProperty("--diagnostic-y", pixels(top));
        label.line.setAttribute(
          "d",
          stacked
            ? `M ${label.x} ${label.y} H ${bounds.width - 4 - index * 4} V ${top - 10} H ${label.left}`
            : `M ${label.x} ${label.y} V ${label.y + 18} L ${label.left} ${top - 10}`,
        );
      });
    }
    return position;
  };

  const invalidate = () => {
    measured = undefined;
    if (!disposed && frame === undefined && host.getBoundingClientRect().width) {
      frame = view.requestAnimationFrame(() => {
        frame = undefined;
        render(lastProgress);
      });
    }
  };
  const observer = new view.ResizeObserver(invalidate);
  observer.observe(host);
  view.addEventListener("resize", invalidate, { passive: true });
  host.ownerDocument.fonts.addEventListener("loadingdone", invalidate);
  return {
    render,
    dispose() {
      disposed = true;
      observer.disconnect();
      view.removeEventListener("resize", invalidate);
      host.ownerDocument.fonts.removeEventListener("loadingdone", invalidate);
      if (frame !== undefined) {
        view.cancelAnimationFrame(frame);
      }
    },
  };
}
