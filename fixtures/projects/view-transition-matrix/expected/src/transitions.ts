import { stillwindClsx, stillwindViewTransitionName, stillwindClassNameObjectClassKey, stillwindClassNameObjectMerge, stillwindClassNameObjectToFragment } from "stillwind/runtime";

export const pageTitle = {v:"sw-vt-page-title-0b9dx"};
export const pageCard = {v:"sw-vt-hl4nk"};

export const compact = Date.now() % 2 === 0;
export const transitionKey = "fixture-product";
export const selectedTransition = compact ? pageTitle : pageCard;

// Static and conditional identities, each used with and without a key.
export const staticTransition = {className:"sw_sjay2"};
export const staticKeyedTransition = {className:"sw_v7488",style:{"view-transition-name":stillwindViewTransitionName("sw-vt-page-title-0b9dx",transitionKey)}};
export const staticLiteralKey = {className:"sw_fh23y"};
export const conditionalTransition = {className:stillwindClsx("sw_ot9ws",({"sw-vt-hl4nk":"sw_oy2xo","sw-vt-page-title-0b9dx":"sw_x1ckt"})[(selectedTransition).v])};
export const conditionalKeyedTransition = {className:stillwindClsx("sw_dsmy7",({"sw-vt-hl4nk":"sw_8vx9d","sw-vt-page-title-0b9dx":"sw_hb6d1"})[(selectedTransition).v]),style:{"view-transition-name":stillwindViewTransitionName(selectedTransition,transitionKey)}};
export const conditionalLiteralKey = {className:stillwindClsx("sw_u10oq",({"sw-vt-hl4nk":"sw_5arxl","sw-vt-page-title-0b9dx":"sw_81l10"})[(selectedTransition).v])};

// Completed props can be reused; ordinary data retains a transition and its key.
export const keyedProps = {className:"sw_fdm03"};
export const transitionData = { transition: pageCard, key: transitionKey };
export const usesTransitionData = {className:"sw_ipk4z",style:{"view-transition-name":stillwindViewTransitionName("sw-vt-hl4nk",transitionData.key)}};
export const usesKeyedProps = stillwindClassNameObjectMerge({[stillwindClassNameObjectClassKey]:"sw_eqgu5"},stillwindClassNameObjectToFragment((keyedProps)));

// A name another declaration states, and several transitions in one call.
export const namedExplicitly = {className:"sw_sqk0k"};
export const unioned = {className:"sw_6azb7"};

// A guarded use fills a slot its own class carries, beside the unconditional classes.
export const guarded = {className:stillwindClsx("sw_mteqs",(compact) && ("sw_xymwq"))};
