import { stillwindClassKey, stillwindCssValue, stillwindCustomPropertiesKey, stillwindMerge } from "stillwind/react/runtime";

export function Conditions({ enabled, second }: { enabled: boolean; second: boolean }) {
  return (
    <section {...stillwindMerge({[stillwindClassKey]:"_s_3ddadc9e"},{[stillwindCustomPropertiesKey]:{["--enabled"]:stillwindCssValue((enabled))}},{[stillwindCustomPropertiesKey]:{["--second"]:stillwindCssValue((second))}})}>
      <div {...{className:"_s_249c1946"}} />
    </section>
  );
}
