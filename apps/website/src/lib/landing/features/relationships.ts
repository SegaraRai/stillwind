import { clamp, ease } from "./progress";

const containerWidths = {
  wide: 300,
  narrow: 180,
};

/** Controls container, scoped-ref, and anchor demos, including their manual overrides. */
export function createRelationshipDemos(
  root: HTMLElement,
  signal: AbortSignal,
  scheduleMeasure: () => void,
) {
  const container = root.querySelector<HTMLElement>("[data-responsive-container]")!;
  const containerPreview = container.closest<HTMLElement>("[data-feature-preview-frame]")!;
  const size = root.querySelector<HTMLElement>("[data-container-size]")!;
  const measure = root.querySelector<HTMLElement>("[data-container-measure]")!;
  const resize = root.querySelector<HTMLButtonElement>("[data-resize-demo]")!;
  const scopeButtons = [...root.querySelectorAll<HTMLButtonElement>("[data-scope-button]")];
  const anchorDemo = root.querySelector<HTMLElement>("[data-anchor-demo]")!;
  const anchorButton = root.querySelector<HTMLElement>("[data-anchor-button]")!;
  const move = root.querySelector<HTMLButtonElement>("[data-move-anchor]")!;
  let manualWidth: number | undefined;
  let manualAnchor: number | undefined;
  let manualScope = false;
  let anchorProgress = 0;

  const wideContainerWidth = () =>
    Math.min(
      containerWidths.wide,
      Math.floor(containerPreview.getBoundingClientRect().width / 10) * 10,
    );
  const drawContainer = (progress: number) => {
    container.style.setProperty(
      "--container-width",
      String(
        manualWidth ??
          wideContainerWidth() - ease(progress) * (wideContainerWidth() - containerWidths.narrow),
      ) + "px",
    );
  };
  const drawScope = (progress: number) => {
    if (!manualScope) {
      scopeButtons[0]!.setAttribute("aria-pressed", String(progress >= 0.25 && progress < 0.7));
      scopeButtons[1]!.setAttribute("aria-pressed", String(progress >= 0.7));
    }
  };
  const drawAnchor = (progress = anchorProgress) => {
    if (!anchorDemo.clientWidth) {
      return;
    }
    anchorProgress = progress;
    const insetX = anchorButton.offsetWidth / 2 + 12;
    const insetY = anchorButton.offsetHeight / 2 + 12;
    const right = anchorDemo.clientWidth - insetX;
    const bottom = anchorDemo.clientHeight - insetY;
    const points = [
      [anchorDemo.clientWidth / 2, 58],
      [insetX, 58],
      [insetX, bottom],
      [right, bottom],
      [right, insetY],
      [anchorDemo.clientWidth / 2, 58],
    ] as const;
    const phase = manualAnchor ?? progress * (points.length - 1);
    const from = Math.min(points.length - 2, Math.floor(phase));
    const blend = ease(clamp((phase - from - 0.2) / 0.6));
    const start = points[from]!;
    const end = points[from + 1]!;
    anchorDemo.style.setProperty("--anchor-x", start[0] + (end[0] - start[0]) * blend + "px");
    anchorDemo.style.setProperty("--anchor-y", start[1] + (end[1] - start[1]) * blend + "px");
  };

  resize.hidden = false;
  move.hidden = false;
  scopeButtons.forEach((button) => {
    button.disabled = false;
    button.addEventListener(
      "click",
      () => {
        manualScope = true;
        button.setAttribute("aria-pressed", String(button.getAttribute("aria-pressed") !== "true"));
      },
      { signal },
    );
  });
  resize.addEventListener(
    "click",
    () => {
      manualWidth =
        container.getBoundingClientRect().width >
        (wideContainerWidth() + containerWidths.narrow) / 2
          ? containerWidths.narrow
          : wideContainerWidth();
      container.style.setProperty("--container-width", String(manualWidth) + "px");
      scheduleMeasure();
    },
    { signal },
  );
  move.addEventListener(
    "click",
    () => {
      manualAnchor = (Math.floor(manualAnchor ?? anchorProgress * 5) + 1) % 6;
      drawAnchor();
      scheduleMeasure();
    },
    { signal },
  );

  return {
    render(frame: number, progress: number) {
      if (frame === 0) {
        drawContainer(progress);
      } else if (frame === 1) {
        drawScope(progress);
      } else {
        anchorProgress = progress;
      }
    },
    scrub(progressFor: (element: HTMLElement) => number) {
      drawContainer(progressFor(container));
      drawScope(progressFor(scopeButtons[0]!));
      drawAnchor(progressFor(anchorDemo));
    },
    measureSize() {
      const width = container.getBoundingClientRect().width;
      if (container.clientWidth) {
        size.textContent = Math.round(width) + "px";
        measure.style.width = width + "px";
      }
    },
    measure(frame?: number) {
      if (frame === 2) {
        drawAnchor();
      }
    },
    reset() {
      manualWidth = undefined;
      manualAnchor = undefined;
      manualScope = false;
    },
    dispose() {
      container.style.removeProperty("--container-width");
      anchorDemo.style.removeProperty("--anchor-x");
      anchorDemo.style.removeProperty("--anchor-y");
      scopeButtons.forEach((button) => {
        button.disabled = true;
      });
      resize.hidden = true;
      move.hidden = true;
    },
  };
}
