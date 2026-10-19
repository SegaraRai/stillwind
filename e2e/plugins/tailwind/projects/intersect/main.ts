import { style } from "stillwind/react";

const props = style("intersect:bg-brand");
document.querySelector("#target")!.className = props.className ?? "";
