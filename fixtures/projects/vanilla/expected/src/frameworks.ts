import { stillwindClassKey, stillwindCssValue, stillwindCustomPropertiesKey, stillwindCva, stillwindMerge } from "stillwind/runtime/class-name-object";
import { stillwindClassKey as stillwindClassKey_2, stillwindCssValue as stillwindCssValue_2, stillwindCustomPropertiesKey as stillwindCustomPropertiesKey_2, stillwindMerge as stillwindMerge_2 } from "stillwind/runtime/class-string";
import { stillwindClassKey as stillwindClassKey_1, stillwindCssValue as stillwindCssValue_1, stillwindCustomPropertiesKey as stillwindCustomPropertiesKey_1, stillwindMerge as stillwindMerge_1 } from "stillwind/runtime/class-object";

export function frameworkProps(active: boolean, tone: string) {
  const choose = stillwindCva("sw_9ij3v", {
    variants: { active: { true: {[stillwindCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}}, false: "" } },
  });
  return {
    react: stillwindMerge({[stillwindClassKey]:"sw_oxkd1",[stillwindCustomPropertiesKey]:{"--tone":stillwindCssValue(tone)}},(active) && ({[stillwindCustomPropertiesKey]:{"--active":" "}})),
    vanilla: stillwindMerge_1({[stillwindClassKey_1]:"sw_23sbd",[stillwindCustomPropertiesKey_1]:{"--tone":stillwindCssValue_1(tone)}},(active) && ({[stillwindCustomPropertiesKey_1]:{"--active":" "}})),
    svelte: stillwindMerge_2({[stillwindClassKey_2]:"sw_y15vs",[stillwindCustomPropertiesKey_2]:{"--tone":stillwindCssValue_2(tone)}},(active) && ({[stillwindCustomPropertiesKey_2]:{"--active":" "}})),
    choice: choose({ active }),
  };
}
