import { style } from "stillwind/react";

export function show() {
  document.querySelector("#lazy").className = style("lazy").className;
}
