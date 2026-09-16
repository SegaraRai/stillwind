import { stillwindCssValue } from "stillwind/react/runtime";

export function Conditions({ enabled, second }: { enabled: boolean; second: boolean }) {
  return (
    <section
      {...{className:"_s_9ee3cee6",style:{"--enabled":stillwindCssValue(enabled),"--second":stillwindCssValue(second)}}}
    >
      <div {...{className:"_s_8fde1294"}} />
      <div
        {...{className:"_s_6752b06a",style:{"--mode":stillwindCssValue("compact")}}}
      />
    </section>
  );
}
