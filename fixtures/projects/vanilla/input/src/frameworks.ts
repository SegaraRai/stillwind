import { style as react } from "stillwind/react";
import { style as svelte } from "stillwind/svelte";
import { style as vanilla } from "stillwind/vanilla";

export function frameworkProps(active: boolean, tone: string) {
  const choose = react.cva("font-medium", {
    variants: { active: { true: { "--tone": tone }, false: null } },
  });
  return {
    react: react("p-4", { "--tone": tone }, active && { "--active": true }),
    vanilla: vanilla("p-4", { "--tone": tone }, active && { "--active": true }),
    svelte: svelte("p-4", { "--tone": tone }, active && { "--active": true }),
    choice: choose({ active }),
  };
}
