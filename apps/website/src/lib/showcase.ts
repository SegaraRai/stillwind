import { style } from "stillwind/astro";

import { createCodeMorph, morphPosition } from "./code-morph";
import { trackScroll } from "./scroll";

/** Scene indices for six complete states, including the three stages of Normalize & Format. */
export const showcaseScenes = [0, 1, 2, 2, 2, 3] as const;

/** Scroll distance in viewport heights. */
export const showcaseScrollLength = (showcaseScenes.length - 1) * 0.8;

/**
 * Maps finite scroll progress to six states, four scene opacities, and their source labels.
 * Normalization and sorting keep the combined scene's heading visible throughout both morphs.
 * Refinement, sorting, and compilation receive separate progress values for their presentation.
 */
export function showcasePosition(progress: number) {
  const position = morphPosition(progress, showcaseScenes.length);
  const stateOpacity = (state: number) =>
    state === position.from
      ? Math.max(0, 1 - position.blend * 2)
      : state === position.to
        ? Math.max(0, position.blend * 2 - 1)
        : 0;
  const fromScene = showcaseScenes[position.from];
  const toScene = showcaseScenes[position.to];
  const opacity = Array.from({ length: 4 }, (_, scene) => {
    if (fromScene === toScene) {
      return Number(scene === fromScene);
    }
    return scene === fromScene
      ? stateOpacity(position.from)
      : scene === toScene
        ? stateOpacity(position.to)
        : 0;
  });
  const reveal = (state: number) =>
    position.from < state ? 0 : position.from === state ? position.blend : 1;
  return {
    ...position,
    scene: showcaseScenes[position.active],
    opacity,
    labels: [2, 3, 4, 5].map(stateOpacity),
    diagnostic: stateOpacity(2),
    refine: reveal(1),
    sort: reveal(3),
    compile: reveal(4),
  };
}

/**
 * Connects four showcase scenes and their six source states through one scroll range.
 * The root must supply four matching heading, detail, stop, and navigation elements.
 * Its live demo contains one primary expression, four source labels, and six measured sources.
 * The returned function restores the static presentation and releases browser subscriptions.
 */
export function mountShowcase(root: HTMLElement): () => void {
  const view = root.ownerDocument.defaultView!;
  const stops = [...root.querySelectorAll<HTMLElement>("[data-stop]")];
  const steps = [...root.querySelectorAll<HTMLAnchorElement>("[data-step]")];
  const headings = [...root.querySelectorAll<HTMLElement>("[data-heading]")];
  const details = [...root.querySelectorAll<HTMLElement>("[data-detail]")];
  const demo = root.querySelector<HTMLElement>("[data-live-demo]")!;
  const description = root.querySelector<HTMLElement>("[data-live-description]")!;
  const readable = root.querySelector<HTMLElement>("[data-readable-source]")!;
  const labels = [...root.querySelectorAll<HTMLElement>("[data-source-label]")];
  const diagnostics = root.querySelector<HTMLElement>("[data-format-diagnostics]")!;
  const commands = [...root.querySelectorAll<HTMLElement>("[data-format-command] span")];
  const sources = [...root.querySelectorAll<HTMLElement>("[data-measure]")];
  const morph = createCodeMorph(root.querySelector<HTMLElement>("[data-code-host]")!, demo);
  let enhanced = false;
  let active = -1;

  const setActive = (state: number) => {
    if (active === state) {
      return;
    }
    active = state;
    const index = showcaseScenes[state]!;
    readable.textContent = sources[state]!.dataset.source!;
    [...headings, ...details].forEach((element, offset) => {
      const selected = offset % stops.length === index;
      element.inert = !selected;
      element.setAttribute("aria-hidden", String(!selected));
      element.toggleAttribute("data-active", selected);
    });
    diagnostics.inert = state !== 2;
    diagnostics.setAttribute("aria-hidden", String(state !== 2));
    commands.forEach((command, index) => {
      const selected = index === 0 ? state === 2 || state === 3 : state === 4;
      command.setAttribute("aria-hidden", String(!selected));
    });
    steps.forEach((step, stepIndex) => {
      if (enhanced && stepIndex === index) {
        step.setAttribute("aria-current", "step");
      } else {
        step.removeAttribute("aria-current");
      }
    });
  };
  const controller = trackScroll(root, {
    media: style.media(
      "inline:pin-story:motion-safe",
      "demo:max-inline:pin-story-compact:motion-safe",
    ),
    onEnable(enabled) {
      enhanced = enabled;
      root.toggleAttribute("data-enhanced", enabled);
      demo.hidden = !enabled;
      description.hidden = !enabled;
      stops.forEach((stop) => {
        stop.inert = enabled;
        if (enabled) {
          stop.setAttribute("aria-hidden", "true");
        } else {
          stop.removeAttribute("aria-hidden");
        }
      });
      active = -1;
      setActive(0);
    },
    onProgress(progress) {
      const position = showcasePosition(progress);
      demo.style.setProperty("--refine-progress", position.refine.toFixed(5));
      demo.style.setProperty("--sort-progress", position.sort.toFixed(5));
      demo.style.setProperty("--compile-progress", position.compile.toFixed(5));
      morph.render(progress);
      [...headings, ...details].forEach((element, offset) => {
        const index = offset % stops.length;
        element.style.setProperty("--detail-opacity", position.opacity[index]!.toFixed(5));
      });
      labels.forEach((element, index) => {
        element.style.setProperty("--label-opacity", position.labels[index]!.toFixed(5));
      });
      diagnostics.style.setProperty("--diagnostic-opacity", position.diagnostic.toFixed(5));
      setActive(position.active);
    },
  });

  const restoreHash = () => {
    if (enhanced) {
      const index = stops.findIndex((stop) => `#${stop.id}` === view.location.hash);
      if (index >= 0) {
        controller.scrollTo(
          showcaseScenes.findIndex((scene) => scene === index) / (showcaseScenes.length - 1),
        );
      }
    }
  };
  view.addEventListener("hashchange", restoreHash);
  if (root.ownerDocument.readyState === "complete") {
    restoreHash();
  } else {
    view.addEventListener("load", restoreHash, { once: true });
  }
  const navigate = (event: MouseEvent) => {
    if (
      !enhanced ||
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey ||
      event.altKey ||
      event.button !== 0
    ) {
      return;
    }
    const step = (event.target as Element).closest<HTMLAnchorElement>("[data-step]");
    if (!step) {
      return;
    }
    event.preventDefault();
    controller.scrollTo(
      showcaseScenes.findIndex((scene) => scene === steps.indexOf(step)) /
        (showcaseScenes.length - 1),
      "smooth",
    );
    view.history.replaceState(null, "", step.hash);
  };
  root.addEventListener("click", navigate);
  return () => {
    controller.dispose();
    morph.dispose();
    root.removeEventListener("click", navigate);
    view.removeEventListener("hashchange", restoreHash);
    view.removeEventListener("load", restoreHash);
    root.removeAttribute("data-enhanced");
    demo.hidden = true;
    description.hidden = true;
    stops.forEach((stop) => {
      stop.inert = false;
      stop.removeAttribute("aria-hidden");
    });
    enhanced = false;
    active = -1;
    setActive(0);
  };
}
