import { expect, it } from "vite-plus/test";

import { createMotionProgress } from "./progress";

it("holds manual playback through layout updates and interior scrolling after completion", () => {
  const motion = createMotionProgress();
  expect(motion.scroll(0.4)).toBe(0.4);
  const replay = motion.replay();
  replay(0.7);
  expect(motion.scroll(0.4)).toBe(0.7);
  replay(1);
  expect(motion.scroll(0.4)).toBe(1);
  expect(motion.scroll(0.6)).toBe(1);
  expect(motion.manual).toBe(true);
});

it("resumes scroll control at completion and rejects updates from interrupted playback", () => {
  const motion = createMotionProgress();
  motion.scroll(0.5);
  const replay = motion.replay();
  replay(0.7);
  expect(motion.scroll(1)).toBe(1);
  expect(motion.manual).toBe(false);
  expect(replay(1)).toBeUndefined();
  expect(motion.scroll(0.4)).toBe(0.4);
});

it("keeps visible manual playback when scrolling back to zero", () => {
  const motion = createMotionProgress();
  motion.scroll(0.5);
  motion.replay()(1);
  expect(motion.scroll(0)).toBe(1);
  expect(motion.scroll(0.5)).toBe(1);
  expect(motion.manual).toBe(true);
});

it("keeps a replay started at an endpoint until scroll progress leaves and reaches completion", () => {
  for (const endpoint of [0, 1]) {
    const motion = createMotionProgress();
    motion.scroll(endpoint);
    motion.replay()(1);
    expect(motion.scroll(endpoint)).toBe(1);
    expect(motion.manual).toBe(true);
    expect(motion.scroll(0.5)).toBe(1);
    expect(motion.scroll(1)).toBe(1);
    expect(motion.manual).toBe(false);
  }
});

it("releases manual playback when the demonstration leaves the screen or becomes fully transparent", () => {
  const motion = createMotionProgress();
  motion.scroll(0.5);
  const replay = motion.replay();
  replay(1);
  expect(motion.scroll(0.5, false)).toBe(0.5);
  expect(motion.manual).toBe(false);
  expect(replay(1)).toBeUndefined();
  expect(motion.scroll(0.4, true)).toBe(0.4);
});

it("ignores obsolete replay callbacks after a new playback or source reset", () => {
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
