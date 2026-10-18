import { style } from "stillwind/react";

const props = style("icon-[lucide--home] text-2xl");
document.querySelector("#target")!.className = props.className ?? "";
