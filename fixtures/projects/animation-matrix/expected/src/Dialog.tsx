import { stillwindAssetClass, stillwindClsx } from "stillwind/react/runtime";

import { exit, selectedAnimation } from "./animations";

export const panelProps = {className:stillwindClsx("_s_4ac10ad6",stillwindAssetClass(selectedAnimation("duration-normal ease-standard both"),{"--sw-a-5bc60e1e":"_s_740e1092","--sw-a-59b85d3d":"_s_56b7698e"}))};

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
