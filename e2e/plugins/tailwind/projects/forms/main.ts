import { style } from "stillwind/react";

const props = style("form-input");
document.querySelector("#target")!.className = props.className ?? "";
