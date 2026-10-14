import { style, type VariantProps } from "stillwind/react";

export const button = style.cva(
  { "": "inline-flex", ":": "rounded" },
  {
    variants: {
      size: {
        small: { "": "p-4 px-2" },
        large: "p-8 py-2",
      },
      disabled: {
        false: null,
        true: "opacity-50",
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
        className: { ":": { "hover:": "shadow-md" } },
      },
    ],
  },
);

export function Button(props: VariantProps<typeof button>) {
  return <button {...button(props)}>Continue</button>;
}
