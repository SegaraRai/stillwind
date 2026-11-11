import { applyStyle, clearStyle  } from "stillwind/vanilla";
import { stillwindClsx, stillwindCssValue, stillwindClassObjectClassKey, stillwindClassObjectCustomPropertiesKey, stillwindClassObjectMerge, stillwindClassObjectToFragment } from "stillwind/runtime";

import { base } from "./styles";

export function update(button: HTMLButtonElement, active: boolean, tone: string) {
  applyStyle(button, stillwindClassObjectMerge({[stillwindClassObjectClassKey]:"sw_oplxh"},stillwindClassObjectToFragment((base)),{[stillwindClassObjectClassKey]:stillwindClsx((active) && ("sw_n7iyq")),[stillwindClassObjectCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}}));
}

export function dispose(button: HTMLButtonElement) {
  clearStyle(button);
}
