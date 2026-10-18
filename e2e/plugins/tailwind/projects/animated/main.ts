import { style } from "stillwind/react";

const props = style("animate-fade animate-duration-700");
document.querySelector("#target")!.className = props.className ?? "";
