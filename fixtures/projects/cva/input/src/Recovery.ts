import { style } from "stillwind/react";

export const recovered = style.cva("block", {
  variants: {
    size: { small: "p-4", unsupported: 42 },
  },
  defaultVariants: { size: "small", absent: "value" },
  compoundVariants: [
    { absent: "value", class: "opacity-50" },
    { size: "small", class: { "hover:": "shadow-md" } },
  ],
});
