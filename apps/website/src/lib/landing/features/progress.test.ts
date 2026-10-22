import { expect, it } from "vite-plus/test";

import { createMotionProgress } from "./progress";

it("holds manual playback until scrolling reaches the end or the scene leaves view", () => {
  for (const start of [0, 0.5, 1]) {
    const motion = createMotionProgress();
    motion.scroll(start);
    const replay = motion.replay();
    replay(0.7);
    expect(motion.scroll(start)).toBe(0.7);
    replay(1);
    expect(motion.scroll(start)).toBe(1);
    expect(motion.scroll(0)).toBe(1);
    expect(motion.scroll(0.6)).toBe(1);
    expect(motion.manual).toBe(true);
    expect(motion.scroll(1)).toBe(1);
    expect(motion.manual).toBe(false);
    expect(replay(0.2)).toBeUndefined();
    expect(motion.scroll(0.4)).toBe(0.4);

    motion.replay()(1);
    expect(motion.scroll(0.4, false)).toBe(0.4);
    expect(motion.manual).toBe(false);
  }
});

it("ignores callbacks from replaced playback and from the previous scene", () => {
  const motion = createMotionProgress();
  const previous = motion.replay();
  const current = motion.replay();
  current(0.3);
  expect(previous(1)).toBeUndefined();
  expect(motion.scroll(0)).toBe(0.3);
  motion.reset();
  expect(current(1)).toBeUndefined();
  expect(motion.scroll(0.6)).toBe(0.6);
});
