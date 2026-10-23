import { applyStyle, clearStyle, style } from "stillwind/vanilla";

import { base } from "./styles";

export function update(button: HTMLButtonElement, active: boolean, tone: string) {
  applyStyle(button, style(base, "text-(--tone)", active && "font-bold", { "--tone": tone }));
}

export function dispose(button: HTMLButtonElement) {
  clearStyle(button);
}
