import { style } from "stillwind/react";

const props = style("radix-state-open:bg-brand");
document.querySelector("#target")!.className = props.className ?? "";
