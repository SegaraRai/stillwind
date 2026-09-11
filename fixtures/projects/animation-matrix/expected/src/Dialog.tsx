import { stillwindAssetClass, stillwindClsx } from "stillwind/react/runtime";

import { exit, selectedAnimation } from "./animations";

export const panelProps = {className:stillwindClsx("_s_2cd391d8",stillwindAssetClass(selectedAnimation("duration-normal ease-standard both"),"--sw-a-5bc60e1e":"_s_31dcda56","--sw-a-59b85d3d":"_s_b60071b6"))};

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
