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

// Static and conditional identities, each used with and without a key.
export const staticTransition = style(pageTitle);
export const staticKeyedTransition = style(pageTitle.for(transitionKey));
export const staticLiteralKey = style("block", pageTitle.for("hero"));
export const conditionalTransition = style(selectedTransition);
export const conditionalKeyedTransition = style(selectedTransition.for(transitionKey));
export const conditionalLiteralKey = style("block", selectedTransition.for("hero"));

// A key applied where the transition was declared travels in the value.
export const keyedOutside = pageTitle.for("outside");
export const keyedDynamically = pageCard.for(transitionKey);
export const usesKeyedOutside = style("block", keyedOutside);

// A name another declaration states, and several transitions in one call.
export const namedExplicitly = style("block", pageTitle, "[view-transition-name:hero]");
export const unioned = style("block", pageTitle.for("one"), pageCard.for("two"));

// A guarded use declares the complete class list for its condition.
export const guarded = style("block", compact && pageCard.for("guarded"), pageTitle.for("base"));
