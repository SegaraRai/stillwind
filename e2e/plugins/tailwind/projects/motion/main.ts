import { style } from "stillwind/react";

const props = style("motion-preset-fade motion-duration-700");
document.querySelector("#target")!.className = props.className ?? "";
