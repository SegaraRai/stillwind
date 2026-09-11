import { stillwindAssetClass, stillwindClsx } from "stillwind/react/runtime";

import { exit, selectedAnimation } from "./animations";

export const panelProps = {className:stillwindClsx("_s_895ad4cc",stillwindAssetClass(selectedAnimation("duration-normal ease-standard both"),{"--sw-a-5bc60e1e":"_s_c3f1d382","--sw-a-59b85d3d":"_s_3148efc7"}))};

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
