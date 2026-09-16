import { style } from "stillwind/react";

export const pageTitle = style.viewTransition({
  old: "opacity-0",
  new: "opacity-100",
});
export const pageCard = style.viewTransition({
  old: "opacity-100 motion-reduce:opacity-75",
  new: "opacity-0",
  imagePair: "isolation-auto",
});

export const compact = Date.now() % 2 === 0;
export const transitionKey = "fixture-product";
export const selectedTransition = compact ? pageTitle : pageCard;

export const staticTransition = style(pageTitle);
export const staticKeyedTransition = style(pageTitle.for(transitionKey));
export const conditionalTransition = style(selectedTransition);
export const conditionalKeyedTransition = style(selectedTransition.for(transitionKey));
