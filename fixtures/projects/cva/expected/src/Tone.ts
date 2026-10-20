import { stillwindClassKey, stillwindCssValue, stillwindCustomPropertiesKey, stillwindCva } from "stillwind/runtime/svelte";

export function createTone(tone: string) {
  return stillwindCva(
    {[stillwindClassKey]:"sw_nr42u",[stillwindCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}},
    {
      variants: {
        kind: {
          inherited: "",
          fixed: {[stillwindCustomPropertiesKey]:{"--tone":"red"}},
          emphasized: "sw_jtu0n",
        },
      },
      defaultVariants: {
        kind: "inherited",
      },
      compoundVariants: [
        {
          kind: "fixed",
          class: {[stillwindCustomPropertiesKey]:{"--tone":"blue"}},
        },
      ],
    },
  );
}
