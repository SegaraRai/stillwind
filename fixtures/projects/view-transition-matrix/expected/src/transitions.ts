import { stillwindClassKey, stillwindMerge, stillwindStyleKey, stillwindViewTransitionName } from "stillwind/react/runtime";

export const pageTitle = {v:"--sw-v-7e58d129"};
export const pageCard = {v:"--sw-v-5ce18560"};

export const compact = Date.now() % 2 === 0;
export const transitionKey = "fixture-product";
export const selectedTransition = compact ? pageTitle : pageCard;

export const staticTransition = stillwindMerge({[stillwindClassKey]:(({"--sw-v-7e58d129":"_s_98b0e44c"})[pageTitle.v])});
export const staticKeyedTransition = stillwindMerge({[stillwindClassKey]:"_s_a088be8a"},{[stillwindStyleKey]:{"view-transition-name":stillwindViewTransitionName("--sw-v-7e58d129",transitionKey)}});
export const conditionalTransition = stillwindMerge({[stillwindClassKey]:(({"--sw-v-7e58d129":"_s_f6cdbfb9","--sw-v-5ce18560":"_s_e84d886d"})[selectedTransition.v])});
export const conditionalKeyedTransition = stillwindMerge({[stillwindClassKey]:(({"--sw-v-7e58d129":"_s_418ebbc0","--sw-v-5ce18560":"_s_eff5c445"})[selectedTransition.v])},{[stillwindStyleKey]:{"view-transition-name":stillwindViewTransitionName(selectedTransition,transitionKey)}});
