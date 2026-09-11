import { stillwindClassKey, stillwindMerge, stillwindStyleKey, stillwindViewTransitionStyle } from "stillwind/react/runtime";

import { pageTitle, selectedTransition } from "./transitions";

// Imported identities exercise the same TypeScript fact propagation used by
// the Vite project compiler.
export const cardTransition = stillwindMerge({[stillwindClassKey]:"_s_655c5f11"},{[stillwindStyleKey]:stillwindViewTransitionStyle(pageTitle,undefined,["--sw-v-7e58d129"],"--sw-vt-name","--sw-vt-class")});
export const selectedCardTransition = stillwindMerge({[stillwindClassKey]:"_s_3ea37c87"},{[stillwindStyleKey]:stillwindViewTransitionStyle(selectedTransition,undefined,["--sw-v-7e58d129","--sw-v-5ce18560"],"--sw-vt-name","--sw-vt-class")});
