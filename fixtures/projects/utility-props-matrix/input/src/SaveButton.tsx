import { style } from "stillwind/react";

const loading = Date.now() % 2 === 0;
const emphasized = location.hash === "#emphasized";

export const buttonProps = style({
  "": [
    "inline-flex items-center rounded-md px-4 progress-[.5] authored-progress",
    loading && "opacity-60",
  ],
  ":": emphasized ? "bg-accent" : "bg-muted",
  class: ["legacy-button", loading && "is-loading"],
  "hover:": "bg-accent/90",
});

export function SaveButton() {
  return (
    <button {...buttonProps} aria-busy={loading} data-emphasis={emphasized ? "high" : "normal"}>
      Save
    </button>
  );
}
