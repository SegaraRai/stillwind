import { stillwindClassKey, stillwindClsx, stillwindMerge, stillwindStyleKey, stillwindViewTransitionName, stillwindViewTransitionKey } from "stillwind/react/runtime";

export const pageTitle = {v:"--sw-v-7e58d129"};
export const pageCard = {v:"--sw-v-5ce18560"};

export const compact = Date.now() % 2 === 0;
export const transitionKey = "fixture-product";
export const selectedTransition = compact ? pageTitle : pageCard;

// Static and conditional identities, each used with and without a key.
export const staticTransition = stillwindMerge({[stillwindClassKey]:(({"--sw-v-7e58d129":"_s_2cb9eec0"})[pageTitle.v])});
export const staticKeyedTransition = stillwindMerge({[stillwindClassKey]:"_s_ba3304a0"},{[stillwindStyleKey]:{"view-transition-name":stillwindViewTransitionName("--sw-v-7e58d129",transitionKey)}});
export const staticLiteralKey = {className:"_s_27fd0933"};
export const conditionalTransition = stillwindMerge({[stillwindClassKey]:(({"--sw-v-7e58d129":"_s_538d6843","--sw-v-5ce18560":"_s_50c920c4"})[selectedTransition.v])});
export const conditionalKeyedTransition = stillwindMerge({[stillwindClassKey]:(({"--sw-v-7e58d129":"_s_a66a5d83","--sw-v-5ce18560":"_s_913700e8"})[selectedTransition.v])},{[stillwindStyleKey]:{"view-transition-name":stillwindViewTransitionName(selectedTransition,transitionKey)}});
export const conditionalLiteralKey = {className:stillwindClsx("_s_ff2da744",({"--sw-v-7e58d129":"_s_62030c47","--sw-v-5ce18560":"_s_24aea93b"})[selectedTransition.v])};

// A key applied where the transition was declared travels in the value.
export const keyedOutside = {v:"--sw-v-7e58d129",k:"outside"};
export const keyedDynamically = stillwindViewTransitionKey(pageCard,transitionKey);
export const usesKeyedOutside = stillwindMerge({[stillwindClassKey]:"_s_03bd2150"},{[stillwindStyleKey]:{"view-transition-name":stillwindViewTransitionName(keyedOutside)}});

// A name another declaration states, and several transitions in one call.
export const namedExplicitly = {className:stillwindClsx("_s_c5134c6d",({"--sw-v-7e58d129":"_s_814fc071"})[pageTitle.v])};
export const unioned = {className:"_s_b9926253"};

// A guarded use fills a slot its own class carries, beside the unconditional classes.
export const guarded = {className:stillwindClsx("_s_c8add1c2",(compact) && ("_s_a37e7f43"))};
