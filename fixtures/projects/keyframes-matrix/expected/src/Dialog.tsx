import { stillwindAssetClass, stillwindClsx } from "stillwind/react/runtime";

import { exit, selectedAnimation } from "./keyframes";

export const panelProps = {className:stillwindClsx("sw_t9sxx",stillwindAssetClass(selectedAnimation("duration-normal ease-standard both"),{"sw-kf-enter-7nc28":"sw_s674c","sw-kf-tym79":"sw_35jbw"}))};

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
