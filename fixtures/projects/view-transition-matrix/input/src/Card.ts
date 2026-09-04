import { style } from "stillwind/react";
import { pageTitle, selectedTransition } from "./transitions";

// Imported identities exercise the same TypeScript fact propagation used by
// the Vite project compiler.
export const cardTransition = style(pageTitle);
export const selectedCardTransition = style(selectedTransition);
