import { fadeIn, fadeOut, first, pageCard, pageTitle, second } from "./producer";

declare const enabled: boolean;
export const selected = enabled ? first : second;
export const selectedKeyframes = enabled ? fadeIn : fadeOut;
export const selectedTransition = enabled ? pageTitle : pageCard;
