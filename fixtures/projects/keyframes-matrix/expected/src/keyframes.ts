import { stillwindKeyframes } from "stillwind/react/runtime";

export const enter = stillwindKeyframes("sw-kf-enter-7nc28");
export const exit = stillwindKeyframes("sw-kf-tym79");

export const motionEnabled = Date.now() % 2 === 0;
export const selectedAnimation = motionEnabled ? enter : exit;
