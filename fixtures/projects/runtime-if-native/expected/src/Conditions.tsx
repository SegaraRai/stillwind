import { stillwindClassKey, stillwindCssValue, stillwindCustomPropertiesKey, stillwindMerge } from "stillwind/react/runtime";

export function Conditions({ enabled, second }: { enabled: boolean; second: boolean }) {
  return (
    <section {...stillwindMerge({[stillwindClassKey]:"_s_3ddadc9e"},{[stillwindCustomPropertiesKey]:{["--enabled"]:stillwindCssValue((enabled))}},{[stillwindCustomPropertiesKey]:{["--second"]:stillwindCssValue((second))}})}>
      <div {...{className:"_s_25c0b58c"}} />
      <div {...stillwindMerge({[stillwindClassKey]:"_s_e86d655f"},{[stillwindCustomPropertiesKey]:{["--mode"]:stillwindCssValue(("compact"))}})} />
    </section>
  );
}
