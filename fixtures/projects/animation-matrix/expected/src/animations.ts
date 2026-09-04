import { stillwindAnimation } from "stillwind/react/runtime";

export const enter = stillwindAnimation("--sw-a-5bc60e1e");
export const exit = stillwindAnimation("--sw-a-59b85d3d");

export const motionEnabled = Date.now() % 2 === 0;
export const selectedAnimation = motionEnabled ? enter : exit;
