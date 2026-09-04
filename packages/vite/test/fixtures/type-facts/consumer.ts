import { style } from "stillwind/react";
import { selected, selectedAnimation, selectedTransition } from "./barrel";
import { pageTitle } from "./producer";

declare const transitionKey: string;
const keyedTransitionAlias = selectedTransition.for(transitionKey);

export const attached = style(selected);
export const anchored = style(selected.anchor);
export const animated = style(selectedAnimation("infinite"));
export const transitioned = style(selectedTransition);
export const staticKeyedTransition = style(pageTitle.for(transitionKey));
export const conditionalKeyedTransition = style(selectedTransition.for(transitionKey));
export const aliasedKeyedTransition = style(keyedTransitionAlias);
