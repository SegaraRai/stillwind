import { stillwindClassKey, stillwindMerge } from "stillwind/react/runtime";

import { pageTitle, selectedTransition } from "./transitions";

// Imported identities exercise the same TypeScript fact propagation used by
// the Vite project compiler.
export const cardTransition = stillwindMerge({[stillwindClassKey]:(({"sw-vt-page-title-vtl83":"sw_rtuzf"})[pageTitle.v])});
export const selectedCardTransition = stillwindMerge({[stillwindClassKey]:(({"sw-vt-page-title-vtl83":"sw_vffuv","sw-vt-woqm1":"sw_06qde"})[selectedTransition.v])});
