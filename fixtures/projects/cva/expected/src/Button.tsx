import {  type VariantProps } from "stillwind/react";
import { stillwindCva } from "stillwind/runtime/react";

export const button = stillwindCva(
  "sw_1chda",
  {
    variants: {
      size: {
        small: "sw_jy94h",
        large: "sw_riolb",
      },
      disabled: {
        false: "",
        true: "sw_1agbk",
      },
    },
    defaultVariants: {
      size: "small",
      disabled: false,
    },
    compoundVariants: [
      {
        size: ["small", "large"],
        disabled: false,
        className: "sw_9fmy7",
      },
    ],
  },
);

export function Button(props: VariantProps<typeof button>) {
  return <button {...button(props)}>Continue</button>;
}
