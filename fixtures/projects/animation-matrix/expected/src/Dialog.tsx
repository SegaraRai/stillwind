import { stillwindAssetClass, stillwindClsx } from "stillwind/react/runtime";
import { exit, selectedAnimation } from "./animations";

export const panelProps = {className:stillwindClsx("_s_4c5285b6",stillwindAssetClass(selectedAnimation("duration-normal ease-standard both"),{"--sw-a-5bc60e1e":"_s_0075b133","--sw-a-59b85d3d":"_s_f89b7213"}))};

export function DialogPanel() {
  return <div {...panelProps} data-state="open" />;
}
