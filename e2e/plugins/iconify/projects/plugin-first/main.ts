import { style } from "stillwind/react";

document.querySelector("#icon-first")!.className =
  style("icon-lucide--home size-full").className ?? "";
document.querySelector("#size-first")!.className =
  style("size-full icon-lucide--home").className ?? "";
