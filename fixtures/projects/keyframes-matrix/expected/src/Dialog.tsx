import { stillwindClsx } from "stillwind/runtime";

import { exit, selectedAnimation } from "./keyframes";

export const panelProps = {className:stillwindClsx("sw_3sup4",({"sw-kf-enter-zhg87":"sw_d6d1w","sw-kf-074gt":"sw_gairg"})[(selectedAnimation).k])};

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
