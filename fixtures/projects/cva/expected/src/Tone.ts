import { stillwindClassKey, stillwindClsx, stillwindCssValue, stillwindCustomPropertiesKey, stillwindCva } from "stillwind/runtime/svelte";

export function createTone(tone: string, active: boolean) {
  return stillwindCva(
    {[stillwindClassKey]:"sw_eloo8",[stillwindCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}},
    {
      variants: {
        kind: {
          inherited: "",
          fixed: {[stillwindCustomPropertiesKey]:{"--tone":"red"}},
          guarded: stillwindClsx("sw_b9uo2",(active) && ("sw_jqgrh")),
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
