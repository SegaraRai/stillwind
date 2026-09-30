import { style } from "stillwind/react";

import { selected, selectedKeyframes, selectedTransition } from "./barrel";
import { pageTitle } from "./producer";

declare const transitionKey: string;
const transitionData = { transition: selectedTransition, key: transitionKey };

export const attached = style(selected);
export const anchored = style(selected.asAnchor());
export const animated = style(selectedKeyframes("infinite"));
export const transitioned = style(selectedTransition);
export const staticKeyedTransition = style(pageTitle.for(transitionKey));
export const conditionalKeyedTransition = style(selectedTransition.for(transitionKey));
export const aliasedKeyedTransition = style(transitionData.transition.for(transitionData.key));
