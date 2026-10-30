import { expect, it } from "vite-plus/test";

import { type HeaderScroll, isFragmentLink, nextHeaderScroll } from "./header";

function scrollThrough(positions: number[], viewportHeight = 800): boolean[] {
  let state: HeaderScroll = {
    visible: false,
    anchor: positions[0]!,
    previous: positions[0]!,
  };
  return positions.slice(1).map((y) => {
    state = nextHeaderScroll(state, y, viewportHeight);
    return state.visible;
  });
}

it("shows the header while scrolling up and hides it while scrolling down", () => {
  expect(scrollThrough([1000, 1100, 1090, 1080, 1000, 1005, 1020])).toEqual([
    false,
    true,
    true,
    true,
    true,
    false,
  ]);
});

it("ignores movements against the direction up to the tolerance", () => {
  expect(scrollThrough([1000, 994, 1000])).toEqual([false, false]);
  expect(scrollThrough([1000, 900, 906, 909])).toEqual([true, true, false]);
});

it("hides the header after a jump farther than the viewport", () => {
  expect(scrollThrough([3000, 2900, 1000])).toEqual([true, false]);
});

it("recognizes links to a fragment of the current page", () => {
  const page = "https://stillwind.dev/docs/guides/style#states";
  expect(isFragmentLink("https://stillwind.dev/docs/guides/style#values", page)).toBe(true);
  expect(isFragmentLink("https://stillwind.dev/docs/guides/style#states", page)).toBe(true);
  expect(isFragmentLink("https://stillwind.dev/docs/guides/style", page)).toBe(false);
  expect(isFragmentLink("https://stillwind.dev/docs/guides/cva#values", page)).toBe(false);
  expect(isFragmentLink("https://stillwind.dev/docs/guides/style?tab=vp#values", page)).toBe(false);
  expect(isFragmentLink("https://example.com/docs/guides/style#values", page)).toBe(false);
});
