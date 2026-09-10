import { style } from "stillwind/react";

import { exit, selectedAnimation } from "./animations";

export const panelProps = style(
  "rounded-md",
  selectedAnimation("duration-normal ease-standard both"),
  { "[&[data-state=closed]]:": exit("duration-fast ease-standard both") },
);

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
