import { style } from "stillwind/react";
import { alternateTrigger, menuTrigger } from "./MenuScope";

export function MenuItem({ alternate = false }: { alternate?: boolean }) {
  const selected = alternate ? alternateTrigger : menuTrigger;
  return (
    <span
      {...style("inline-flex rounded-md bg-accent px-4 py-2", {
        [selected`&:hover`]: "text-white",
      })}
    >
      Cross-module symbolic ref
    </span>
  );
}
