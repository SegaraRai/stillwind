import { stillwindCssValue, stillwindClassObjectClassKey, stillwindClassObjectCustomPropertiesKey, stillwindClassObjectMerge, stillwindClassStringClassKey, stillwindClassStringCustomPropertiesKey, stillwindClassStringMerge, stillwindClassNameObjectClassKey, stillwindClassNameObjectCustomPropertiesKey, stillwindClassNameObjectCva, stillwindClassNameObjectMerge } from "stillwind/runtime";



export function frameworkProps(active: boolean, tone: string) {
  const choose = stillwindClassNameObjectCva("sw_1ki7e", {
    variants: { active: { true: {[stillwindClassNameObjectCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}}, false: "" } },
  });
  return {
    react: stillwindClassNameObjectMerge({[stillwindClassNameObjectClassKey]:"sw_g6run",[stillwindClassNameObjectCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}},(active) && ({[stillwindClassNameObjectCustomPropertiesKey]:{"--active":" "}})),
    vanilla: stillwindClassObjectMerge({[stillwindClassObjectClassKey]:"sw_vjkdq",[stillwindClassObjectCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}},(active) && ({[stillwindClassObjectCustomPropertiesKey]:{"--active":" "}})),
    svelte: stillwindClassStringMerge({[stillwindClassStringClassKey]:"sw_pbuwh",[stillwindClassStringCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}},(active) && ({[stillwindClassStringCustomPropertiesKey]:{"--active":" "}})),
    choice: choose({ active }),
  };
}
