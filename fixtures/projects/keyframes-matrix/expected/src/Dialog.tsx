import { stillwindClsx } from "stillwind/runtime";

import { exit, selectedAnimation } from "./keyframes";

export const panelProps = {className:stillwindClsx("sw_rdg4x",({"sw-kf-enter-zhg87":"sw_qndp2","sw-kf-074gt":"sw_bkvqj"})[(selectedAnimation).k])};

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
