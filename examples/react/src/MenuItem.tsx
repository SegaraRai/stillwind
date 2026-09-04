import { style } from "stillwind/react";
import { menuTrigger } from "./MenuScope";

export function MenuItem() {
  return (
    <span
      {...style("inline-flex rounded-md bg-accent px-4 py-2", {
        [menuTrigger`&:hover`]: "text-white",
      })}
    >
      Cross-module ref
    </span>
  );
}
