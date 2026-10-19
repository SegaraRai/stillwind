import { style } from "stillwind/react";

// The `hs-dropdown-open` variant selects `.hs-dropdown.open > &`, which needs literal classes.
const dropdown = style("hs-dropdown open");
const menu = style("hs-dropdown-open:bg-brand");
document.querySelector("#dropdown")!.className = dropdown.className ?? "";
document.querySelector("#target")!.className = menu.className ?? "";
