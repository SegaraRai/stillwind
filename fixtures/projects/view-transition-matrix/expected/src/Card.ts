import { stillwindClassKey, stillwindMerge } from "stillwind/react/runtime";

import { pageTitle, selectedTransition } from "./transitions";

// Imported identities exercise the same TypeScript fact propagation used by
// the Vite project compiler.
export const cardTransition = stillwindMerge({[stillwindClassKey]:(({"--sw-v-7e58d129":"_s_e75a26c8"})[pageTitle.v])});
export const selectedCardTransition = stillwindMerge({[stillwindClassKey]:(({"--sw-v-7e58d129":"_s_04565c77","--sw-v-5ce18560":"_s_6ca79e06"})[selectedTransition.v])});
