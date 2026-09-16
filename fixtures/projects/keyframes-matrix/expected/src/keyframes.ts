import { stillwindKeyframes } from "stillwind/react/runtime";

export const enter = stillwindKeyframes("--sw-a-59fbe1ad");
export const exit = stillwindKeyframes("--sw-a-29446666");

export const motionEnabled = Date.now() % 2 === 0;
export const selectedAnimation = motionEnabled ? enter : exit;
