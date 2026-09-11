import { stillwindViewTransition, stillwindViewTransitionStyle } from "stillwind/react/runtime";

export const pageTitle = stillwindViewTransition("--sw-v-7e58d129");
export const pageCard = stillwindViewTransition("--sw-v-5ce18560");

export const compact = Date.now() % 2 === 0;
export const transitionKey = "fixture-product";
export const selectedTransition = compact ? pageTitle : pageCard;

export const staticTransition = {className:"_s_dd456996",style:stillwindViewTransitionStyle(pageTitle,undefined,["--sw-v-7e58d129"],"--sw-vt-name","--sw-vt-class")};
export const staticKeyedTransition = {className:"_s_4de531c3",style:stillwindViewTransitionStyle("--sw-v-7e58d129",transitionKey,["--sw-v-7e58d129"],"--sw-vt-name","--sw-vt-class")};
export const conditionalTransition = {className:"_s_3977cd6f",style:stillwindViewTransitionStyle(selectedTransition,undefined,["--sw-v-7e58d129","--sw-v-5ce18560"],"--sw-vt-name","--sw-vt-class")};
export const conditionalKeyedTransition = {className:"_s_bca849c5",style:stillwindViewTransitionStyle(selectedTransition,transitionKey,["--sw-v-7e58d129","--sw-v-5ce18560"],"--sw-vt-name","--sw-vt-class")};
