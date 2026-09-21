import { stillwindClassKey, stillwindClsx, stillwindMerge, stillwindStyleKey, stillwindViewTransitionName, stillwindViewTransitionKey } from "stillwind/react/runtime";

export const pageTitle = {v:"sw-vt-page-title-vtl83"};
export const pageCard = {v:"sw-vt-woqm1"};

export const compact = Date.now() % 2 === 0;
export const transitionKey = "fixture-product";
export const selectedTransition = compact ? pageTitle : pageCard;

// Static and conditional identities, each used with and without a key.
export const staticTransition = stillwindMerge({[stillwindClassKey]:(({"sw-vt-page-title-vtl83":"sw_1ucjt"})[(pageTitle).v])});
export const staticKeyedTransition = stillwindMerge({[stillwindClassKey]:"sw_rylqx"},{[stillwindStyleKey]:{"view-transition-name":stillwindViewTransitionName("sw-vt-page-title-vtl83",transitionKey)}});
export const staticLiteralKey = {className:"sw_2qlao"};
export const conditionalTransition = stillwindMerge({[stillwindClassKey]:(({"sw-vt-page-title-vtl83":"sw_itpxj","sw-vt-woqm1":"sw_7gx3z"})[(selectedTransition).v])});
export const conditionalKeyedTransition = stillwindMerge({[stillwindClassKey]:(({"sw-vt-page-title-vtl83":"sw_gfnst","sw-vt-woqm1":"sw_qsx45"})[(selectedTransition).v])},{[stillwindStyleKey]:{"view-transition-name":stillwindViewTransitionName(selectedTransition,transitionKey)}});
export const conditionalLiteralKey = {className:stillwindClsx("sw_43hgg",({"sw-vt-page-title-vtl83":"sw_3oyfk","sw-vt-woqm1":"sw_556g5"})[(selectedTransition).v])};

// A key applied where the transition was declared travels in the value.
export const keyedOutside = {v:"sw-vt-page-title-vtl83",w:"outside"};
export const keyedDynamically = stillwindViewTransitionKey(pageCard,transitionKey);
export const usesKeyedOutside = stillwindMerge({[stillwindClassKey]:"sw_8pa1h"},{[stillwindStyleKey]:{"view-transition-name":stillwindViewTransitionName(keyedOutside)}});

// A name another declaration states, and several transitions in one call.
export const namedExplicitly = {className:stillwindClsx("sw_0j70v",({"sw-vt-page-title-vtl83":"sw_uieoj"})[(pageTitle).v])};
export const unioned = {className:"sw_j0jnc"};

// A guarded use fills a slot its own class carries, beside the unconditional classes.
export const guarded = {className:stillwindClsx("sw_mo7p2",(compact) && ("sw_wiq0e"))};
