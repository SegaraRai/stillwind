export const clamp = (value: number) => Math.min(1, Math.max(0, value));

export const ease = (value: number) => value * value * (3 - 2 * value);

/**
 * Keeps manual playback until scroll progress reaches 1 or the demonstration becomes invisible.
 * scroll accepts whether the demonstration is on screen and has nonzero scene opacity.
 * Progress values are normalized to 0..1. replay returns an updater for that playback;
 * an updater returns undefined after another replay, reset, or scroll handoff supersedes it.
 * reset releases manual control and discards the previous scroll position.
 */
export function createMotionProgress() {
  let manualProgress: number | undefined;
  let lastScroll: number | undefined;
  let generation = 0;
  return {
    get manual() {
      return manualProgress !== undefined;
    },
    scroll(progress: number, visible = true) {
      progress = clamp(progress);
      if (
        manualProgress !== undefined &&
        (!visible || (lastScroll !== undefined && progress !== lastScroll && progress === 1))
      ) {
        manualProgress = undefined;
        generation++;
      }
      lastScroll = progress;
      return manualProgress ?? progress;
    },
    replay() {
      manualProgress = 0;
      const replay = ++generation;
      return (progress: number) => {
        if (replay !== generation) {
          return undefined;
        }
        return (manualProgress = clamp(progress));
      };
    },
    reset() {
      manualProgress = undefined;
      lastScroll = undefined;
      generation++;
    },
  };
}
