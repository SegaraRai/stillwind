import { style } from "stillwind/react";

const props = style("scrollbar-thin scrollbar-thumb-brand");
document.querySelector("#target")!.className = props.className ?? "";
