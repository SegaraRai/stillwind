import { applyStyle, clearStyle  } from "stillwind/vanilla";
import { stillwindClassKey, stillwindClsx, stillwindCssValue, stillwindCustomPropertiesKey, stillwindMerge, stillwindToFragment } from "stillwind/runtime/class-object";

import { base } from "./styles";

export function update(button: HTMLButtonElement, active: boolean, tone: string) {
  applyStyle(button, stillwindMerge({[stillwindClassKey]:"sw_0d02s"},stillwindToFragment((base)),{[stillwindClassKey]:stillwindClsx((active) && ("sw_5lom8")),[stillwindCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}}));
}

export function dispose(button: HTMLButtonElement) {
  clearStyle(button);
}
