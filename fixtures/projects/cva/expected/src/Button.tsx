import {  type VariantProps } from "stillwind/react";
import { stillwindCva } from "stillwind/runtime/react";

export const button = stillwindCva("sw_li79w rounded", {
  variants: {
    size: {
      small: "sw_7q2q7",
      large: "sw_jwlf4",
    },
    disabled: {
      false: "",
      true: "sw_cnh4g",
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
      className: "sw_dndui",
    },
  ],
});

export function Button(props: VariantProps<typeof button>) {
  return <button {...button(props)}>Continue</button>;
}
