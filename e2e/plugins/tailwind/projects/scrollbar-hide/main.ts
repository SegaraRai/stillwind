import { style } from "stillwind/react";

const props = style("scrollbar-hide");
document.querySelector("#target")!.className = props.className ?? "";
