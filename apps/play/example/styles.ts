import { style } from "stillwind/react";

/** Card that frames each region of the page. */
export const panel = style(
  "p-panel bg-surface rounded-panel border border-border shadow-panel",
);

/** Base for every button. `hocus` and `focus-ring` are defined in stillwind.css. */
export const control = style(
  "inline-flex px-3 py-1.5 items-center gap-2 text-sm font-medium rounded-control",
  "focus-visible:focus-ring hocus:bg-surface-muted",
);

/** Button that shows its selected state through `aria-pressed`. */
export const toggle = style(
  control,
  "text-muted aria-pressed:text-action-foreground aria-pressed:bg-action",
);

/** Text input or select in a form. */
export const field = style(
  "px-3 py-1.5 text-sm bg-canvas rounded-control border border-border placeholder:text-muted focus-visible:focus-ring",
);
