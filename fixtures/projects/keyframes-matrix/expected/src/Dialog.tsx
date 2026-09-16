import { stillwindAssetClass, stillwindClsx } from "stillwind/react/runtime";

import { exit, selectedAnimation } from "./keyframes";

export const panelProps = {className:stillwindClsx("_s_09430877",stillwindAssetClass(selectedAnimation("duration-normal ease-standard both"),{"--sw-a-59fbe1ad":"_s_e5866475","--sw-a-29446666":"_s_2e7a7f27"}))};

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
