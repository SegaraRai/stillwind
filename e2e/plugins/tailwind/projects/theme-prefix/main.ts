import { style } from "stillwind/react";

const group = style("join text-brand");
const button = style("btn btn-primary join-item");
document.querySelector("#group")!.className = group.className ?? "";
document.querySelector("#target")!.className = button.className ?? "";
