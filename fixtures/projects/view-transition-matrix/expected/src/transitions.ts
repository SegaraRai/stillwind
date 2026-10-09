import { stillwindClassKey, stillwindClsx, stillwindMerge, stillwindToFragment, stillwindViewTransitionName } from "stillwind/runtime/react";

export const pageTitle = {v:"sw-vt-page-title-56mrr"};
export const pageCard = {v:"sw-vt-9jm9s"};

export const compact = Date.now() % 2 === 0;
export const transitionKey = "fixture-product";
export const selectedTransition = compact ? pageTitle : pageCard;

// Static and conditional identities, each used with and without a key.
export const staticTransition = {className:"sw_ib2y3"};
export const staticKeyedTransition = {className:"sw_dsj5u",style:{"view-transition-name":stillwindViewTransitionName("sw-vt-page-title-56mrr",transitionKey)}};
export const staticLiteralKey = {className:"sw_49cip"};
export const conditionalTransition = {className:stillwindClsx("sw_07l45",({"sw-vt-page-title-56mrr":"sw_b9mfe","sw-vt-9jm9s":"sw_xivv8"})[(selectedTransition).v])};
export const conditionalKeyedTransition = {className:stillwindClsx("sw_eg4yn",({"sw-vt-page-title-56mrr":"sw_4qjib","sw-vt-9jm9s":"sw_yq1jw"})[(selectedTransition).v]),style:{"view-transition-name":stillwindViewTransitionName(selectedTransition,transitionKey)}};
export const conditionalLiteralKey = {className:stillwindClsx("sw_wg0m9",({"sw-vt-page-title-56mrr":"sw_w8eru","sw-vt-9jm9s":"sw_54tbn"})[(selectedTransition).v])};

// Completed props can be reused; ordinary data retains a transition and its key.
export const keyedProps = {className:"sw_zu69b"};
export const transitionData = { transition: pageCard, key: transitionKey };
export const usesTransitionData = {className:"sw_y96tt",style:{"view-transition-name":stillwindViewTransitionName("sw-vt-9jm9s",transitionData.key)}};
export const usesKeyedProps = stillwindMerge({[stillwindClassKey]:"sw_2ce4i"},stillwindToFragment((keyedProps)));

// A name another declaration states, and several transitions in one call.
export const namedExplicitly = {className:"sw_a2388"};
export const unioned = {className:"sw_gpk2a"};

// A guarded use fills a slot its own class carries, beside the unconditional classes.
export const guarded = {className:stillwindClsx("sw_dud2z",(compact) && ("sw_jpu0d"))};
