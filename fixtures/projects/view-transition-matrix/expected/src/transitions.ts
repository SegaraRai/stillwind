import { stillwindClassKey, stillwindMerge, stillwindStyleKey, stillwindViewTransition, stillwindViewTransitionStyle } from "stillwind/react/runtime";

export const pageTitle = stillwindViewTransition("--sw-v-7e58d129");
export const pageCard = stillwindViewTransition("--sw-v-5ce18560");

export const compact = Date.now() % 2 === 0;
export const transitionKey = "fixture-product";
export const selectedTransition = compact ? pageTitle : pageCard;

export const staticTransition = stillwindMerge({[stillwindClassKey]:"_s_be6f85cc"},{[stillwindStyleKey]:stillwindViewTransitionStyle(pageTitle,undefined,["--sw-v-7e58d129"],"--sw-vt-name","--sw-vt-class")});
export const staticKeyedTransition = stillwindMerge({[stillwindClassKey]:"_s_6c9e3039"},{[stillwindStyleKey]:stillwindViewTransitionStyle("--sw-v-7e58d129",transitionKey,["--sw-v-7e58d129"],"--sw-vt-name","--sw-vt-class")});
export const conditionalTransition = stillwindMerge({[stillwindClassKey]:"_s_062cacfe"},{[stillwindStyleKey]:stillwindViewTransitionStyle(selectedTransition,undefined,["--sw-v-7e58d129","--sw-v-5ce18560"],"--sw-vt-name","--sw-vt-class")});
export const conditionalKeyedTransition = stillwindMerge({[stillwindClassKey]:"_s_83d5525c"},{[stillwindStyleKey]:stillwindViewTransitionStyle(selectedTransition,transitionKey,["--sw-v-7e58d129","--sw-v-5ce18560"],"--sw-vt-name","--sw-vt-class")});
