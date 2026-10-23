import { style as react } from "stillwind/react";
import { style } from "stillwind/vanilla";

export const base = react("rounded-md px-4 py-2");
export const choose = style.cva("font-medium", {
  variants: {
    active: {
      true: "underline",
      false: null,
    },
  },
});
