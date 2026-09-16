import { style } from "stillwind/react";

export const enter = style.keyframes({
  from: "opacity-0 translate-y-2 scale-98",
  to: "opacity-100 translate-y-0 scale-100",
});
export const exit = style.keyframes({
  from: "opacity-100",
  to: "opacity-0 scale-98",
});

export const motionEnabled = Date.now() % 2 === 0;
export const selectedAnimation = motionEnabled ? enter : exit;
