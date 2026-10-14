import { featureTopics } from "./features/examples";
import { createMotionDemos } from "./features/motion";
import { createFeaturePresentation } from "./features/presentation";
import { clamp } from "./features/progress";
import { createRelationshipDemos } from "./features/relationships";
import {
  featureOffsets,
  featurePosition,
  featureScenes,
  featureScrollLength,
} from "./features/scenes";
import { trackScroll } from "./scroll";

/**
 * Enhances paired source and UI examples with scroll navigation and motion.
 * Tall viewports pin the active example. Short viewports scrub each preview in document flow.
 * Disposal releases observers and listeners and restores the sequential presentation.
 */
export function mountFeatures(root: HTMLElement): () => void {
  const view = root.ownerDocument.defaultView!;
  const steps = [...root.querySelectorAll<HTMLAnchorElement>("[data-feature-step]")];
  const cues = [...root.querySelectorAll<HTMLButtonElement>("[data-feature-cue]")];
  const abort = new view.AbortController();
  const signal = abort.signal;
  const reducedMotion = view.matchMedia("(prefers-reduced-motion: reduce)");
  const compact = view.matchMedia("(width < 1120px)");
  let enhanced = false;
  let active = -1;
  let activeFrame = -1;
  let measureFrame: number | undefined;
  let focusOpacity = 1;

  const measure = () => {
    measureFrame = undefined;
    if (!enhanced && !reducedMotion.matches) {
      const progressFor = (element: HTMLElement) =>
        clamp(
          (view.innerHeight * 0.85 - element.getBoundingClientRect().top) /
            (view.innerHeight * 0.6),
        );
      relationships.scrub(progressFor);
      motion.scrub(progressFor);
    }
    relationships.measureSize();
    if (enhanced) {
      presentation.measureSizes(compact.matches);
      relationships.measure(active === 1 ? activeFrame : undefined);
      motion.measure(active === 2 ? activeFrame : undefined);
      presentation.measure(active, activeFrame, focusOpacity);
    }
  };
  const scheduleMeasure = () => {
    if (measureFrame === undefined) {
      measureFrame = view.requestAnimationFrame(measure);
    }
  };
  const presentation = createFeaturePresentation(root, signal);
  const relationships = createRelationshipDemos(root, signal, scheduleMeasure);
  const motion = createMotionDemos(root, {
    signal,
    reducedMotion,
    isEnhanced: () => enhanced,
    scheduleMeasure,
  });
  const setActive = (scene: number, frame: number) => {
    if (scene !== active || frame !== activeFrame) {
      relationships.reset();
      motion.reset();
      active = scene;
      activeFrame = frame;
    }
    presentation.setActive(scene, frame, enhanced);
  };
  const controller = trackScroll(root, {
    media:
      "(min-width: 1120px) and (min-height: 720px) and (prefers-reduced-motion: no-preference), (840px <= width < 1120px) and (min-height: 740px) and (prefers-reduced-motion: no-preference), (640px <= width < 840px) and (min-height: 900px) and (prefers-reduced-motion: no-preference), (320px <= width < 640px) and (min-height: 800px) and (prefers-reduced-motion: no-preference)",
    onEnable(enabled) {
      enhanced = enabled;
      root.toggleAttribute("data-enhanced", enabled);
      root.toggleAttribute("data-motion-enabled", !reducedMotion.matches);
      root.toggleAttribute("data-compact", compact.matches);
      motion.refreshControls();
      if (!enabled) {
        root.style.removeProperty("--demo-progress");
      }
      presentation.clearProgress();
      setActive(0, 0);
      scheduleMeasure();
    },
    onProgress(progress) {
      const current = featurePosition(progress);
      setActive(current.scene, current.frame);
      const scrollProgress = clamp((current.local - 0.18) / 0.64);
      const demoProgress =
        current.scene === 2
          ? motion.render(current.frame, scrollProgress, current.opacity[2]! > 0)
          : scrollProgress;
      focusOpacity = current.focusOpacity;
      root.style.setProperty("--demo-progress", demoProgress.toFixed(5));
      presentation.render(current, compact.matches);
      if (current.scene === 1) {
        relationships.render(current.frame, demoProgress);
      }
      scheduleMeasure();
    },
  });

  const restoreHash = () => {
    if (signal.aborted) {
      return;
    }
    const hash = view.location.hash === "#states" ? "#syntax" : view.location.hash;
    const scene = featureTopics.findIndex((topic) => "#" + topic.id === hash);
    if (enhanced && scene >= 0) {
      controller.scrollTo(featureScenes[featureOffsets[scene]!]!.start / featureScrollLength);
    }
  };
  view.addEventListener("hashchange", restoreHash, { signal });
  if (root.ownerDocument.readyState === "complete") {
    view.requestAnimationFrame(restoreHash);
  } else {
    view.addEventListener("load", () => view.requestAnimationFrame(restoreHash), {
      once: true,
      signal,
    });
  }
  root.addEventListener(
    "click",
    (event) => {
      if (
        !enhanced ||
        event.ctrlKey ||
        event.metaKey ||
        event.shiftKey ||
        event.altKey ||
        event.button
      ) {
        return;
      }
      const target = event.target as Element;
      const step = target.closest<HTMLAnchorElement>("[data-feature-step]");
      const cue = target.closest<HTMLButtonElement>("[data-feature-cue]");
      const direction = target.closest<HTMLButtonElement>("[data-feature-direction]");
      if (step) {
        event.preventDefault();
        controller.scrollTo(
          featureScenes[featureOffsets[steps.indexOf(step)]!]!.start / featureScrollLength,
          "smooth",
        );
        view.history.replaceState(null, "", step.hash);
      } else if (cue) {
        controller.scrollTo(
          featureScenes[cues.indexOf(cue)]!.start / featureScrollLength,
          "smooth",
        );
      } else if (direction) {
        const next =
          featureOffsets[active]! + activeFrame + Number(direction.dataset.featureDirection);
        controller.scrollTo(featureScenes[next]!.start / featureScrollLength, "smooth");
      }
    },
    { signal },
  );
  const observer = new view.ResizeObserver(scheduleMeasure);
  root
    .querySelectorAll<HTMLElement>("[data-feature-panel], [data-responsive-container]")
    .forEach((element) => observer.observe(element));
  view.addEventListener(
    "scroll",
    () => {
      if (!enhanced) {
        scheduleMeasure();
      }
    },
    { passive: true, signal },
  );
  view.addEventListener("resize", scheduleMeasure, { passive: true, signal });
  root.ownerDocument.fonts.addEventListener("loadingdone", scheduleMeasure, { signal });
  compact.addEventListener(
    "change",
    () => {
      root.toggleAttribute("data-compact", compact.matches);
      scheduleMeasure();
    },
    { signal },
  );

  return () => {
    abort.abort();
    controller.dispose();
    observer.disconnect();
    if (measureFrame !== undefined) {
      view.cancelAnimationFrame(measureFrame);
    }
    enhanced = false;
    active = -1;
    activeFrame = -1;
    root.removeAttribute("data-enhanced");
    root.removeAttribute("data-motion-enabled");
    root.removeAttribute("data-compact");
    setActive(0, 0);
    presentation.dispose();
    relationships.dispose();
    motion.dispose();
    root.style.removeProperty("--demo-progress");
    presentation.clearProgress();
  };
}
