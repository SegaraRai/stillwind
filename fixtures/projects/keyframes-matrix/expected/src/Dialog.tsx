import { stillwindClsx } from "stillwind/runtime/class-name-object";

import { exit, selectedAnimation } from "./keyframes";

export const panelProps = {className:stillwindClsx("sw_rdg4x",({"sw-kf-enter-cxv19":"sw_oyibx","sw-kf-12rqq":"sw_0xhzs"})[(selectedAnimation).k])};

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
