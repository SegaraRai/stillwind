import { stillwindAssetClass, stillwindClsx } from "stillwind/react/runtime";

import { exit, selectedAnimation } from "./animations";

export const panelProps = {className:stillwindClsx("_s_3af2a4aa",stillwindAssetClass(selectedAnimation("duration-normal ease-standard both"),{"--sw-a-5bc60e1e":"_s_812594a0","--sw-a-59b85d3d":"_s_fa43b627"}))};

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
