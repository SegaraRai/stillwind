import { style } from "stillwind/vanilla";

export const base = style("rounded-md px-4 py-2");
export const choose = style.cva("font-medium", {
  variants: {
    active: {
      true: "underline",
      false: null,
    },
  },
});
