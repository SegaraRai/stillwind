import { style } from "stillwind/react";

const props = style("pt-safe-or-4");
document.querySelector("#target")!.className = props.className ?? "";
