import { style } from "stillwind/react";

const props = style("ui-open:bg-brand");
document.querySelector("#target")!.className = props.className ?? "";
