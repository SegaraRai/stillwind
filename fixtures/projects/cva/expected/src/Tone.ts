import { stillwindCssValue, stillwindClassStringClassKey, stillwindClassStringCustomPropertiesKey, stillwindClassStringCva } from "stillwind/runtime";

export function createTone(tone: string) {
  return stillwindClassStringCva(
    {[stillwindClassStringClassKey]:"sw_k7zjz",[stillwindClassStringCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}},
    {
      variants: {
        kind: {
          inherited: "",
          fixed: {[stillwindClassStringCustomPropertiesKey]:{"--tone":"red"}},
          emphasized: "sw_ur6v5",
        },
      },
      defaultVariants: {
        kind: "inherited",
      },
      compoundVariants: [
        {
          kind: "fixed",
          class: {[stillwindClassStringCustomPropertiesKey]:{"--tone":"blue"}},
        },
      ],
    },
  );
}
