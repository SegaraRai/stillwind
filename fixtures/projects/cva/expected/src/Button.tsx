import {  type VariantProps } from "stillwind/react";
import { stillwindClassNameObjectCva } from "stillwind/runtime";

export const button = stillwindClassNameObjectCva(
  "sw_m4jpd",
  {
    variants: {
      size: {
        small: "sw_2jols",
        large: "sw_0id4f",
      },
      disabled: {
        false: "",
        true: "sw_tgtx6",
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
        className: "sw_aoat0",
      },
    ],
  },
);

export function Button(props: VariantProps<typeof button>) {
  return <button {...button(props)}>Continue</button>;
}
