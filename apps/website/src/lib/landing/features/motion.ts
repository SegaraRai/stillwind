import { clamp, createMotionProgress, ease } from "./progress";

/** Scrubs and replays keyframes and paired transition elements without releasing manual playback early. */
export function createMotionDemos(
  root: HTMLElement,
  options: {
    signal: AbortSignal;
    reducedMotion: MediaQueryList;
    isEnhanced: () => boolean;
    scheduleMeasure: () => void;
  },
) {
  const { signal, reducedMotion, isEnhanced, scheduleMeasure } = options;
  const view = root.ownerDocument.defaultView!;
  const entrance = root.querySelector<HTMLElement>("[data-enter-preview]")!;
  const motionStage = entrance.closest<HTMLElement>("[data-motion-stage]")!;
  const replay = root.querySelector<HTMLButtonElement>("[data-replay]")!;
  const replayTransition = root.querySelector<HTMLButtonElement>("[data-replay-transition]")!;
  const transitionStage = root.querySelector<HTMLElement>("[data-transition-stage]")!;
  const transitionLayouts = [
    ...transitionStage.querySelectorAll<HTMLElement>("[data-transition-layout]"),
  ];
  const transitionElements = [
    ...transitionStage.querySelectorAll<HTMLElement>("[data-transition-element]"),
  ];
  const entranceProgress = createMotionProgress();
  const transitionProgress = createMotionProgress();
  let replayFrame: number | undefined;
  let transitionReplayFrame: number | undefined;
  let transitionValue = 0;

  const onScreen = (element: HTMLElement) => {
    const bounds = element.getBoundingClientRect();
    return (
      bounds.bottom > 0 &&
      bounds.top < view.innerHeight &&
      bounds.right > 0 &&
      bounds.left < view.innerWidth
    );
  };
  const drawEntrance = (progress: number) => {
    if (!entranceProgress.manual) {
      for (const animation of entrance.getAnimations()) {
        animation.pause();
        animation.currentTime = progress * 420;
      }
    }
  };
  const drawTransition = (progress: number) => {
    const bounds = transitionStage.getBoundingClientRect();
    const layout = ease(clamp((progress - 0.25) / 0.75));
    const mix = (from: number, to: number) => from + (to - from) * layout;
    const participants = transitionElements.map((element) => {
      const key = element.dataset.transitionElement!;
      const start =
        key === "surface"
          ? transitionLayouts[0]!
          : transitionLayouts[0]!.querySelector<HTMLElement>(
              '[data-transition-slot="' + key + '"]',
            )!;
      const end =
        key === "surface"
          ? transitionLayouts[1]!
          : transitionLayouts[1]!.querySelector<HTMLElement>(
              '[data-transition-slot="' + key + '"]',
            )!;
      const after = end.getBoundingClientRect();
      const before = key === "detail" ? after : start.getBoundingClientRect();
      return {
        element,
        key,
        before,
        after,
        x: mix(before.left, after.left) - bounds.left,
        y: mix(before.top, after.top) - bounds.top,
        width: mix(before.width, after.width),
        height: mix(before.height, after.height),
        fontSize: mix(
          Number.parseFloat(view.getComputedStyle(start).fontSize),
          Number.parseFloat(view.getComputedStyle(end).fontSize),
        ),
      };
    });
    const thumbnail = participants.find((part) => part.key === "thumbnail")!;
    const title = participants.find((part) => part.key === "title")!;
    const meta = participants.find((part) => part.key === "meta")!;
    const detail = participants.find((part) => part.key === "detail")!;
    const surface = participants.find((part) => part.key === "surface")!;
    const titleGap = title.after.top - thumbnail.after.bottom;
    const titleBefore = title.before.top - bounds.top;
    title.y =
      titleBefore +
      (thumbnail.y + thumbnail.height + titleGap - titleBefore) * ease(clamp(progress / 0.25));
    meta.y =
      title.y +
      title.height +
      mix(meta.before.top - title.before.bottom, meta.after.top - title.after.bottom);
    detail.y = meta.y + meta.height + detail.after.top - meta.after.bottom;
    const bottomInset = surface.after.bottom - detail.after.bottom;
    surface.height = Math.max(
      surface.height,
      meta.y + meta.height + bottomInset - surface.y,
      progress > 0.82 ? detail.y + detail.height + bottomInset - surface.y : 0,
    );
    for (const part of participants) {
      part.element.style.transform = "translate(" + part.x + "px," + part.y + "px)";
      part.element.style.width = part.width + "px";
      part.element.style.height = part.height + "px";
      part.element.style.fontSize = part.fontSize + "px";
      if (part.key === "detail") {
        part.element.style.opacity = String(clamp((progress - 0.82) / 0.18));
      }
    }
    transitionStage.style.setProperty("--transition-progress", String(progress));
  };
  const showProgress = (progress: number, stage: HTMLElement) => {
    (isEnhanced()
      ? root
      : stage.closest<HTMLElement>("[data-feature-preview-frame]")!
    ).style.setProperty("--demo-progress", String(progress));
  };

  replay.addEventListener(
    "click",
    () => {
      if (reducedMotion.matches) {
        return;
      }
      if (replayFrame !== undefined) {
        view.cancelAnimationFrame(replayFrame);
      }
      const update = entranceProgress.replay();
      const animations = entrance.getAnimations();
      animations.forEach((animation) => {
        animation.currentTime = 0;
        animation.play();
      });
      const drawReplay = () => {
        replayFrame = undefined;
        if (signal.aborted) {
          return;
        }
        const progress = update(Number(animations[0]?.currentTime ?? 420) / 420);
        if (progress === undefined) {
          return;
        }
        showProgress(progress, motionStage);
        scheduleMeasure();
        if (progress < 1) {
          replayFrame = view.requestAnimationFrame(drawReplay);
        }
      };
      drawReplay();
    },
    { signal },
  );
  replayTransition.addEventListener(
    "click",
    () => {
      if (reducedMotion.matches) {
        return;
      }
      if (transitionReplayFrame !== undefined) {
        view.cancelAnimationFrame(transitionReplayFrame);
      }
      const update = transitionProgress.replay();
      const started = view.performance.now();
      const drawReplay = (time: number) => {
        transitionReplayFrame = undefined;
        if (signal.aborted) {
          return;
        }
        const progress = update((time - started) / 300);
        if (progress === undefined) {
          return;
        }
        transitionValue = progress;
        showProgress(progress, transitionStage);
        drawTransition(progress);
        scheduleMeasure();
        if (progress < 1) {
          transitionReplayFrame = view.requestAnimationFrame(drawReplay);
        }
      };
      drawReplay(started);
    },
    { signal },
  );

  return {
    refreshControls() {
      replay.hidden = reducedMotion.matches;
      replayTransition.hidden = reducedMotion.matches;
    },
    render(frame: number, scrollProgress: number, visible: boolean) {
      if (frame === 0) {
        const progress = entranceProgress.scroll(scrollProgress, visible && onScreen(motionStage));
        drawEntrance(progress);
        return progress;
      }
      transitionValue = transitionProgress.scroll(
        scrollProgress,
        visible && onScreen(transitionStage),
      );
      return transitionValue;
    },
    scrub(progressFor: (element: HTMLElement) => number) {
      const progress = entranceProgress.scroll(progressFor(motionStage), onScreen(motionStage));
      entrance
        .closest<HTMLElement>("[data-feature-preview-frame]")!
        .style.setProperty("--demo-progress", String(progress));
      drawEntrance(progress);
      transitionValue = transitionProgress.scroll(
        progressFor(transitionStage),
        onScreen(transitionStage),
      );
      transitionStage
        .closest<HTMLElement>("[data-feature-preview-frame]")!
        .style.setProperty("--demo-progress", String(transitionValue));
      drawTransition(transitionValue);
    },
    measure(frame?: number) {
      if (frame === 1) {
        drawTransition(transitionValue);
      }
    },
    reset() {
      entranceProgress.reset();
      transitionProgress.reset();
      entrance.getAnimations().forEach((animation) => animation.pause());
      if (replayFrame !== undefined) {
        view.cancelAnimationFrame(replayFrame);
        replayFrame = undefined;
      }
      if (transitionReplayFrame !== undefined) {
        view.cancelAnimationFrame(transitionReplayFrame);
        transitionReplayFrame = undefined;
      }
    },
    dispose() {
      replay.hidden = true;
      replayTransition.hidden = true;
      transitionElements.forEach((element) => element.removeAttribute("style"));
      entrance.getAnimations().forEach((animation) => animation.finish());
    },
  };
}
