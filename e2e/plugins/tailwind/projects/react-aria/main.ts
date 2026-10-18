import { style } from "stillwind/react";

const props = style("selected:bg-brand");
document.querySelector("#target")!.className = props.className ?? "";
