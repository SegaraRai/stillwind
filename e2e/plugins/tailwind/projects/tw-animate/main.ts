import { style } from "stillwind/react";

const props = style("animate-in fade-in-50 duration-300");
document.querySelector("#target")!.className = props.className ?? "";
