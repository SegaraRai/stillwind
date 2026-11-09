import { stillwindClsx, stillwindViewTransitionName, stillwindClassNameObjectClassKey, stillwindClassNameObjectMerge, stillwindClassNameObjectToFragment } from "stillwind/runtime";

export const pageTitle = {v:"sw-vt-page-title-0b9dx"};
export const pageCard = {v:"sw-vt-hl4nk"};

export const compact = Date.now() % 2 === 0;
export const transitionKey = "fixture-product";
export const selectedTransition = compact ? pageTitle : pageCard;

// Static and conditional identities, each used with and without a key.
export const staticTransition = {className:"sw_1xcps"};
export const staticKeyedTransition = {className:"sw_pz6bl",style:{"view-transition-name":stillwindViewTransitionName("sw-vt-page-title-0b9dx",transitionKey)}};
export const staticLiteralKey = {className:"sw_00q1e"};
export const conditionalTransition = {className:stillwindClsx("sw_07l45",({"sw-vt-hl4nk":"sw_5jpjv","sw-vt-page-title-0b9dx":"sw_ys5a3"})[(selectedTransition).v])};
export const conditionalKeyedTransition = {className:stillwindClsx("sw_eg4yn",({"sw-vt-hl4nk":"sw_q3o0f","sw-vt-page-title-0b9dx":"sw_rdjv1"})[(selectedTransition).v]),style:{"view-transition-name":stillwindViewTransitionName(selectedTransition,transitionKey)}};
export const conditionalLiteralKey = {className:stillwindClsx("sw_wg0m9",({"sw-vt-hl4nk":"sw_4s6o8","sw-vt-page-title-0b9dx":"sw_tii40"})[(selectedTransition).v])};

// Completed props can be reused; ordinary data retains a transition and its key.
export const keyedProps = {className:"sw_15xh2"};
export const transitionData = { transition: pageCard, key: transitionKey };
export const usesTransitionData = {className:"sw_9q7tz",style:{"view-transition-name":stillwindViewTransitionName("sw-vt-hl4nk",transitionData.key)}};
export const usesKeyedProps = stillwindClassNameObjectMerge({[stillwindClassNameObjectClassKey]:"sw_2ce4i"},stillwindClassNameObjectToFragment((keyedProps)));

// A name another declaration states, and several transitions in one call.
export const namedExplicitly = {className:"sw_5hjl1"};
export const unioned = {className:"sw_shfd5"};

// A guarded use fills a slot its own class carries, beside the unconditional classes.
export const guarded = {className:stillwindClsx("sw_u0flf",(compact) && ("sw_kgrf9"))};
