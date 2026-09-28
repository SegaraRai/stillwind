import { stillwindClsx, stillwindViewTransitionName, stillwindViewTransitionKey } from "stillwind/react/runtime";

export const pageTitle = {v:"sw-vt-page-title-56mrr"};
export const pageCard = {v:"sw-vt-9jm9s"};

export const compact = Date.now() % 2 === 0;
export const transitionKey = "fixture-product";
export const selectedTransition = compact ? pageTitle : pageCard;

// Static and conditional identities, each used with and without a key.
export const staticTransition = {className:(({"sw-vt-page-title-56mrr":"sw_fqcbp"})[(pageTitle).v])};
export const staticKeyedTransition = {className:"sw_emmun",style:{"view-transition-name":stillwindViewTransitionName("sw-vt-page-title-56mrr",transitionKey)}};
export const staticLiteralKey = {className:"sw_9fn5a"};
export const conditionalTransition = {className:(({"sw-vt-page-title-56mrr":"sw_n0yg1","sw-vt-9jm9s":"sw_5fcha"})[(selectedTransition).v])};
export const conditionalKeyedTransition = {className:(({"sw-vt-page-title-56mrr":"sw_xfi1g","sw-vt-9jm9s":"sw_r7mvt"})[(selectedTransition).v]),style:{"view-transition-name":stillwindViewTransitionName(selectedTransition,transitionKey)}};
export const conditionalLiteralKey = {className:stillwindClsx("sw_wg0m9",({"sw-vt-page-title-56mrr":"sw_5gz1v","sw-vt-9jm9s":"sw_eh2pn"})[(selectedTransition).v])};

// A key applied where the transition was declared travels in the value.
export const keyedOutside = {v:"sw-vt-page-title-56mrr",w:"outside"};
export const keyedDynamically = stillwindViewTransitionKey(pageCard,transitionKey);
export const usesKeyedOutside = {className:"sw_976sw",style:{"view-transition-name":stillwindViewTransitionName(keyedOutside)}};

// A name another declaration states, and several transitions in one call.
export const namedExplicitly = {className:stillwindClsx("sw_0nemx",({"sw-vt-page-title-56mrr":"sw_m84lw"})[(pageTitle).v])};
export const unioned = {className:"sw_cplaa"};

// A guarded use fills a slot its own class carries, beside the unconditional classes.
export const guarded = {className:stillwindClsx("sw_x7k17",(compact) && ("sw_q11eg"))};
