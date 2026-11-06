import { stillwindCssValue, stillwindClassStringClassKey, stillwindClassStringCustomPropertiesKey, stillwindClassStringCva } from "stillwind/runtime";

export function createTone(tone: string) {
  return stillwindClassStringCva(
    {[stillwindClassStringClassKey]:"sw_nr42u",[stillwindClassStringCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}},
    {
      variants: {
        kind: {
          inherited: "",
          fixed: {[stillwindClassStringCustomPropertiesKey]:{"--tone":"red"}},
          emphasized: "sw_jtu0n",
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
